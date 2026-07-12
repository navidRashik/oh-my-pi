import { afterEach, describe, expect, it, vi } from "bun:test";
import type { AuthStorage, FetchImpl } from "@oh-my-pi/pi-ai";
import type { SearchParams } from "@oh-my-pi/pi-coding-agent/web/search/providers/base";
import { searchCodex } from "@oh-my-pi/pi-coding-agent/web/search/providers/codex";

type CapturedRequest = {
	url: string;
	headers: RequestInit["headers"];
	body: Record<string, unknown> | null;
};

const originalCodexSearchModel = process.env.PI_CODEX_WEB_SEARCH_MODEL;

function makeSseResponse(model: string): string {
	return [
		`data: ${JSON.stringify({
			type: "response.output_item.done",
			item: {
				type: "message",
				content: [
					{
						type: "output_text",
						text: "Codex answer",
						annotations: [{ type: "url_citation", url: "https://example.com/article", title: "Example Article" }],
					},
				],
			},
		})}`,
		"",
		`data: ${JSON.stringify({
			type: "response.completed",
			response: {
				id: "resp_codex_test",
				model,
				usage: {
					input_tokens: 12,
					output_tokens: 7,
					total_tokens: 19,
				},
			},
		})}`,
		"",
	].join("\n");
}

function makeImagePlaceholderSseResponse(model: string): string {
	return [
		`data: ${JSON.stringify({
			type: "response.output_text.delta",
			delta: "OpenAI Responses API defaults `store` to false unless you opt in.",
		})}`,
		"",
		`data: ${JSON.stringify({
			type: "response.output_item.done",
			item: {
				type: "message",
				content: [
					{
						type: "output_text",
						text: "(see attached image)",
						annotations: [
							{ type: "url_citation", url: "https://platform.openai.com/docs/api-reference/responses" },
						],
					},
				],
			},
		})}`,
		"",
		`data: ${JSON.stringify({
			type: "response.completed",
			response: {
				id: "resp_codex_placeholder_test",
				model,
			},
		})}`,
		"",
	].join("\n");
}

function makeMarkdownLinkSseResponse(model: string): string {
	return [
		`data: ${JSON.stringify({
			type: "response.output_item.done",
			item: {
				type: "message",
				content: [
					{
						type: "output_text",
						text: "See [Example Article](https://example.com/article) for details.",
						annotations: [],
					},
				],
			},
		})}`,
		"",
		`data: ${JSON.stringify({
			type: "response.completed",
			response: { id: "resp_codex_markdown_test", model },
		})}`,
		"",
	].join("\n");
}

function makePlainUrlSseResponse(model: string): string {
	return [
		`data: ${JSON.stringify({
			type: "response.output_item.done",
			item: {
				type: "message",
				content: [
					{
						type: "output_text",
						text: "Sources:\n- https://example.com/article\n- https://example.com/faq",
						annotations: [],
					},
				],
			},
		})}`,
		"",
		`data: ${JSON.stringify({
			type: "response.completed",
			response: { id: "resp_codex_plain_url_test", model },
		})}`,
		"",
	].join("\n");
}

function makeMarkdownParenthesesSseResponse(model: string): string {
	return [
		`data: ${JSON.stringify({
			type: "response.output_item.done",
			item: {
				type: "message",
				content: [
					{
						type: "output_text",
						text: "See [Function](https://en.wikipedia.org/wiki/Function_(mathematics)) for details.",
						annotations: [],
					},
				],
			},
		})}`,
		"",
		`data: ${JSON.stringify({
			type: "response.completed",
			response: { id: "resp_codex_markdown_parentheses_test", model },
		})}`,
		"",
	].join("\n");
}

function makePlainUrlPunctuationSseResponse(model: string): string {
	return [
		`data: ${JSON.stringify({
			type: "response.output_item.done",
			item: {
				type: "message",
				content: [
					{
						type: "output_text",
						text: "Read https://example.com/article. Then compare https://example.com/faq), and keep https://en.wikipedia.org/wiki/Function_(mathematics).",
						annotations: [],
					},
				],
			},
		})}`,
		"",
		`data: ${JSON.stringify({
			type: "response.completed",
			response: { id: "resp_codex_plain_url_punctuation_test", model },
		})}`,
		"",
	].join("\n");
}

