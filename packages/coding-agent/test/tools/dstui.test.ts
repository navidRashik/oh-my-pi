import { describe, expect, it } from "bun:test";
import type { AgentToolContext } from "@oh-my-pi/pi-agent-core";
import { Settings } from "@oh-my-pi/pi-coding-agent/config/settings";
import type { ToolSession } from "@oh-my-pi/pi-coding-agent/tools";
import { DstuiTool } from "@oh-my-pi/pi-coding-agent/tools/dstui";
import { ToolAbortError } from "@oh-my-pi/pi-coding-agent/tools/tool-errors";
import type { Component } from "@oh-my-pi/pi-tui";

function makeSession(overrides: Partial<ToolSession> = {}): ToolSession {
	const settings = Settings.isolated();
	return {
		cwd: "/tmp/test",
		hasUI: true,
		getSessionFile: () => null,
		getSessionSpawns: () => "*",
		settings,
		...overrides,
	};
}

function makeContext(): {
	context: AgentToolContext;
	rendered(width: number): string[];
	drive(input: string): void;
} {
	let active: Component | undefined;
	const ui = {
		custom: async <T>(
			factory: (
				tui: unknown,
				theme: unknown,
				keybindings: unknown,
				done: (result: T) => void,
			) => Component | Promise<Component>,
		): Promise<T> => {
			const { promise, resolve } = Promise.withResolvers<T>();
			active = await factory(null, null, null, value => resolve(value));
			return promise;
		},
	};
	return {
		context: { hasUI: true, ui, abort: () => {} } as unknown as AgentToolContext,
		rendered: width => active?.render(width) ?? [],
		drive: input => active?.handleInput?.(input),
	};
}

describe("DstuiTool.createIf", () => {
	it("returns null when UI is unavailable", () => {
		const session = makeSession({ hasUI: false });
		session.settings.set("dstui.enabled", true);
		expect(DstuiTool.createIf(session)).toBeNull();
	});

	it("returns null when dstui.enabled is false (default)", () => {
		const session = makeSession();
		expect(session.settings.get("dstui.enabled")).toBe(false);
		expect(DstuiTool.createIf(session)).toBeNull();
	});

	it("returns a tool instance when enabled and UI is available", () => {
		const session = makeSession();
		session.settings.set("dstui.enabled", true);
		const tool = DstuiTool.createIf(session);
		expect(tool).not.toBeNull();
		expect(tool?.name).toBe("dstui");
	});
});

describe("DstuiTool.execute", () => {
	function makeTool(): DstuiTool {
		const session = makeSession();
		session.settings.set("dstui.enabled", true);
		const tool = DstuiTool.createIf(session);
		if (!tool) throw new Error("tool gate misconfigured");
		return tool;
	}

	it("rejects when neither source nor store is provided", async () => {
		const tool = makeTool();
		const ctx = makeContext();
		const result = await tool.execute("id", {}, undefined, undefined, ctx.context);
		expect(result.content?.[0]).toMatchObject({
			type: "text",
			text: expect.stringMatching(/requires `source` or `store`/),
		});
	});

	it("aborts when no interactive UI is in context", async () => {
		const tool = makeTool();
		let aborted = false;
		const ctx = { hasUI: false, abort: () => (aborted = true) } as unknown as AgentToolContext;
		await expect(
			tool.execute("id", { source: '(defcomponent t () (view (text "x")))' }, undefined, undefined, ctx),
		).rejects.toBeInstanceOf(ToolAbortError);
		expect(aborted).toBe(true);
	});

	it("mounts inline source and returns the settle value on emit", async () => {
		const tool = makeTool();
		const ctx = makeContext();
		const promise = tool.execute(
			"id",
			{
				source: `
					(defcomponent picker ()
						(state (idx 0))
						(view (text (str idx)))
						(bind :right (set! idx (+ idx 1)))
						(bind :enter (emit idx)))
				`,
			},
			undefined,
			undefined,
			ctx.context,
		);
		await Promise.resolve();
		ctx.drive("\u001b[C");
		ctx.drive("\u001b[C");
		ctx.drive("\r");
		const result = await promise;
		expect(result.details).toMatchObject({ source: "inline", settle: { reason: "emit", value: 2 } });
		expect(result.content?.[0]).toMatchObject({ type: "text", text: "User confirmed: 2" });
	});

	it("reports cancel on Esc and never persists state", async () => {
		const tool = makeTool();
		const ctx = makeContext();
		const promise = tool.execute(
			"id",
			{ source: `(defcomponent t () (view (text "x")) (bind :enter (emit 1)))` },
			undefined,
			undefined,
			ctx.context,
		);
		await Promise.resolve();
		ctx.drive("\u001b");
		const result = await promise;
		expect(result.details?.settle).toEqual({ reason: "cancel", value: null });
		expect(result.content?.[0]).toMatchObject({ type: "text", text: expect.stringMatching(/User cancelled/) });
	});
});
