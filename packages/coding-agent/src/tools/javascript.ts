import type * as fs from "node:fs";
import * as module from "node:module";
import * as path from "node:path";
import * as util from "node:util";
import * as vm from "node:vm";
import type { AgentTool, AgentToolContext, AgentToolResult, AgentToolUpdateCallback } from "@oh-my-pi/pi-agent-core";
import type { Component } from "@oh-my-pi/pi-tui";
import { Text } from "@oh-my-pi/pi-tui";
import { type Static, Type } from "@sinclair/typebox";
import { renderPromptTemplate } from "../config/prompt-templates";
import type { RenderResultOptions } from "../extensibility/custom-tools/types";
import type { Theme } from "../modes/theme/theme";
import javascriptDescription from "../prompts/tools/javascript.md" with { type: "text" };
import { renderCodeCell, renderStatusLine } from "../tui";
import type { ToolSession } from ".";
import type { OutputMeta } from "./output-meta";
import { resolveToCwd } from "./path-utils";
import { PREVIEW_LIMITS, truncateToWidth } from "./render-utils";
import { ToolAbortError, ToolError } from "./tool-errors";
import { toolResult } from "./tool-result";
import { clampTimeout } from "./tool-timeouts";

export interface JavaScriptStatusEvent {
	op: string;
	[key: string]: unknown;
}

export const javascriptSchema = Type.Object({
	cells: Type.Array(
		Type.Object({
			code: Type.String({ description: "JavaScript code to execute" }),
			title: Type.Optional(Type.String({ description: "Cell label, e.g. 'imports', 'helper'" })),
		}),
		{ description: "Cells to execute sequentially in persistent VM context" },
	),
	timeout: Type.Optional(Type.Number({ description: "Timeout in seconds (default: 30)" })),
	cwd: Type.Optional(Type.String({ description: "Working directory (default: cwd)" })),
	reset: Type.Optional(Type.Boolean({ description: "Reset VM context before execution" })),
});

export type JavaScriptToolParams = Static<typeof javascriptSchema>;

export interface JavaScriptCellResult {
	index: number;
	title?: string;
	code: string;
	output: string;
	status: "pending" | "running" | "complete" | "error";
	durationMs?: number;
	error?: string;
	statusEvents?: JavaScriptStatusEvent[];
}

export interface JavaScriptToolDetails {
	cells: JavaScriptCellResult[];
	sessionKey?: string;
	statusEvents?: JavaScriptStatusEvent[];
	exports?: string[];
	isError?: boolean;
	meta?: OutputMeta;
}

type OutputWriter = (chunk: string) => void;
type StatusWriter = (event: JavaScriptStatusEvent) => void;

type JavaScriptVmState = {
	context: vm.Context;
	exportsObject: Record<string, unknown>;
	setWriter: (writer: OutputWriter) => void;
	setStatusWriter: (writer: StatusWriter) => void;
	setRuntimeContext: (context: JavaScriptRuntimeContext) => void;
};

type JavaScriptRuntimeContext = {
	cwd: string;
	sessionKey: string;
	toolCallId: string;
	hasUI: boolean;
	toolNames: string[];
};

const VM_STATES = new Map<string, JavaScriptVmState>();

function createAbortPromise(signal: AbortSignal): Promise<never> {
	const { promise, reject } = Promise.withResolvers<never>();
	if (signal.aborted) {
		reject(new ToolAbortError());
		return promise;
	}
	const onAbort = () => reject(new ToolAbortError());
	signal.addEventListener("abort", onAbort, { once: true });
	return promise.finally(() => {
		signal.removeEventListener("abort", onAbort);
	});
}

async function resolveCellValue(value: unknown, signal: AbortSignal): Promise<unknown> {
	if (!(value instanceof Promise)) {
		return value;
	}
	return await Promise.race([value, createAbortPromise(signal)]);
}

function formatEvalValue(value: unknown): string | undefined {
	if (value === undefined) {
		return undefined;
	}
	if (typeof value === "string") {
		return value;
	}
	return util.inspect(value, {
		colors: false,
		depth: 6,
		maxArrayLength: 100,
		maxStringLength: 10_000,
		compact: false,
		sorted: true,
	});
}

