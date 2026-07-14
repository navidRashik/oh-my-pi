import { describe, expect, test } from "bun:test";
import { serializeConversation } from "@oh-my-pi/pi-agent-core/compaction";
import type { AssistantMessage, Message, ToolResultMessage, Usage } from "@oh-my-pi/pi-ai";

const ZERO_USAGE: Usage = {
	input: 0,
	output: 0,
	cacheRead: 0,
	cacheWrite: 0,
	totalTokens: 0,
	cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
};

function assistantMessage(content: AssistantMessage["content"]): Message {
	return {
		role: "assistant",
		content,
		api: "mock",
		provider: "mock",
		model: "mock",
		usage: ZERO_USAGE,
		stopReason: "stop",
		timestamp: 0,
	};
}

function toolResultMessage(toolCallId: string, text: string, extra: Partial<ToolResultMessage> = {}): Message {
	return {
		role: "toolResult",
		toolCallId,
		toolName: "search",
		content: [{ type: "text", text }],
		isError: false,
		timestamp: 0,
		...extra,
	};
}

describe("serializeConversation — useless pairs", () => {
	test("skips a useless-flagged tool call/result pair but keeps its sibling", () => {
		const out = serializeConversation([
			assistantMessage([
				{ type: "toolCall", id: "c-keep", name: "search", arguments: { pattern: "alpha" } },
				{ type: "toolCall", id: "c-drop", name: "search", arguments: { pattern: "zzz_nothing" } },
			]),
			toolResultMessage("c-keep", "alpha match found in src/alpha.ts"),
			toolResultMessage("c-drop", "No matches found", { useless: true }),
		]);

		expect(out).toContain('search(pattern="alpha")');
		expect(out).toContain("alpha match found in src/alpha.ts");
		expect(out).not.toContain("zzz_nothing");
		expect(out).not.toContain("No matches found");
	});

	test("error results stay serialized even when flagged useless", () => {
		const out = serializeConversation([
			assistantMessage([{ type: "toolCall", id: "c-err", name: "search", arguments: { pattern: "beta" } }]),
			toolResultMessage("c-err", "grep crashed", { useless: true, isError: true }),
		]);

		expect(out).toContain('search(pattern="beta")');
		expect(out).toContain("[Tool Result]: grep crashed");
	});

	test("renders native dialect transcripts when a dialect is provided", () => {
		const out = serializeConversation(
			[
				assistantMessage([
					{ type: "text", text: "Searching." },
					{ type: "toolCall", id: "c-native", name: "search", arguments: { pattern: "gamma" } },
				]),
				toolResultMessage("c-native", "gamma match found"),
			],
			"anthropic",
		);

		expect(out).toContain("\n\nAssistant:");
		expect(out).toContain("<function_calls>");
		expect(out).toContain("<function_results>");
		expect(out).not.toContain("[Tool Call]:");
		expect(out).not.toContain("[Assistant tool calls]:");
	});

	test("native dialect serialization drops empty assistants left by useless calls", () => {
		const out = serializeConversation(
			[
				assistantMessage([
					{ type: "toolCall", id: "c-drop", name: "search", arguments: { pattern: "zzz_nothing" } },
				]),
				toolResultMessage("c-drop", "No matches found", { useless: true }),
			],
			"harmony",
		);

		expect(out).toBe("");
	});

	// Harmony/Gemma renderTranscript emits chat-template control tokens
	// (`<|channel|>analysis`, `<|start|>`) that GPT-5.6 rejects with
	// `Request blocked` when the summary payload is re-sent as prompt text
	// (issues #5184, #5337). Those dialects must fall back to the plain-text
	// form while still carrying the reasoning content forward.
	test("harmony summaries strip control tokens but keep thinking content", () => {
		const out = serializeConversation(
			[
				assistantMessage([
					{ type: "thinking", thinking: "Planning broad tool discovery." },
					{ type: "text", text: "Let me search." },
				]),
			],
			"harmony",
		);

		expect(out).not.toContain("<|channel|>");
		expect(out).not.toContain("<|start|>");
		expect(out).toContain("[Think]: Planning broad tool discovery.");
		expect(out).toContain("[Assistant]: Let me search.");
	});

	test("gemma summaries strip control tokens but keep thinking content", () => {
		const out = serializeConversation(
			[
				assistantMessage([
					{ type: "thinking", thinking: "Reasoning step." },
					{ type: "text", text: "Answer." },
				]),
			],
			"gemma",
		);

		expect(out).not.toContain("<start_of_turn>");
		expect(out).not.toContain("<|channel");
		expect(out).toContain("[Think]: Reasoning step.");
		expect(out).toContain("[Assistant]: Answer.");
	});
});
