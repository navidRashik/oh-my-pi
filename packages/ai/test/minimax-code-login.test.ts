import { describe, expect, it } from "bun:test";
import { hookFetch } from "@oh-my-pi/pi-utils";
import {
	loginMiniMaxCode,
	loginMiniMaxCodeCn,
	loginMiniMaxTokenPlan,
	loginMiniMaxTokenPlanCn,
} from "../src/utils/oauth/minimax-code";

describe("MiniMax Token Plan login", () => {
	it("opens the international platform and validates against the international API", async () => {
		const authUrls: string[] = [];
		const validationUrls: string[] = [];

		using _hook = hookFetch(input => {
			validationUrls.push(String(input));
			return new Response("{}", { status: 200, headers: { "Content-Type": "application/json" } });
		});

		const apiKey = await loginMiniMaxCode({
			onAuth: info => authUrls.push(info.url),
			onPrompt: async () => "  sk-intl  ",
		});

		expect(apiKey).toBe("sk-intl");
		expect(authUrls).toEqual(["https://platform.minimax.io/subscribe/token-plan"]);
		expect(validationUrls).toEqual(["https://api.minimax.io/v1/chat/completions"]);
	});

	it("opens the China platform and validates against the China API", async () => {
		const authUrls: string[] = [];
		const validationUrls: string[] = [];

		using _hook = hookFetch(input => {
			validationUrls.push(String(input));
			return new Response("{}", { status: 200, headers: { "Content-Type": "application/json" } });
		});

		const apiKey = await loginMiniMaxCodeCn({
			onAuth: info => authUrls.push(info.url),
			onPrompt: async () => "  sk-cn  ",
		});

		expect(apiKey).toBe("sk-cn");
		expect(authUrls).toEqual(["https://platform.minimaxi.com/subscribe/token-plan"]);
		expect(validationUrls).toEqual(["https://api.minimaxi.com/v1/chat/completions"]);
	});

	it("validates the international Anthropic-compatible Token Plan API", async () => {
		const validationUrls: string[] = [];
		let validationBody = "";

		using _hook = hookFetch((input, init) => {
			validationUrls.push(String(input));
			validationBody = String(init?.body ?? "");
			return new Response("{}", { status: 200, headers: { "Content-Type": "application/json" } });
		});

		const apiKey = await loginMiniMaxTokenPlan({
			onAuth: () => {},
			onPrompt: async () => "  sk-intl  ",
		});

		expect(apiKey).toBe("sk-intl");
		expect(validationUrls).toEqual(["https://api.minimax.io/anthropic/v1/messages"]);
		expect(JSON.parse(validationBody)).toMatchObject({ model: "MiniMax-M3", max_tokens: 1 });
	});

	it("validates the China Anthropic-compatible Token Plan API", async () => {
		const validationUrls: string[] = [];
		let validationBody = "";

		using _hook = hookFetch((input, init) => {
			validationUrls.push(String(input));
			validationBody = String(init?.body ?? "");
			return new Response("{}", { status: 200, headers: { "Content-Type": "application/json" } });
		});

		const apiKey = await loginMiniMaxTokenPlanCn({
			onAuth: () => {},
			onPrompt: async () => "  sk-cn  ",
		});

		expect(apiKey).toBe("sk-cn");
		expect(validationUrls).toEqual(["https://api.minimaxi.com/anthropic/v1/messages"]);
		expect(JSON.parse(validationBody)).toMatchObject({ model: "MiniMax-M3", max_tokens: 1 });
	});
});
