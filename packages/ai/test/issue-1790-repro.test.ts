import { describe, expect, test } from "bun:test";
import { MODELS_DEV_PROVIDER_DESCRIPTORS, mapModelsDevToModels } from "../src/provider-models/openai-compat";

const minimaxM3ModelsDevPayload: Record<string, unknown> = {
	"minimax-coding-plan": {
		models: {
			"MiniMax-M3": {
				id: "MiniMax-M3",
				name: "MiniMax-M3",
				reasoning: true,
				tool_call: true,
				modalities: {
					input: ["text", "image", "video"],
					output: ["text"],
				},
				limit: {
					context: 512000,
					output: 128000,
				},
				cost: {
					input: 0,
					output: 0,
					cache_read: 0,
					cache_write: 0,
				},
			},
		},
	},
	"minimax-cn-coding-plan": {
		models: {
			"MiniMax-M3": {
				id: "MiniMax-M3",
				name: "MiniMax-M3",
				reasoning: true,
				tool_call: true,
				modalities: {
					input: ["text", "image", "video"],
					output: ["text"],
				},
				limit: {
					context: 512000,
					output: 128000,
				},
				cost: {
					input: 0,
					output: 0,
					cache_read: 0,
					cache_write: 0,
				},
			},
		},
	},
};

const cases: readonly [provider: "minimax-code" | "minimax-code-cn", baseUrl: string][] = [
	["minimax-code", "https://api.minimax.io/v1"],
	["minimax-code-cn", "https://api.minimaxi.com/v1"],
];

describe("MiniMax Coding Plan model mapping (issue #1790)", () => {
	test.each(cases)("maps MiniMax-M3 for %s", (provider, baseUrl) => {
		const models = mapModelsDevToModels(minimaxM3ModelsDevPayload, MODELS_DEV_PROVIDER_DESCRIPTORS);
		const model = models.find(candidate => candidate.provider === provider && candidate.id === "MiniMax-M3");

		expect(model).toEqual({
			id: "MiniMax-M3",
			name: "MiniMax-M3",
			api: "openai-completions",
			provider,
			baseUrl,
			reasoning: true,
			input: ["text", "image"],
			cost: {
				input: 0,
				output: 0,
				cacheRead: 0,
				cacheWrite: 0,
			},
			contextWindow: 512000,
			maxTokens: 128000,
			compat: {
				supportsStore: false,
				supportsDeveloperRole: false,
				supportsReasoningEffort: false,
				reasoningContentField: "reasoning_content",
			},
		});
	});
});
