import { afterEach, describe, expect, it, vi } from "bun:test";
import * as kimiOauth from "@oh-my-pi/pi-ai/oauth/kimi";
import { streamOpenAICompletions } from "@oh-my-pi/pi-ai/providers/openai-completions";
import type { Context } from "@oh-my-pi/pi-ai/types";
import { buildModel } from "@oh-my-pi/pi-catalog/build";
import { Effort } from "@oh-my-pi/pi-catalog/effort";
import { kimiCodeModelManagerOptions } from "@oh-my-pi/pi-catalog/provider-models/openai-compat";
import type { FetchImpl, Model, ModelSpec } from "@oh-my-pi/pi-catalog/types";
import { applyGeneratedModelPolicies } from "../scripts/generated-policies";

const KIMI_HEADERS = Object.freeze({
	"User-Agent": "KimiCLI/test",
	"X-Msh-Platform": "kimi_cli",
	"X-Msh-Version": "test",
	"X-Msh-Device-Name": "test",
	"X-Msh-Device-Model": "test",
	"X-Msh-Os-Version": "test",
	"X-Msh-Device-Id": "test",
});

const context: Context = {
	messages: [{ role: "user", content: "hello", timestamp: Date.now() }],
};

afterEach(() => {
	vi.restoreAllMocks();
});

async function discoverKimiCodeK3(): Promise<Model<"openai-completions">> {
	const fetchMock: FetchImpl = async () =>
		new Response(
			JSON.stringify({
				data: [
					{
						id: "k3",
						display_name: "K3",
						supports_reasoning: true,
						supports_image_in: true,
						context_length: 1_048_576,
					},
				],
			}),
			{ status: 200, headers: { "content-type": "application/json" } },
		);
	const models = await kimiCodeModelManagerOptions({ apiKey: "test-key", fetch: fetchMock }).fetchDynamicModels?.();
	const k3 = models?.find(model => model.id === "k3");
	if (!k3) throw new Error("kimi-code/k3 was not discovered");
	return buildModel(k3);
}

describe("issue #5831 — kimi-code/k3 max-only effort", () => {
	it("corrects generated K3 metadata to the provider's max-only contract", () => {
		const k3: ModelSpec<"openai-completions"> = {
			id: "k3",
			name: "K3",
			api: "openai-completions",
			provider: "kimi-code",
			baseUrl: "https://api.kimi.com/coding/v1",
			reasoning: true,
			input: ["text", "image"],
			cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
			contextWindow: 1_048_576,
			maxTokens: 32_000,
			thinking: { mode: "effort", efforts: [Effort.Minimal, Effort.Low, Effort.Medium, Effort.High] },
			compat: { thinkingFormat: "zai" },
		};

		applyGeneratedModelPolicies([k3]);

		expect(k3.thinking).toEqual({ mode: "effort", efforts: [Effort.Max], requiresEffort: true });
		expect(k3.compat?.thinkingFormat).toBe("openai");
	});

	it("keeps dynamically discovered K3 metadata max-only", async () => {
		const k3 = await discoverKimiCodeK3();

		expect(k3.thinking).toEqual({ mode: "effort", efforts: [Effort.Max], requiresEffort: true });
		expect(k3.compat.thinkingFormat).toBe("openai");
		expect(k3.compat.reasoningDisableMode).toBe("lowest-effort");
	});

	it("sends selected max effort through the OpenAI reasoning dialect", async () => {
		vi.spyOn(kimiOauth, "getKimiCommonHeaders").mockReturnValue(KIMI_HEADERS);
		const k3 = await discoverKimiCodeK3();
		let requestBody: string | undefined;
		const fetchMock: FetchImpl = async (_input, init) => {
			requestBody = typeof init?.body === "string" ? init.body : undefined;
			return new Response('{"error":{"message":"captured"}}', { status: 400 });
		};

		const stream = streamOpenAICompletions(k3, context, {
			apiKey: "test-key",
			reasoning: Effort.Max,
			fetch: fetchMock,
		});
		for await (const _event of stream) {
			// Drain the response so the request body is captured.
		}
		if (!requestBody) throw new Error("K3 request body was not captured");
		const body = JSON.parse(requestBody) as { reasoning_effort?: string; thinking?: unknown };

		expect(body.reasoning_effort).toBe("max");
		expect(body.thinking).toBeUndefined();
	});
});
