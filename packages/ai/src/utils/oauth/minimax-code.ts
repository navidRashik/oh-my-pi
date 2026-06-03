/**
 * MiniMax Coding Plan login flow.
 *
 * MiniMax Token Plan is a subscription service that provides access to
 * MiniMax models through MiniMax's regional OpenAI- and Anthropic-compatible APIs.
 *
 * This is not OAuth - it's a simple API key flow:
 * 1. Open browser to the matching regional MiniMax subscription page
 * 2. User subscribes and copies their API key
 * 3. User pastes the API key back into the CLI
 *
 * International OpenAI-compatible: https://api.minimax.io/v1
 * China OpenAI-compatible: https://api.minimaxi.com/v1
 * International Anthropic-compatible: https://api.minimax.io/anthropic
 * China Anthropic-compatible: https://api.minimaxi.com/anthropic
 */

import { validateAnthropicCompatibleApiKey, validateOpenAICompatibleApiKey } from "./api-key-validation";
import type { OAuthController } from "./types";

const AUTH_URL_INTL = "https://platform.minimax.io/subscribe/token-plan";
const AUTH_URL_CN = "https://platform.minimaxi.com/subscribe/token-plan";
const OPENAI_API_BASE_URL_INTL = "https://api.minimax.io/v1";
const OPENAI_API_BASE_URL_CN = "https://api.minimaxi.com/v1";
const ANTHROPIC_API_BASE_URL_INTL = "https://api.minimax.io/anthropic";
const ANTHROPIC_API_BASE_URL_CN = "https://api.minimaxi.com/anthropic";
const OPENAI_VALIDATION_MODEL = "MiniMax-M2";
const ANTHROPIC_VALIDATION_MODEL = "MiniMax-M3";

/**
 * Login to MiniMax OpenAI-compatible Token Plan (international).
 *
 * Opens browser to subscription page, prompts user to paste their API key.
 * Returns the API key directly (not OAuthCredentials - this isn't OAuth).
 */
export async function loginMiniMaxCode(options: OAuthController): Promise<string> {
	return loginMiniMaxOpenAICompatiblePlan(options, AUTH_URL_INTL, OPENAI_API_BASE_URL_INTL, "MiniMax Token Plan");
}

async function promptMiniMaxApiKey(options: OAuthController, authUrl: string): Promise<string> {
	if (!options.onPrompt) {
		throw new Error("MiniMax Token Plan login requires onPrompt callback");
	}
	options.onAuth?.({
		url: authUrl,
		instructions: "Subscribe to Token Plan and copy your API key",
	});
	// Prompt user to paste their API key
	const apiKey = await options.onPrompt({
		message: "Paste your MiniMax Token Plan API key",
		placeholder: "sk-...",
	});
	if (options.signal?.aborted) {
		throw new Error("Login cancelled");
	}
	const trimmed = apiKey.trim();
	if (!trimmed) {
		throw new Error("API key is required");
	}
	return trimmed;
}

async function loginMiniMaxOpenAICompatiblePlan(
	options: OAuthController,
	authUrl: string,
	baseUrl: string,
	providerName: string,
): Promise<string> {
	const apiKey = await promptMiniMaxApiKey(options, authUrl);

	options.onProgress?.("Validating API key...");
	await validateOpenAICompatibleApiKey({
		provider: providerName,
		apiKey,
		baseUrl,
		model: OPENAI_VALIDATION_MODEL,
		signal: options.signal,
	});
	return apiKey;
}

async function loginMiniMaxAnthropicCompatiblePlan(
	options: OAuthController,
	authUrl: string,
	baseUrl: string,
	providerName: string,
): Promise<string> {
	const apiKey = await promptMiniMaxApiKey(options, authUrl);

	options.onProgress?.("Validating API key...");
	await validateAnthropicCompatibleApiKey({
		provider: providerName,
		apiKey,
		baseUrl,
		model: ANTHROPIC_VALIDATION_MODEL,
		signal: options.signal,
	});
	return apiKey;
}

/**
 * Login to MiniMax Coding Plan (China).
 *
 * Same flow as international but uses China endpoint.
 */
export async function loginMiniMaxCodeCn(options: OAuthController): Promise<string> {
	return loginMiniMaxOpenAICompatiblePlan(options, AUTH_URL_CN, OPENAI_API_BASE_URL_CN, "MiniMax Token Plan (China)");
}

/** Login to MiniMax Token Plan through the Anthropic-compatible international endpoint. */
export async function loginMiniMaxTokenPlan(options: OAuthController): Promise<string> {
	return loginMiniMaxAnthropicCompatiblePlan(
		options,
		AUTH_URL_INTL,
		ANTHROPIC_API_BASE_URL_INTL,
		"MiniMax Token Plan",
	);
}

/** Login to MiniMax Token Plan through the Anthropic-compatible China endpoint. */
export async function loginMiniMaxTokenPlanCn(options: OAuthController): Promise<string> {
	return loginMiniMaxAnthropicCompatiblePlan(
		options,
		AUTH_URL_CN,
		ANTHROPIC_API_BASE_URL_CN,
		"MiniMax Token Plan (China)",
	);
}
