import { describe, expect, it } from "bun:test";
import { AutoLearnController, buildAutoLearnInstructions } from "@oh-my-pi/pi-coding-agent/autolearn/controller";
import { Settings } from "@oh-my-pi/pi-coding-agent/config/settings";
import type { AgentSession, AgentSessionEvent } from "@oh-my-pi/pi-coding-agent/session/agent-session";

interface CapturedNudge {
	message: { customType: string; content: string; display?: boolean; attribution?: string };
	options?: { deliverAs?: string; triggerTurn?: boolean };
}

class FakeSession {
	readonly listeners: Array<(event: AgentSessionEvent) => void> = [];
	readonly sent: CapturedNudge[] = [];
	readonly captures: string[] = [];
	planEnabled = false;
	goalEnabled = false;

	subscribe(listener: (event: AgentSessionEvent) => void): () => void {
		this.listeners.push(listener);
		return () => {};
	}

	async sendCustomMessage(message: CapturedNudge["message"], options?: CapturedNudge["options"]): Promise<boolean> {
		this.sent.push({ message, options });
		return options?.triggerTurn === true;
	}

	getPlanModeState(): { enabled: boolean } | undefined {
		return this.planEnabled ? { enabled: true } : undefined;
	}

	getGoalModeState(): { enabled: boolean } | undefined {
		return this.goalEnabled ? { enabled: true } : undefined;
	}

	emit(event: AgentSessionEvent): void {
		for (const listener of [...this.listeners]) listener(event);
	}

	toolCalls(n: number): void {
		for (let i = 0; i < n; i++) {
			this.emit({ type: "tool_execution_end", toolCallId: `t${i}`, toolName: "read", result: null });
		}
	}

	agentStart(): void {
		this.emit({ type: "agent_start" });
	}

	agentEnd(): void {
		this.emit({ type: "agent_end", messages: [] });
	}
}

function install(
	session: FakeSession,
	overrides: Record<string, unknown> = {},
	capture: (content: string) => Promise<void> = async content => {
		session.captures.push(content);
	},
): Settings {
	const settings = Settings.isolated({ "autolearn.enabled": true, ...overrides });
	new AutoLearnController({ session: session as unknown as AgentSession, settings, capture });
	return settings;
}

async function flushMicrotasks(): Promise<void> {
	await Promise.resolve();
	await Promise.resolve();
}

