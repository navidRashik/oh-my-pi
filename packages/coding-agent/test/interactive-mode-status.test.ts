import { beforeAll, describe, expect, test, vi } from "bun:test";
import { initTheme } from "@oh-my-pi/pi-coding-agent/modes/theme/theme";
import type { InteractiveModeContext } from "@oh-my-pi/pi-coding-agent/modes/types";
import { UiHelpers } from "@oh-my-pi/pi-coding-agent/modes/utils/ui-helpers";
import { buildSessionContext } from "@oh-my-pi/pi-coding-agent/session/session-manager";
import { Container } from "@oh-my-pi/pi-tui";

function renderLastLine(container: Container, width = 120): string {
	const last = container.children[container.children.length - 1];
	if (!last) return "";
	return last.render(width).join("\n");
}

describe("InteractiveMode.showStatus", () => {
	beforeAll(() => {
		// showStatus uses the global theme instance
		initTheme();
	});

	test("coalesces immediately-sequential status messages", () => {
		const ctx = {
			chatContainer: new Container(),
			ui: { requestRender: vi.fn() },
			isBackgrounded: false,
			lastStatusSpacer: undefined,
			lastStatusText: undefined,
		} as unknown as InteractiveModeContext;
		const helpers = new UiHelpers(ctx);

		helpers.showStatus("STATUS_ONE");
		expect(ctx.chatContainer.children).toHaveLength(2);
		expect(renderLastLine(ctx.chatContainer)).toContain("STATUS_ONE");

		helpers.showStatus("STATUS_TWO");
		// second status updates the previous line instead of appending
		expect(ctx.chatContainer.children).toHaveLength(2);
		expect(renderLastLine(ctx.chatContainer)).toContain("STATUS_TWO");
		expect(renderLastLine(ctx.chatContainer)).not.toContain("STATUS_ONE");
	});

	test("appends a new status line if something else was added in between", () => {
		const ctx = {
			chatContainer: new Container(),
			ui: { requestRender: vi.fn() },
			isBackgrounded: false,
			lastStatusSpacer: undefined,
			lastStatusText: undefined,
		} as unknown as InteractiveModeContext;
		const helpers = new UiHelpers(ctx);

		helpers.showStatus("STATUS_ONE");
		expect(ctx.chatContainer.children).toHaveLength(2);

		// Something else gets added to the chat in between status updates
		ctx.chatContainer.addChild({ render: () => ["OTHER"], invalidate: () => {} });
		expect(ctx.chatContainer.children).toHaveLength(3);

		helpers.showStatus("STATUS_TWO");
		// adds spacer + text
		expect(ctx.chatContainer.children).toHaveLength(5);
		expect(renderLastLine(ctx.chatContainer)).toContain("STATUS_TWO");
	});

	test("preserves optimistic user signatures when rebuilding transcript state", () => {
		const ctx = {
			chatContainer: new Container(),
			pendingTools: new Map(),
			ui: { requestRender: vi.fn() },
			optimisticUserMessageSignature: "hello\u00001",
		} as unknown as InteractiveModeContext;
		const helpers = new UiHelpers(ctx);

		helpers.renderSessionContext(buildSessionContext([]));

		// renderSessionContext must not clear the signature — the message_start
		// handler owns this lifecycle and uses it to guard against clearing the
		// user's in-progress editor draft during an optimistic send (#783).
		expect(ctx.optimisticUserMessageSignature).toBe("hello\u00001");
	});
});

describe("UiHelpers.renderSessionContextIncrementally", () => {
	beforeAll(() => {
		initTheme();
	});

	test("yields and requests renders between transcript chunks", async () => {
		let timerFired = false;
		const progress: number[] = [];
		const chatContainer = new Container();
		const requestRender = vi.fn();
		const ctx = {
			chatContainer,
			pendingTools: new Map(),
			ui: { requestRender },
			statusLine: { invalidate: vi.fn() },
			updateEditorBorderColor: vi.fn(),
			addMessageToChat: (message: { role: string }) => {
				chatContainer.addChild({ render: () => [message.role], invalidate: () => {} });
				return [];
			},
		} as unknown as InteractiveModeContext;
		const helpers = new UiHelpers(ctx);
		const messages = Array.from({ length: 5 }, (_, index) => ({
			role: "custom" as const,
			customType: "test",
			content: `message ${index}`,
			display: false,
			timestamp: index,
		}));

		setTimeout(() => {
			timerFired = true;
		}, 0);
		await helpers.renderSessionContextIncrementally(
			{ ...buildSessionContext([]), messages },
			{
				chunkSize: 2,
				onProgress: rendered => progress.push(rendered),
			},
		);

		expect(timerFired).toBe(true);
		expect(progress).toEqual([2, 4, 5]);
		expect(ctx.chatContainer.children).toHaveLength(5);
		expect(requestRender).toHaveBeenCalledTimes(3);
	});

	test("aborts before rendering the next chunk", async () => {
		const abortController = new AbortController();
		const chatContainer = new Container();
		const ctx = {
			chatContainer,
			pendingTools: new Map(),
			ui: { requestRender: vi.fn() },
			addMessageToChat: (message: { role: string }) => {
				chatContainer.addChild({ render: () => [message.role], invalidate: () => {} });
				return [];
			},
		} as unknown as InteractiveModeContext;
		const helpers = new UiHelpers(ctx);
		const messages = Array.from({ length: 5 }, (_, index) => ({
			role: "custom" as const,
			customType: "test",
			content: `message ${index}`,
			display: false,
			timestamp: index,
		}));

		await expect(
			helpers.renderSessionContextIncrementally(
				{ ...buildSessionContext([]), messages },
				{
					chunkSize: 2,
					signal: abortController.signal,
					onProgress: () => abortController.abort(),
				},
			),
		).rejects.toThrow("The operation was aborted.");
		expect(ctx.chatContainer.children).toHaveLength(2);
	});
});