function appendLine(writer: OutputWriter, ...args: unknown[]): void {
	writer(`${util.formatWithOptions({ colors: false }, ...args)}\n`);
}

function asStatusEvent(op: unknown, data: unknown): JavaScriptStatusEvent {
	if (typeof op === "object" && op !== null && "op" in op) {
		return op as JavaScriptStatusEvent;
	}
	const event: JavaScriptStatusEvent = {
		op: typeof op === "string" && op.trim().length > 0 ? op : "notify",
	};
	if (data && typeof data === "object") {
		Object.assign(event, data as Record<string, unknown>);
	} else if (data !== undefined) {
		event.value = data;
	}
	return event;
}

function createVmState(commandCwd: string): JavaScriptVmState {
	let writer: OutputWriter = () => {};
	let statusWriter: StatusWriter = () => {};
	const exportsObject: Record<string, unknown> = {};
	const runtimeContext: JavaScriptRuntimeContext = {
		cwd: commandCwd,
		sessionKey: "",
		toolCallId: "",
		hasUI: false,
		toolNames: [],
	};

	const localRequire = module.createRequire(path.join(commandCwd, "__omp_js_tool__.cjs"));

	const ompApi = {
		notify: (op: unknown, data?: unknown) => {
			statusWriter(asStatusEvent(op, data));
		},
		export: (name: string, value: unknown) => {
			exportsObject[name] = value;
			return value;
		},
		getExport: (name: string) => exportsObject[name],
		listExports: () => Object.keys(exportsObject).sort(),
		exports: exportsObject,
		ctx: runtimeContext,
	};

	const consoleProxy = {
		log: (...args: unknown[]) => appendLine(writer, ...args),
		info: (...args: unknown[]) => appendLine(writer, ...args),
		warn: (...args: unknown[]) => appendLine(writer, ...args),
		error: (...args: unknown[]) => appendLine(writer, ...args),
		dir: (value: unknown) => appendLine(writer, util.inspect(value, { colors: false, depth: 6 })),
	};

	const sandbox: Record<string, unknown> = {
		console: consoleProxy,
		require: localRequire,
		process,
		Buffer,
		setTimeout,
		clearTimeout,
		setInterval,
		clearInterval,
		setImmediate,
		clearImmediate,
		queueMicrotask,
		URL,
		URLSearchParams,
		TextEncoder,
		TextDecoder,
		AbortController,
		AbortSignal,
		atob,
		btoa,
		fetch,
		__omp: ompApi,
		notify: ompApi.notify,
		setExport: ompApi.export,
		getExport: ompApi.getExport,
		listExports: ompApi.listExports,
		exports: exportsObject,
		ctx: runtimeContext,
	};
	sandbox.globalThis = sandbox;

	const context = vm.createContext(sandbox, {
		name: `omp-js:${commandCwd}`,
		codeGeneration: { strings: true, wasm: true },
	});

	return {
		context,
		exportsObject,
		setWriter(next: OutputWriter) {
			writer = next;
		},
		setStatusWriter(next: StatusWriter) {
			statusWriter = next;
		},
		setRuntimeContext(next: JavaScriptRuntimeContext) {
			runtimeContext.cwd = next.cwd;
			runtimeContext.sessionKey = next.sessionKey;
			runtimeContext.toolCallId = next.toolCallId;
			runtimeContext.hasUI = next.hasUI;
			runtimeContext.toolNames = [...next.toolNames];
		},
	};
}

function getSessionKey(session: ToolSession, commandCwd: string): string {
	const sessionFile = session.getSessionFile?.() ?? undefined;
	return sessionFile ? `session:${sessionFile}:cwd:${commandCwd}` : `cwd:${commandCwd}`;
}

function getOrCreateVmState(sessionKey: string, commandCwd: string, reset: boolean): JavaScriptVmState {
	if (reset) {
		VM_STATES.delete(sessionKey);
	}
	let state = VM_STATES.get(sessionKey);
	if (!state) {
		state = createVmState(commandCwd);
		VM_STATES.set(sessionKey, state);
	}
	return state;
}