describe("AutoLearnController", () => {
	it("does not inject a passive nudge into the conversation prefix", () => {
		const session = new FakeSession();
		install(session);
		session.toolCalls(5);
		session.agentEnd();

		expect(session.captures).toHaveLength(0);
		expect(session.sent).toHaveLength(0);
	});

	it("auto-continue captures privately instead of mutating the primary transcript", () => {
		const session = new FakeSession();
		install(session, { "autolearn.autoContinue": true });
		session.toolCalls(5);
		session.agentEnd();
		expect(session.captures).toHaveLength(1);
		expect(session.sent).toHaveLength(0);
		const body = session.captures[0] ?? "";
		// Frames the prompt as automated, not as the user's response.
		expect(body).toMatch(/not a user reply|not from the user/i);
		// Forbids inferring approval / acting on pending questions.
		expect(body).toMatch(/not.*(approval|accept|pending|prior)/i);
		// Demands a hard stop after capture, with no continuation.
		expect(body).toMatch(/then stop\./i);
		expect(body).toMatch(/do not.*(continue|resume|other tools)/i);
		expect(body).toMatch(/wait for the user'?s next prompt/i);
	});

	it("does not nudge below the threshold", () => {
		const session = new FakeSession();
		install(session, { "autolearn.autoContinue": true });
		session.toolCalls(4);
		session.agentEnd();
		expect(session.captures).toHaveLength(0);
		expect(session.sent).toHaveLength(0);
	});

	it("does not nudge during plan mode", () => {
		const session = new FakeSession();
		session.planEnabled = true;
		install(session, { "autolearn.autoContinue": true });
		session.toolCalls(5);
		session.agentEnd();
		expect(session.captures).toHaveLength(0);
	});
	it("does not combine tool calls across separate sub-threshold turns", () => {
		const session = new FakeSession();
		install(session, { "autolearn.autoContinue": true });
		session.toolCalls(3);
		session.agentEnd();
		session.toolCalls(3);
		session.agentEnd();
		// Neither turn reached the threshold; the counter must not accumulate.
		expect(session.captures).toHaveLength(0);
	});

	it("discards plan-mode tool calls instead of leaking them into the next turn", () => {
		const session = new FakeSession();
		session.planEnabled = true;
		install(session, { "autolearn.autoContinue": true });
		session.toolCalls(5);
		session.agentEnd(); // plan mode: no fire, counter reset
		session.planEnabled = false;
		session.toolCalls(1);
		session.agentEnd(); // 1 < threshold -> no fire (no plan-mode leak)
		expect(session.captures).toHaveLength(0);
	});

	it("stops auto-continuing when autolearn is disabled mid-session", () => {
		const session = new FakeSession();
		// Enable via the global layer (not an isolated override) so the live flag
		// can be flipped and the controller's fire-time re-check is exercised.
		const settings = Settings.isolated({ "autolearn.autoContinue": true });
		settings.set("autolearn.enabled", true);
		new AutoLearnController({
			session: session as unknown as AgentSession,
			settings,
			capture: async content => {
				session.captures.push(content);
			},
		});
		session.toolCalls(5);
		session.agentEnd();
		expect(session.captures).toHaveLength(1); // fires while enabled
		settings.set("autolearn.enabled", false);
		session.toolCalls(5);
		session.agentEnd();
		expect(session.captures).toHaveLength(1); // no new capture after disable
		// The disabled stop must NOT leave its tool calls queued: re-enabling and
		// doing a sub-threshold turn must not fire from leaked counts.
		settings.set("autolearn.enabled", true);
		session.toolCalls(1);
		session.agentEnd();
		expect(session.captures).toHaveLength(1);
	});

	it("does not nudge during goal mode and leaks no suppression latch", () => {
		const session = new FakeSession();
		session.goalEnabled = true;
		install(session, { "autolearn.autoContinue": true });
		session.toolCalls(5);
		session.agentEnd();
		// Goal mode owns the continuation; auto-learn stays out of the loop.
		expect(session.captures).toHaveLength(0);
		// The skipped stop must not leak into the next non-goal stop.
		session.goalEnabled = false;
		session.toolCalls(5);
		session.agentEnd();
		expect(session.captures).toHaveLength(1);
	});

	it("never nudges a turn that started in goal mode even if the goal ended mid-turn", () => {
		const session = new FakeSession();
		session.goalEnabled = true;
		install(session, { "autolearn.autoContinue": true });
		// The turn begins as a goal continuation...
		session.agentStart();
		session.toolCalls(5);
		// ...then a `goal` tool completes/drops the goal mid-turn: the live flag is
		// off by the time the turn stops, but this turn must still never be nudged.
		session.goalEnabled = false;
		session.agentEnd();
		expect(session.captures).toHaveLength(0);

		// The capture is per-turn: a fresh turn that did not start in goal mode
		// nudges normally, proving the latch resets.
		session.agentStart();
		session.toolCalls(5);
		session.agentEnd();
		expect(session.captures).toHaveLength(1);
	});

	it("does not overlap private captures and runs one pending capture after teardown", async () => {
		const session = new FakeSession();
		const gate = Promise.withResolvers<void>();
		install(session, { "autolearn.autoContinue": true }, async content => {
			session.captures.push(content);
			await gate.promise;
		});

		session.toolCalls(5);
		session.agentEnd();
		expect(session.captures).toHaveLength(1);
		expect(session.sent).toHaveLength(0);

		session.toolCalls(5);
		session.agentEnd();
		session.toolCalls(5);
		session.agentEnd();
		expect(session.captures).toHaveLength(1);

		gate.resolve();
		await flushMicrotasks();
		expect(session.captures).toHaveLength(2);
	});

	it("does not start a second capture while the private capture is pending", async () => {
		const session = new FakeSession();
		const gate = Promise.withResolvers<void>();
		install(session, { "autolearn.autoContinue": true }, async content => {
			session.captures.push(content);
			await gate.promise;
		});
		session.toolCalls(5);
		session.agentEnd();
		expect(session.captures).toHaveLength(1);

		session.toolCalls(5);
		session.agentEnd();
		expect(session.captures).toHaveLength(1);
		gate.resolve();
		await flushMicrotasks();
		expect(session.captures).toHaveLength(2);
	});

	it("continues future captures when a private capture fails", async () => {
		const session = new FakeSession();
		let failNext = true;
		install(session, { "autolearn.autoContinue": true }, async content => {
			if (failNext) {
				failNext = false;
				throw new Error(`capture failed for ${content.slice(0, 8)}`);
			}
			session.captures.push(content);
		});
		session.toolCalls(5);
		session.agentEnd();
		expect(session.captures).toHaveLength(0);
		await flushMicrotasks(); // flush the async failure cleanup
		session.toolCalls(5);
		session.agentEnd();
		expect(session.captures).toHaveLength(1);
		expect(session.sent).toHaveLength(0);
	});

	it("respects a custom minToolCalls threshold", () => {
		const session = new FakeSession();
		install(session, { "autolearn.autoContinue": true, "autolearn.minToolCalls": 2 });
		session.toolCalls(2);
		session.agentEnd();
		expect(session.captures).toHaveLength(1);
	});
});

describe("buildAutoLearnInstructions", () => {
	it("returns null when manage_skill is not in the active tool set", () => {
		expect(buildAutoLearnInstructions({ manageSkill: false, learn: false })).toBeNull();
		// learn without manage_skill still yields no guidance (manage_skill gates it).
		expect(buildAutoLearnInstructions({ manageSkill: false, learn: true })).toBeNull();
	});

	it("includes the learn addendum when the learn tool is present", () => {
		const text = buildAutoLearnInstructions({ manageSkill: true, learn: true });
		expect(text).toContain("manage_skill");
		expect(text).toContain("long-term memory");
	});

	it("omits the learn addendum when only manage_skill is present", () => {
		const text = buildAutoLearnInstructions({ manageSkill: true, learn: false });
		expect(text).toContain("manage_skill");
		expect(text).not.toContain("long-term memory");
	});
});
