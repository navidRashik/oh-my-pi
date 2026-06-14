import { afterEach, beforeEach, describe, expect, it, vi } from "bun:test";
import type { AssistantMessage } from "@oh-my-pi/pi-ai";
import { resetSettingsForTest, Settings } from "@oh-my-pi/pi-coding-agent/config/settings";
import { EventController } from "@oh-my-pi/pi-coding-agent/modes/controllers/event-controller";
import type { InteractiveModeContext } from "@oh-my-pi/pi-coding-agent/modes/types";

function createAssistantMessage(): AssistantMessage {
	return {
		role: "assistant",
		content: [{ type: "text", text: "done" }],
		api: "anthropic-messages",
		provider: "anthropic",
		model: "claude-sonnet-4-5",
		usage: {
			input: 200,
			output: 10,
			cacheRead: 0,
			cacheWrite: 0,
			totalTokens: 210,
			cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
		},
		stopReason: "stop",
		timestamp: Date.now(),
	};
}

describe("EventController idle compaction teardown", () => {
	beforeEach(async () => {
		resetSettingsForTest();
		await Settings.init({
			inMemory: true,
			overrides: {
				"compaction.idleEnabled": true,
				"compaction.idleThresholdTokens": 100,
				"compaction.idleTimeoutSeconds": 60,
			},
		});
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.useRealTimers();
		vi.restoreAllMocks();
		resetSettingsForTest();
	});

	function makeContext(
		runIdleCompaction: () => void,
		hasPendingBackgroundJobs: () => boolean = () => false,
	): InteractiveModeContext {
		const context = {
			isInitialized: true,
			loadingAnimation: undefined,
			streamingComponent: undefined,
			streamingMessage: undefined,
			pendingTools: new Map<string, unknown>(),
			flushPendingModelSwitch: async () => {},
			ui: { requestRender: vi.fn() },
			chatContainer: { removeChild: vi.fn() },
			statusContainer: { clear: vi.fn() },
			statusLine: { invalidate: vi.fn() },
			updateEditorTopBorder: vi.fn(),
			editor: { getText: () => "" },
			sessionManager: { getSessionName: () => undefined },
			session: {
				isCompacting: false,
				isStreaming: false,
				isGeneratingHandoff: false,
				queuedMessageCount: 0,
				runIdleCompaction,
				hasPendingBackgroundJobs,
				getAsyncJobSnapshot: () => null,
				agent: { state: { messages: [createAssistantMessage()] } },
			},
			get viewSession() {
				return (this as typeof context).session;
			},
			clearTransientSessionUi: () => {},
		} as unknown as InteractiveModeContext;
		return context;
	}

	it("cancels scheduled idle compaction when disposed", async () => {
		const runIdleCompaction = vi.fn();
		const controller = new EventController(makeContext(runIdleCompaction));
		await controller.handleEvent({ type: "agent_end", messages: [createAssistantMessage()] });
		controller.dispose();
		vi.advanceTimersByTime(60_000);

		expect(runIdleCompaction).not.toHaveBeenCalled();
	});

	it("fires idle compaction after the delay when idle with no background jobs", async () => {
		const runIdleCompaction = vi.fn();
		const controller = new EventController(makeContext(runIdleCompaction, () => false));
		await controller.handleEvent({ type: "agent_end", messages: [createAssistantMessage()] });
		vi.advanceTimersByTime(60_000);

		expect(runIdleCompaction).toHaveBeenCalledTimes(1);
		controller.dispose();
	});

	it("does not schedule idle compaction while background jobs are running", async () => {
		const runIdleCompaction = vi.fn();
		const controller = new EventController(makeContext(runIdleCompaction, () => true));
		await controller.handleEvent({ type: "agent_end", messages: [createAssistantMessage()] });
		vi.advanceTimersByTime(60_000);

		expect(runIdleCompaction).not.toHaveBeenCalled();
		controller.dispose();
	});
});
