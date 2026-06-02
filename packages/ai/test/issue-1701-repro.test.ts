/**
 * Repro for #1701 — omp emits a `tool_choice` naming a function that is
 * absent from the same request's `tools` array, producing a self-inconsistent
 * request body. Spec-strict OpenAI-compatible endpoints reject it with
 * `400 invalid_parameter_error: The tool specified in tool_choice does not
 * match any of the specified tools`.
 *
 * The fix is symmetric to the `tool_choice: "none"` guard handling #1227: the
 * request builder drops a forced named `tool_choice` whenever its named
 * function is not in `params.tools`. The primary defense lives in the agent
 * loop (`refreshToolChoiceForActiveTools` now runs on the queued choice too,
 * not just `options.toolChoice`), but the request builder is the last line of
 * defense for any other caller of `streamOpenAICompletions` (raw SDK use,
 * external callers) emitting a mismatched pair.
 */
import { describe, expect, it } from "bun:test";
import { getBundledModel } from "@oh-my-pi/pi-ai/models";
import { streamOpenAICompletions } from "@oh-my-pi/pi-ai/providers/openai-completions";
import type { Context, Model, Tool } from "@oh-my-pi/pi-ai/types";
import * as z from "zod/v4";

function abortedSignal(): AbortSignal {
	const controller = new AbortController();
	controller.abort();
	return controller.signal;
}

function openaiCompletionsModel(): Model<"openai-completions"> {
	return {
		...getBundledModel("openai", "gpt-4o-mini"),
		api: "openai-completions",
		id: "glm-5.1",
		name: "GLM 5.1 (test)",
		provider: "alibaba-modelstudio",
		baseUrl: "https://example.test/v1",
	};
}

async function capturePayload(
	context: Context,
	opts: Parameters<typeof streamOpenAICompletions>[2],
): Promise<Record<string, unknown>> {
	const { promise, resolve } = Promise.withResolvers<unknown>();
	streamOpenAICompletions(openaiCompletionsModel(), context, {
		...opts,
		apiKey: "test-key",
		signal: abortedSignal(),
		onPayload: payload => resolve(payload),
	});
	return (await promise) as Record<string, unknown>;
}

const forkAgentTool: Tool = {
	name: "fork_agent",
	description: "Fork a subagent",
	parameters: z.object({ prompt: z.string() }),
};

const dataforseoSearchTool: Tool = {
	name: "dataforseo_search",
	description: "Search via DataForSEO",
	parameters: z.object({ query: z.string() }),
};

const todoWriteTool: Tool = {
	name: "todo_write",
	description: "Manage a phased task list",
	parameters: z.object({ ops: z.array(z.object({ op: z.string() })) }),
};

describe("issue #1701 — eager-todo forces tool_choice for a tool absent from tools", () => {
	it("drops forced tool_choice when the named function is not in params.tools", async () => {
		// Reproduces the exact mismatch in the captured request body from the issue:
		// a restricted active tool set [fork_agent, dataforseo_search] paired with
		// a forced tool_choice naming todo_write. Without the guard, the wire body
		// is internally inconsistent and strict providers return 400.
		const body = await capturePayload(
			{
				messages: [{ role: "user", content: "do the thing", timestamp: Date.now() }],
				tools: [forkAgentTool, dataforseoSearchTool],
			},
			{ toolChoice: { type: "tool", name: "todo_write" } },
		);

		expect(Array.isArray(body.tools)).toBe(true);
		expect((body.tools as { function: { name: string } }[]).map(tool => tool.function.name)).toEqual([
			"fork_agent",
			"dataforseo_search",
		]);
		expect(body.tool_choice).toBeUndefined();
	});

	it("keeps forced tool_choice when the named function is present in params.tools", async () => {
		// Sanity: when the named tool IS offered, the forced choice survives — the
		// guard only drops self-inconsistent pairs.
		const body = await capturePayload(
			{
				messages: [{ role: "user", content: "list everything", timestamp: Date.now() }],
				tools: [forkAgentTool, todoWriteTool],
			},
			{ toolChoice: { type: "tool", name: "todo_write" } },
		);

		expect(Array.isArray(body.tools)).toBe(true);
		expect(body.tool_choice).toMatchObject({ type: "function", function: { name: "todo_write" } });
	});

	it("drops forced tool_choice when params.tools is empty", async () => {
		// Empty `tools` (e.g. /btw side channels with a stray forced choice) is the
		// degenerate case of the mismatch. Belt-and-suspenders coverage.
		const body = await capturePayload(
			{
				messages: [{ role: "user", content: "do the thing", timestamp: Date.now() }],
				tools: [],
			},
			{ toolChoice: { type: "tool", name: "todo_write" } },
		);

		expect(body.tools).toBeUndefined();
		expect(body.tool_choice).toBeUndefined();
	});
});
