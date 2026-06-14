/**
 * Contract: the anchored "Background Jobs" panel lists the main session's
 * running async task/shell jobs above the editor, prefers live task progress
 * over spawn labels, and hides while the UI is focused on a subagent session.
 */
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "bun:test";
import {
	type BackgroundJobRow,
	EventController,
	renderBackgroundJobsLines,
} from "@oh-my-pi/pi-coding-agent/modes/controllers/event-controller";
import { initTheme } from "@oh-my-pi/pi-coding-agent/modes/theme/theme";
import type { InteractiveModeContext } from "@oh-my-pi/pi-coding-agent/modes/types";
import type { AsyncJobSnapshotItem } from "@oh-my-pi/pi-coding-agent/session/agent-session";

beforeAll(async () => {
	await initTheme();
});

const NO_SETTLED = { completed: 0, failed: 0, cancelled: 0 };

interface CapturedText {
	render(width: number): readonly string[];
}

function makeContext(opts: {
	running?: AsyncJobSnapshotItem[];
	recent?: AsyncJobSnapshotItem[];
	focusedAgentId?: string;
	describe?: (id: string) => string | undefined;
}) {
	const addChild = vi.fn();
	const removeChild = vi.fn();
	const state = { focusedAgentId: opts.focusedAgentId };
	const ctx = {
		subagentContainer: { addChild, removeChild, clear: vi.fn() },
		ui: { requestRender: vi.fn(), terminal: { columns: 200 } },
		settings: { get: () => false },
		describeSubagentJob: opts.describe ?? (() => undefined),
		get focusedAgentId() {
			return state.focusedAgentId;
		},
		session: {
			getAsyncJobSnapshot: () => ({
				running: opts.running ?? [],
				recent: opts.recent ?? [],
				delivery: { queued: 0, delivering: false, pendingJobIds: [] },
			}),
		},
	} as unknown as InteractiveModeContext;
	return { ctx, addChild, removeChild, state };
}

describe("renderBackgroundJobsLines", () => {
	it("returns no lines when nothing is running so the container clears", () => {
		expect(renderBackgroundJobsLines([], NO_SETTLED, 120)).toEqual([]);
	});

	it("renders the running-count header and a [task] Id: summary - age row", () => {
		const jobs: BackgroundJobRow[] = [
			{ type: "task", id: "SomeTask", summary: "summarized current action", ageMs: 83_000 },
		];
		const out = Bun.stripANSI(renderBackgroundJobsLines(jobs, NO_SETTLED, 120).join("\n"));
		expect(out).toContain("Background Jobs (1 running):");
		expect(out).toContain("[task] SomeTask: summarized current action - 1m23s");
	});

	it("renders shell jobs as [shell] cmd - age with no id or colon", () => {
		const jobs: BackgroundJobRow[] = [{ type: "bash", id: "", summary: "pnpm test --filter x", ageMs: 5_000 }];
		const out = Bun.stripANSI(renderBackgroundJobsLines(jobs, NO_SETTLED, 120).join("\n"));
		expect(out).toContain("[shell] pnpm test --filter x - 5s");
		expect(out).not.toContain("[shell] :");
	});

	it("appends settled counts to the header", () => {
		const jobs: BackgroundJobRow[] = [
			{ type: "task", id: "SomeTask", summary: "summarized current action", ageMs: 83_000 },
		];
		const out = Bun.stripANSI(
			renderBackgroundJobsLines(jobs, { completed: 2, failed: 1, cancelled: 0 }, 120).join("\n"),
		);
		expect(out).toContain("Background Jobs (1 running, 2 completed, 1 failed):");
	});

	it("truncates a long summary to fit the terminal width", () => {
		const jobs: BackgroundJobRow[] = [{ type: "task", id: "SomeTask", summary: "x".repeat(400), ageMs: 83_000 }];
		const out = Bun.stripANSI(renderBackgroundJobsLines(jobs, NO_SETTLED, 120).join("\n"));
		const row = out.split("\n").find(line => line.includes("[task]")) ?? "";
		expect(row.length).toBeLessThanOrEqual(120);
		expect(row).toContain("…");
		expect(row).toContain("- 1m23s");
	});
});

describe("EventController background-jobs panel", () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => vi.useRealTimers());

	it("renders running jobs into the anchored subagent container", () => {
		const running: AsyncJobSnapshotItem[] = [
			{
				id: "CoreFixes",
				type: "task",
				status: "running",
				label: "core auth hardening fixes",
				startTime: Date.now() - 5_000,
			},
		];
		const { ctx, addChild } = makeContext({ running });
		new EventController(ctx).refreshBackgroundJobs();

		expect(addChild).toHaveBeenCalledTimes(1);
		const out = Bun.stripANSI((addChild.mock.calls[0][0] as CapturedText).render(200).join("\n"));
		expect(out).toContain("Background Jobs (1 running):");
		expect(out).toContain("[task]");
		expect(out).toContain("CoreFixes");
		expect(out).toContain("core auth hardening fixes");
	});

	it("prefers the live current action over the spawn label for task rows", () => {
		const running: AsyncJobSnapshotItem[] = [
			{
				id: "CoreFixes",
				type: "task",
				status: "running",
				label: "core auth hardening fixes",
				startTime: Date.now() - 5_000,
			},
		];
		const { ctx, addChild } = makeContext({
			running,
			describe: id => (id === "CoreFixes" ? "patching the auth handler" : undefined),
		});
		new EventController(ctx).refreshBackgroundJobs();

		const out = Bun.stripANSI((addChild.mock.calls[0][0] as CapturedText).render(200).join("\n"));
		expect(out).toContain("patching the auth handler");
		expect(out).not.toContain("core auth hardening fixes");
	});

	it("labels task and shell jobs with distinct [task]/[shell] prefixes", () => {
		const running: AsyncJobSnapshotItem[] = [
			{
				id: "CoreFixes",
				type: "task",
				status: "running",
				label: "core auth hardening fixes",
				startTime: Date.now() - 5_000,
			},
			{
				id: "bg_2",
				type: "bash",
				status: "running",
				label: "pnpm test",
				startTime: Date.now() - 5_000,
			},
		];
		const { ctx, addChild } = makeContext({ running });
		new EventController(ctx).refreshBackgroundJobs();

		const out = Bun.stripANSI((addChild.mock.calls[0][0] as CapturedText).render(200).join("\n"));
		expect(out).toContain("Background Jobs (2 running):");
		expect(out).toContain("[task]");
		expect(out).toContain("[shell]");
		expect(out).toContain("pnpm test");
	});

	it("suppresses the panel while observing a subagent and restores it on return", () => {
		const running: AsyncJobSnapshotItem[] = [
			{
				id: "CoreFixes",
				type: "task",
				status: "running",
				label: "core auth hardening fixes",
				startTime: Date.now() - 5_000,
			},
		];
		const { ctx, addChild, removeChild, state } = makeContext({ running });
		const controller = new EventController(ctx);

		controller.refreshBackgroundJobs();
		expect(addChild).toHaveBeenCalledTimes(1);

		state.focusedAgentId = "CoreFixes";
		controller.refreshBackgroundJobs();
		expect(removeChild).toHaveBeenCalledTimes(1);

		state.focusedAgentId = undefined;
		controller.refreshBackgroundJobs();
		expect(addChild).toHaveBeenCalledTimes(2);
	});
});
