import { describe, expect, it } from "bun:test";
import modelsJson from "../src/models.json";

const FIRST_PARTY_MINIMAX_PROVIDERS: Array<"minimax" | "minimax-cn"> = ["minimax", "minimax-cn"];

describe("minimax bundled catalog", () => {
	it("pins first-party MiniMax-M3 entries to 1M context", () => {
		for (const provider of FIRST_PARTY_MINIMAX_PROVIDERS) {
			const model = modelsJson[provider]["MiniMax-M3"];

			expect(model.provider).toBe(provider);
			expect(model.contextWindow).toBe(1_000_000);
			expect(model.maxTokens).toBe(128_000);
		}
	});
});