describe("searchCodex model selection", () => {
	const fakeAuthStorage = {
		async getOAuthAccess() {
			return {
				accessToken: "test-access-token",
				accountId: "acct-test",
			};
		},
		hasOAuth() {
			return true;
		},
	} as unknown as AuthStorage;
	let capturedRequest: CapturedRequest | null = null;
	let capturedRequests: CapturedRequest[] = [];

	function makeSearchParams(query: string, fetch?: FetchImpl): SearchParams {
		return {
			query,
			systemPrompt: "Codex test system prompt",
			authStorage: fakeAuthStorage,
			...(fetch ? { fetch } : {}),
		};
	}

	function captureCodexRequest(url: string | URL | Request, init: RequestInit | undefined): CapturedRequest {
		const request = {
			url: typeof url === "string" ? url : url.toString(),
			headers: init?.headers,
			body: init?.body ? (JSON.parse(init.body as string) as Record<string, unknown>) : null,
		};
		capturedRequest = request;
		capturedRequests.push(request);
		return request;
	}

	function mockCodexFetch(responseModel: string, responseBody?: string): FetchImpl {
		capturedRequest = null;
		capturedRequests = [];
		return (url, init) => {
			captureCodexRequest(url, init);
			return Promise.resolve(
				new Response(responseBody ?? makeSseResponse(responseModel), {
					status: 200,
					headers: { "Content-Type": "text/event-stream" },
				}),
			);
		};
	}

	afterEach(() => {
		vi.restoreAllMocks();
		capturedRequest = null;
		capturedRequests = [];
		if (originalCodexSearchModel === undefined) {
			delete process.env.PI_CODEX_WEB_SEARCH_MODEL;
		} else {
			process.env.PI_CODEX_WEB_SEARCH_MODEL = originalCodexSearchModel;
		}
	});

	it("starts the built-in default model preference with gpt-5.6-luna", async () => {
		delete process.env.PI_CODEX_WEB_SEARCH_MODEL;
		const result = await searchCodex(makeSearchParams("default codex model", mockCodexFetch("gpt-5.6-luna")));

		expect(capturedRequest).not.toBeNull();
		expect(capturedRequest?.url).toBe("https://chatgpt.com/backend-api/codex/responses");
		expect(capturedRequest?.body?.model).toBe("gpt-5.6-luna");
		expect(result.model).toBe("gpt-5.6-luna");
		expect(result.sources).toEqual([{ title: "Example Article", url: "https://example.com/article" }]);
	});

	it("falls back to the default model when PI_CODEX_WEB_SEARCH_MODEL is blank", async () => {
		process.env.PI_CODEX_WEB_SEARCH_MODEL = "   ";
		const result = await searchCodex(makeSearchParams("blank codex model", mockCodexFetch("gpt-5.6-luna")));

		expect(capturedRequest).not.toBeNull();
		expect(capturedRequest?.body?.model).toBe("gpt-5.6-luna");
		expect(result.model).toBe("gpt-5.6-luna");
	});

	it("retries the next bundled default when Codex rejects a model for ChatGPT accounts", async () => {
		delete process.env.PI_CODEX_WEB_SEARCH_MODEL;
		let calls = 0;
		capturedRequest = null;
		capturedRequests = [];
		const fetchMock: FetchImpl = (url, init) => {
			calls += 1;
			const request = captureCodexRequest(url, init);

			const requestedModel = request.body?.model;

			if (calls === 1) {
				expect(requestedModel).toBe("gpt-5.6-luna");
				return Promise.resolve(
					new Response(
						JSON.stringify({
							detail: "The 'gpt-5.6-luna' model is not supported when using Codex with a ChatGPT account.",
						}),
						{ status: 400, headers: { "Content-Type": "application/json" } },
					),
				);
			}

			expect(requestedModel).toBe("gpt-5.6-terra");
			return Promise.resolve(
				new Response(makeSseResponse("gpt-5.6-terra"), {
					status: 200,
					headers: { "Content-Type": "text/event-stream" },
				}),
			);
		};

		const result = await searchCodex(makeSearchParams("retry unsupported default", fetchMock));

		expect(calls).toBe(2);
		expect(capturedRequests.map(request => request.body?.model)).toEqual(["gpt-5.6-luna", "gpt-5.6-terra"]);
		expect(result.model).toBe("gpt-5.6-terra");
		expect(result.sources).toEqual([{ title: "Example Article", url: "https://example.com/article" }]);
	});

	it("encodes explicit gpt-5.6-luna as a Responses-Lite Codex request", async () => {
		process.env.PI_CODEX_WEB_SEARCH_MODEL = "gpt-5.6-luna";
		const result = await searchCodex(makeSearchParams("lite codex model", mockCodexFetch("gpt-5.6-luna")));

		expect(capturedRequest).not.toBeNull();
		const headers = new Headers(capturedRequest?.headers);
		expect(headers.get("x-openai-internal-codex-responses-lite")).toBe("true");
		expect(headers.get("session-id")).toBeTruthy();
		expect(headers.get("thread-id")).toBeTruthy();
		expect(headers.get("x-codex-window-id")).toBeTruthy();
		const turnMetadata = JSON.parse(headers.get("x-codex-turn-metadata") ?? "{}") as Record<string, unknown>;
		expect(turnMetadata.request_kind).toBe("turn");
		expect(turnMetadata.session_id).toBe(headers.get("session-id"));
		expect(turnMetadata.thread_id).toBe(headers.get("thread-id"));
		expect(turnMetadata.window_id).toBe(headers.get("x-codex-window-id"));

		const body = capturedRequest?.body;
		expect(body?.model).toBe("gpt-5.6-luna");
		expect(body?.tools).toBeUndefined();
		expect(body?.tool_choice).toBeUndefined();
		expect(body?.instructions).toBeUndefined();
		expect(body?.reasoning).toEqual({ context: "all_turns" });
		expect(body?.parallel_tool_calls).toBe(false);
		expect(body?.input).toEqual([
			{
				type: "additional_tools",
				role: "developer",
				tools: [{ type: "web_search", search_context_size: "high" }],
			},
			{
				type: "message",
				role: "developer",
				content: [{ type: "input_text", text: "Codex test system prompt" }],
			},
			{
				type: "message",
				role: "user",
				content: [{ type: "input_text", text: "lite codex model" }],
			},
		]);
		expect(body?.client_metadata).toEqual(
			expect.objectContaining({
				session_id: headers.get("session-id"),
				thread_id: headers.get("thread-id"),
				"x-codex-window-id": headers.get("x-codex-window-id"),
				"x-codex-turn-metadata": headers.get("x-codex-turn-metadata"),
			}),
		);
		expect(result.model).toBe("gpt-5.6-luna");
	});

	it("applies a supported reasoning effort suffix to Responses-Lite requests", async () => {
		process.env.PI_CODEX_WEB_SEARCH_MODEL = "gpt-5.6-luna:max";
		await searchCodex(makeSearchParams("lite codex effort", mockCodexFetch("gpt-5.6-luna")));

		expect(capturedRequest?.body?.reasoning).toEqual({ effort: "max", context: "all_turns" });
	});

	it("rejects unsupported reasoning effort suffixes before calling Codex", async () => {
		process.env.PI_CODEX_WEB_SEARCH_MODEL = "gpt-5.6-luna:minimal";
		let calls = 0;
		const fetchMock: FetchImpl = () => {
			calls += 1;
			return Promise.resolve(new Response(makeSseResponse("gpt-5.6-luna")));
		};

		await expect(searchCodex(makeSearchParams("unsupported lite effort", fetchMock))).rejects.toThrow(
			/Thinking effort minimal is not supported/,
		);
		expect(calls).toBe(0);
	});

	it("does not retry default candidates when PI_CODEX_WEB_SEARCH_MODEL is explicitly unsupported", async () => {
		process.env.PI_CODEX_WEB_SEARCH_MODEL = "gpt-5.6-luna";
		let calls = 0;
		capturedRequest = null;
		capturedRequests = [];
		const fetchMock: FetchImpl = (url, init) => {
			calls += 1;
			const request = captureCodexRequest(url, init);

			expect(request.body?.model).toBe("gpt-5.6-luna");
			return Promise.resolve(
				new Response(
					JSON.stringify({
						detail: "The 'gpt-5.6-luna' model is not supported when using Codex with a ChatGPT account.",
					}),
					{ status: 400, headers: { "Content-Type": "application/json" } },
				),
			);
		};

		await expect(searchCodex(makeSearchParams("explicit unsupported model", fetchMock))).rejects.toThrow(
			"gpt-5.6-luna",
		);
		expect(calls).toBe(1);
		expect(capturedRequests.map(request => request.body?.model)).toEqual(["gpt-5.6-luna"]);
	});

	it("keeps classic gpt-5.4 request shape while extracting markdown link citations", async () => {
		process.env.PI_CODEX_WEB_SEARCH_MODEL = "gpt-5.4";
		const result = await searchCodex(
			makeSearchParams("markdown citations", mockCodexFetch("gpt-5.4", makeMarkdownLinkSseResponse("gpt-5.4"))),
		);

		expect(capturedRequest).not.toBeNull();
		const headers = new Headers(capturedRequest?.headers);
		expect(headers.get("x-openai-internal-codex-responses-lite")).toBeNull();
		expect(capturedRequest?.body?.instructions).toBe("Codex test system prompt");
		expect(capturedRequest?.body?.tools).toEqual([{ type: "web_search", search_context_size: "high" }]);
		expect(capturedRequest?.body?.tool_choice).toEqual({ type: "web_search" });
		expect(capturedRequest?.body?.input).toEqual([
			{
				type: "message",
				role: "user",
				content: [{ type: "input_text", text: "markdown citations" }],
			},
		]);
		expect(result.sources).toEqual([{ title: "Example Article", url: "https://example.com/article" }]);
	});

	it("extracts plain text URLs when annotations are absent", async () => {
		process.env.PI_CODEX_WEB_SEARCH_MODEL = "gpt-5.4";
		const result = await searchCodex(
			makeSearchParams("plain url citations", mockCodexFetch("gpt-5.4", makePlainUrlSseResponse("gpt-5.4"))),
		);

		expect(result.sources).toEqual([
			{ title: "https://example.com/article", url: "https://example.com/article" },
			{ title: "https://example.com/faq", url: "https://example.com/faq" },
		]);
	});

	it("preserves markdown URLs that contain balanced parentheses", async () => {
		process.env.PI_CODEX_WEB_SEARCH_MODEL = "gpt-5.4";
		const result = await searchCodex(
			makeSearchParams(
				"markdown parentheses citations",
				mockCodexFetch("gpt-5.4", makeMarkdownParenthesesSseResponse("gpt-5.4")),
			),
		);

		expect(result.sources).toEqual([
			{ title: "Function", url: "https://en.wikipedia.org/wiki/Function_(mathematics)" },
		]);
	});

	it("strips trailing prose punctuation from plain text URLs", async () => {
		process.env.PI_CODEX_WEB_SEARCH_MODEL = "gpt-5.4";
		const result = await searchCodex(
			makeSearchParams(
				"plain url punctuation",
				mockCodexFetch("gpt-5.4", makePlainUrlPunctuationSseResponse("gpt-5.4")),
			),
		);

		expect(result.sources).toEqual([
			{ title: "https://example.com/article", url: "https://example.com/article" },
			{ title: "https://example.com/faq", url: "https://example.com/faq" },
			{
				title: "https://en.wikipedia.org/wiki/Function_(mathematics)",
				url: "https://en.wikipedia.org/wiki/Function_(mathematics)",
			},
		]);
	});

	it("prefers streamed text when the final item only contains an image placeholder", async () => {
		const fetchMock: FetchImpl = () =>
			Promise.resolve(
				new Response(makeImagePlaceholderSseResponse("gpt-5.4-mini"), {
					status: 200,
					headers: { "Content-Type": "text/event-stream" },
				}),
			);

		const result = await searchCodex(makeSearchParams("responses api store semantics", fetchMock));

		expect(result.answer).toBe("OpenAI Responses API defaults `store` to false unless you opt in.");
		expect(result.sources).toEqual([
			{
				title: "https://platform.openai.com/docs/api-reference/responses",
				url: "https://platform.openai.com/docs/api-reference/responses",
			},
		]);
	});

	it("throws to advance the chain when both streamed and final answers are image placeholders without sources", async () => {
		const sse = [
			`data: ${JSON.stringify({
				type: "response.output_text.delta",
				delta: "[Attached image]",
			})}`,
			"",
			`data: ${JSON.stringify({
				type: "response.output_item.done",
				item: {
					type: "message",
					content: [{ type: "output_text", text: "See image above.", annotations: [] }],
				},
			})}`,
			"",
			`data: ${JSON.stringify({
				type: "response.completed",
				response: { id: "resp_codex_placeholder_only", model: "gpt-5.5" },
			})}`,
			"",
		].join("\n");

		const fetchMock: FetchImpl = () =>
			Promise.resolve(new Response(sse, { status: 200, headers: { "Content-Type": "text/event-stream" } }));

		await expect(searchCodex(makeSearchParams("image only", fetchMock))).rejects.toThrow(/image-only response/);
	});

	it("drops placeholder prose from the answer but keeps annotation sources when both are placeholders", async () => {
		const sse = [
			`data: ${JSON.stringify({
				type: "response.output_text.delta",
				delta: "(see attached image)",
			})}`,
			"",
			`data: ${JSON.stringify({
				type: "response.output_item.done",
				item: {
					type: "message",
					content: [
						{
							type: "output_text",
							text: "(See attached image.)",
							annotations: [{ type: "url_citation", url: "https://example.com/docs", title: "Docs" }],
						},
					],
				},
			})}`,
			"",
			`data: ${JSON.stringify({
				type: "response.completed",
				response: { id: "resp_codex_placeholder_with_sources", model: "gpt-5.5" },
			})}`,
			"",
		].join("\n");

		const fetchMock: FetchImpl = () =>
			Promise.resolve(new Response(sse, { status: 200, headers: { "Content-Type": "text/event-stream" } }));

		const result = await searchCodex(makeSearchParams("image with sources", fetchMock));
		expect(result.answer).toBeUndefined();
		expect(result.sources).toEqual([{ title: "Docs", url: "https://example.com/docs" }]);
	});
});