function formatStatusEvent(event: JavaScriptStatusEvent): string {
	const { op, ...rest } = event;
	const detail = Object.entries(rest)
		.map(([key, value]) => `${key}=${typeof value === "string" ? value : JSON.stringify(value)}`)
		.join(" ");
	return detail.length > 0 ? `${op} ${detail}` : op;
}

export class JavaScriptTool implements AgentTool<typeof javascriptSchema, JavaScriptToolDetails> {
	readonly name = "javascript";
	readonly label = "JavaScript";
	readonly description = renderPromptTemplate(javascriptDescription, {});
	readonly parameters = javascriptSchema;
	readonly concurrency = "exclusive";
	readonly strict = true;

	constructor(private readonly session: ToolSession | null) {}

	async execute(
		toolCallId: string,
		params: JavaScriptToolParams,
		signal?: AbortSignal,
		onUpdate?: AgentToolUpdateCallback<JavaScriptToolDetails>,
		ctx?: AgentToolContext,
	): Promise<AgentToolResult<JavaScriptToolDetails>> {
		if (!this.session) {
			throw new ToolError("JavaScript tool requires a session");
		}

		if (signal?.aborted) {
			throw new ToolAbortError();
		}

		const { cells, timeout: rawTimeout = 30, cwd, reset = false } = params;
		const timeoutSec = clampTimeout("javascript", rawTimeout);
		const timeoutSignal = AbortSignal.timeout(timeoutSec * 1000);
		const combinedSignal = signal ? AbortSignal.any([signal, timeoutSignal]) : timeoutSignal;

		const commandCwd = cwd ? resolveToCwd(cwd, this.session.cwd) : this.session.cwd;
		let cwdStat: fs.Stats;
		try {
			cwdStat = await Bun.file(commandCwd).stat();
		} catch {
			throw new ToolError(`Working directory does not exist: ${commandCwd}`);
		}
		if (!cwdStat.isDirectory()) {
			throw new ToolError(`Working directory is not a directory: ${commandCwd}`);
		}

		const sessionKey = getSessionKey(this.session, commandCwd);
		const state = getOrCreateVmState(sessionKey, commandCwd, reset);
		state.setRuntimeContext({
			cwd: commandCwd,
			sessionKey,
			toolCallId,
			hasUI: ctx?.hasUI === true,
			toolNames: Array.isArray(ctx?.toolNames) ? [...ctx.toolNames] : [],
		});

		const statusEvents: JavaScriptStatusEvent[] = [];
		const cellResults: JavaScriptCellResult[] = cells.map((cell, index) => ({
			index,
			title: cell.title,
			code: cell.code,
			output: "",
			status: "pending",
		}));
		const outputBlocks: string[] = [];

		const buildDetails = (): JavaScriptToolDetails => ({
			cells: cellResults.map(cell => ({
				...cell,
				statusEvents: cell.statusEvents ? [...cell.statusEvents] : undefined,
			})),
			sessionKey,
			statusEvents: statusEvents.length > 0 ? [...statusEvents] : undefined,
			exports: Object.keys(state.exportsObject).sort(),
		});

		const pushUpdate = () => {
			if (!onUpdate) return;
			onUpdate({
				content: [{ type: "text", text: outputBlocks.join("\n\n") }],
				details: buildDetails(),
			});
		};

		for (let i = 0; i < cells.length; i++) {
			if (combinedSignal.aborted) {
				throw new ToolAbortError();
			}

			const cell = cells[i];
			const cellResult = cellResults[i];
			cellResult.status = "running";
			cellResult.output = "";
			cellResult.error = undefined;
			cellResult.durationMs = undefined;
			cellResult.statusEvents = undefined;
			pushUpdate();

			let cellOutput = "";
			const cellStatusEvents: JavaScriptStatusEvent[] = [];
			state.setWriter(chunk => {
				cellOutput += chunk;
			});
			state.setStatusWriter(event => {
				statusEvents.push(event);
				cellStatusEvents.push(event);
			});

			const start = Date.now();
			try {
				const script = new vm.Script(cell.code, {
					filename: `javascript-cell-${i + 1}.mjs`,
				});
				const rawValue = script.runInContext(state.context, { timeout: timeoutSec * 1000 });
				const resolvedValue = await resolveCellValue(rawValue, combinedSignal);
				const valueText = formatEvalValue(resolvedValue);
				const lines = [cellOutput.trimEnd(), valueText ?? ""].filter(Boolean);
				const merged = lines.join("\n");
				cellResult.output = merged;
				cellResult.durationMs = Date.now() - start;
				cellResult.status = "complete";
				cellResult.statusEvents = cellStatusEvents.length > 0 ? [...cellStatusEvents] : undefined;

				const prefix = `[${i + 1}/${cells.length}]${cell.title ? ` ${cell.title}` : ""}`;
				outputBlocks.push(merged ? `${prefix}\n${merged}` : `${prefix} (ok)`);
				pushUpdate();
			} catch (err) {
				const message = err instanceof Error ? err.message : String(err);
				const stack = err instanceof Error && err.stack ? err.stack : message;
				const merged = [cellOutput.trimEnd(), stack].filter(Boolean).join("\n");
				cellResult.output = merged;
				cellResult.durationMs = Date.now() - start;
				cellResult.status = "error";
				cellResult.error = message;
				cellResult.statusEvents = cellStatusEvents.length > 0 ? [...cellStatusEvents] : undefined;

				const prefix = `[${i + 1}/${cells.length}]${cell.title ? ` ${cell.title}` : ""}`;
				outputBlocks.push(merged ? `${prefix}\n${merged}` : `${prefix}\n${message}`);
				pushUpdate();

				state.setWriter(() => {});
				state.setStatusWriter(() => {});
				const text = `${outputBlocks.join("\n\n")}\n\nCell ${i + 1} failed. Earlier cells succeeded and their state persists.`;
				return toolResult<JavaScriptToolDetails>({
					...buildDetails(),
					isError: true,
				})
					.text(text.trim())
					.done();
			}
		}

		state.setWriter(() => {});
		state.setStatusWriter(() => {});
		const outputText = outputBlocks.join("\n\n") || "(ok)";
		return toolResult<JavaScriptToolDetails>(buildDetails()).text(outputText).done();
	}
}

