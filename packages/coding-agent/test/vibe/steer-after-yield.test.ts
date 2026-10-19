/**
 * Contract: a `vibe_send` steer that the worker consumes only after its turn's
 * `yield` (the steer was still queued when the model chose to yield) is part of
 * that turn. The turn waits for the steered continuation, carries its answer in
 * the turn result, and leaves the worker resumable instead of reporting it dead.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "bun:test";
import { AsyncJobManager } from "@oh-my-pi/pi-coding-agent/async/job-manager";
import { Settings } from "@oh-my-pi/pi-coding-agent/config/settings";
import { AgentRegistry } from "@oh-my-pi/pi-coding-agent/registry/agent-registry";
import type { AgentSession } from "@oh-my-pi/pi-coding-agent/session/agent-session";
import type { ExecutorOptions } from "@oh-my-pi/pi-coding-agent/task/executor";
import * as executorModule from "@oh-my-pi/pi-coding-agent/task/executor";
import type { ToolSession } from "@oh-my-pi/pi-coding-agent/tools";
import { steeredContinuationAnswer, VibeSessionRegistry } from "@oh-my-pi/pi-coding-agent/vibe/runtime";
import type { SingleResult } from "@oh-my-pi/pi-tui/tools/task";

const OWNER = "vibe-parent";

function resultFor(options: ExecutorOptions, output: string): SingleResult {
	return {
		index: options.index,
		id: options.id,
		agent: options.agent.name,
		agentSource: options.agent.source,
		task: options.task,
		exitCode: 0,
		output,
		stderr: "",
		truncated: false,
		durationMs: 1,
		tokens: 0,
		requests: 0,
	};
}

const user = (text: string) => ({ role: "user", content: [{ type: "text", text }] });
const yieldCall = (data: unknown) => ({
	role: "assistant",
	content: [{ type: "toolCall", id: "y", name: "yield", arguments: { data } }],
});

describe("vibe steer consumed after the turn's yield", () => {
	let manager: AsyncJobManager;
	let parent: ToolSession;

	beforeEach(() => {
		manager = new AsyncJobManager({ onJobComplete: () => {} });
		parent = {
			cwd: "/tmp",
			settings: Settings.isolated(),
			asyncJobManager: manager,
			getAgentId: () => OWNER,
			getSessionId: () => "vibe-parent-session",
			getSessionFile: () => null,
			getArtifactsDir: () => null,
			taskDepth: 0,
			enableLsp: false,
		} as unknown as ToolSession;
	});

	afterEach(async () => {
		vi.restoreAllMocks();
		await manager.dispose({ timeoutMs: 100 });
		VibeSessionRegistry.resetGlobalForTests();
		AgentRegistry.resetGlobalForTests();
	});

	it("delivers the continuation's answer and keeps the worker alive", async () => {
		const release = Promise.withResolvers<void>();
		vi.spyOn(executorModule, "runSubprocess").mockImplementation(async options => {
			await release.promise;
			return resultFor(options, "first answer");
		});
		const registry = VibeSessionRegistry.global();
		const spawned = await registry.spawn(parent, { cli: "fast", prompt: "initial work" });

		// The continuation the steer started finishes while the turn waits on it.
		const live = {
			isStreaming: true,
			steer: vi.fn(async () => {}),
			waitForIdle: vi.fn(async () => {
				live.messages.push(yieldCall({ outcome: "late instruction handled" }));
				live.isStreaming = false;
			}),
			messages: [] as unknown[],
		};
		AgentRegistry.global().register({
			id: spawned.id,
			displayName: spawned.id,
			kind: "sub",
			parentId: OWNER,
			session: live as unknown as AgentSession,
			status: "running",
		});
		expect((await registry.send(parent, { session: spawned.id, message: "late instruction" })).mode).toBe("steered");

		// The driven turn yields; the queued steer then enters the transcript and
		// the worker keeps streaming a continuation the driven turn did not observe.
		live.messages.push(user("initial work"), yieldCall("first answer"), user("late instruction"));
		release.resolve();

		const job = manager.getJob(spawned.jobId);
		await job?.promise;
		expect(live.waitForIdle).toHaveBeenCalled();
		expect(job?.status).toBe("completed");
		expect(job?.resultText).toContain("first answer");
		expect(job?.resultText).toContain("late instruction handled");

		vi.spyOn(executorModule, "runSubagentFollowUpTurn").mockImplementation(async options =>
			resultFor({ ...options, cwd: "/tmp", task: options.message } as ExecutorOptions, "next answer"),
		);
		const next = await registry.send(parent, { session: spawned.id, message: "next task" });
		expect(next.mode).toBe("turn");
	});
});

describe("steeredContinuationAnswer", () => {
	it("is undefined when the steer was consumed before the turn's yield", () => {
		const messages = [user("do it"), user("steer"), yieldCall("final")];
		expect(steeredContinuationAnswer(messages, "do it", "steer")).toBeUndefined();
	});

	it("ignores yields from earlier turns", () => {
		const messages = [user("old"), yieldCall("old answer"), user("do it"), user("steer"), yieldCall("final")];
		expect(steeredContinuationAnswer(messages, "do it", "steer")).toBeUndefined();
	});

	it("returns the continuation's yield payload when the steer followed the yield", () => {
		const messages = [user("do it"), yieldCall("final"), user("steer"), yieldCall({ done: 1 })];
		expect(steeredContinuationAnswer(messages, "do it", "steer")).toBe('{"done":1}');
	});
});
