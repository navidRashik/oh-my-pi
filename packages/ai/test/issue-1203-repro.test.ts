import { afterEach, describe, expect, it } from "bun:test";
import { getBundledModel } from "../src/models";
import { streamOpenAICompletions } from "../src/providers/openai-completions";
import type { Context, Model } from "../src/types";

const originalFetch = global.fetch;

afterEach(() => {
	global.fetch = originalFetch;
});

function createSseResponse(events: unknown[]): Response {
	const payload = `${events
		.map(event => `data: ${typeof event === "string" ? event : JSON.stringify(event)}`)
		.join("\n\n")}\n\n`;
	return new Response(payload, {
		status: 200,
		headers: { "content-type": "text/event-stream" },
	});
}

function createMockFetch(events: unknown[]): typeof fetch {
	async function mockFetch(_input: string | URL | Request, _init?: RequestInit): Promise<Response> {
		return createSseResponse(events);
	}
	return Object.assign(mockFetch, { preconnect: originalFetch.preconnect });
}

function baseContext(): Context {
	return {
		messages: [{ role: "user", content: "hello", timestamp: Date.now() }],
	};
}

/**
 * Regression test for issue #1203.
 *
 * MiniMax Coding Plan CN (`minimax-code-cn`) streams reasoning inside
 * `delta.content` wrapped in `<think>...</think>` tags — the same wire
 * format as the international `minimax-code` provider. The tag-parser gate
 * in `streamOpenAICompletions` previously excluded `minimax-code-cn`,
 * causing the raw `<think>` markup to be stored as visible assistant text
 * instead of a structured `thinking` content block.
 */
describe("issue #1203 — minimax-code-cn <think> tag parser", () => {
	const model = getBundledModel("minimax-code-cn", "MiniMax-M2.5") as Model<"openai-completions">;

	function thinkTagSseEvents(modelId: string): unknown[] {
		return [
			{
				id: "chatcmpl-minimax-cn-1",
				object: "chat.completion.chunk",
				created: 0,
				model: modelId,
				choices: [{ index: 0, delta: { content: "<think>", role: "assistant" } }],
			},
			{
				id: "chatcmpl-minimax-cn-1",
				object: "chat.completion.chunk",
				created: 0,
				model: modelId,
				choices: [{ index: 0, delta: { content: "The user wrote in Chinese: ...", role: "assistant" } }],
			},
			{
				id: "chatcmpl-minimax-cn-1",
				object: "chat.completion.chunk",
				created: 0,
				model: modelId,
				choices: [{ index: 0, delta: { content: "</think>", role: "assistant" } }],
			},
			{
				id: "chatcmpl-minimax-cn-1",
				object: "chat.completion.chunk",
				created: 0,
				model: modelId,
				choices: [{ index: 0, delta: { content: "完成", role: "assistant" } }],
			},
			{
				id: "chatcmpl-minimax-cn-1",
				object: "chat.completion.chunk",
				created: 0,
				model: modelId,
				choices: [{ index: 0, delta: {}, finish_reason: "stop" }],
			},
			"[DONE]",
		];
	}

	it("parses <think>...</think> as a structured thinking block", async () => {
		global.fetch = createMockFetch(thinkTagSseEvents(model.id));

		const result = await streamOpenAICompletions(model, baseContext(), { apiKey: "test-key" }).result();

		const thinkingBlocks = result.content.filter(b => b.type === "thinking");
		const textBlocks = result.content.filter(b => b.type === "text");

		expect(thinkingBlocks).toHaveLength(1);
		expect(textBlocks).toHaveLength(1);

		const thinking = thinkingBlocks[0];
		if (thinking.type === "thinking") {
			expect(thinking.thinking).toBe("The user wrote in Chinese: ...");
		}

		const text = textBlocks.map(b => (b.type === "text" ? b.text : "")).join("");
		expect(text).toBe("完成");
	});

	it("does not leak raw <think> tags as visible text", async () => {
		global.fetch = createMockFetch(thinkTagSseEvents(model.id));

		const result = await streamOpenAICompletions(model, baseContext(), { apiKey: "test-key" }).result();

		const allText = result.content
			.filter(b => b.type === "text")
			.map(b => (b.type === "text" ? b.text : ""))
			.join("");

		expect(allText).not.toContain("<think>");
		expect(allText).not.toContain("</think>");
	});
});