export const javascriptToolRenderer = {
	renderCall(args: JavaScriptToolParams, _options: RenderResultOptions, uiTheme: Theme): Component {
		const cell = args.cells?.[0];
		if (!cell) {
			return new Text(uiTheme.fg("muted", "No code"));
		}
		const lines = renderCodeCell(
			{
				code: cell.code,
				title: cell.title,
				index: 0,
				total: args.cells.length,
				status: "pending",
				language: "javascript",
				width: 120,
				outputMaxLines: PREVIEW_LIMITS.OUTPUT_EXPANDED,
			},
			uiTheme,
		);
		return new Text(lines.join("\n"), 1, 0);
	},

	renderResult(
		result: { content: Array<{ type: string; text?: string }>; details?: JavaScriptToolDetails },
		options: RenderResultOptions,
		uiTheme: Theme,
	): Component {
		const details = result.details;
		const text =
			result.content
				.filter(block => block.type === "text")
				.map(block => block.text ?? "")
				.join("\n") || "";

		if (details?.cells && details.cells.length > 0) {
			const rendered: string[] = [];
			for (const cell of details.cells) {
				const statusText = cell.statusEvents?.map(formatStatusEvent).join("\n");
				const combinedOutput = [cell.output, statusText].filter(Boolean).join("\n");
				rendered.push(
					...renderCodeCell(
						{
							code: cell.code,
							title: cell.title,
							index: cell.index,
							total: details.cells.length,
							status: cell.status,
							duration: cell.durationMs,
							output: combinedOutput,
							language: "javascript",
							expanded: options.expanded,
							outputMaxLines: PREVIEW_LIMITS.OUTPUT_EXPANDED,
							width: 120,
						},
						uiTheme,
					),
				);
			}

			if ((details.exports?.length ?? 0) > 0) {
				rendered.push("");
				rendered.push(
					renderStatusLine(
						{
							title: "Exports",
							description: details.exports!.join(", "),
							icon: "success",
						},
						uiTheme,
					),
				);
			}
			return new Text(rendered.join("\n"), 1, 0);
		}

		const lines = text.split("\n").map(line => truncateToWidth(line, 120));
		return new Text(lines.join("\n"), 1, 0);
	},
};
