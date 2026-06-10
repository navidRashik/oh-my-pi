import type { AfterToolCallContext, BeforeToolCallContext, BeforeToolCallResult } from "@oh-my-pi/pi-agent-core";
import type { AssistantMessage, TextContent } from "@oh-my-pi/pi-ai";
import { countTokens } from "@oh-my-pi/pi-natives";
import type { Settings } from "../config/settings";

const READ_TOOL_NAME = "read";

/** Tool-choice queue label used when the investigation guard forces a synthesis turn. */
export const INVESTIGATION_GUARD_TOOL_CHOICE_LABEL = "investigation-guard";

interface InvestigationGuardLimits {
	enabled: boolean;
	maxReadCalls: number;
	maxReadTokens: number;
	maxConsecutiveToolUseTurns: number;
}

type SynthesisTrigger = "read-call" | "read-token" | "consecutive-tool-use";

/**
 * Tracks repeated investigation loops at two granularities and asks the model
 * to synthesize before context spirals:
 *
 * - Per-prompt `read` budget: too many `read` calls or too many read-output
 *   tokens means the agent should stop pulling more files into context.
 * - Per-prompt consecutive-`toolUse` turn cap: every turn ending in `toolUse`
 *   indicates the agent never broke out to answer. Once the count crosses
 *   the configured threshold the next LLM call is forced with no tools so
 *   the model has to produce a text response.
 */
export class InvestigationGuard {
	readonly #settings: Settings;
	#readCalls = 0;
	#readTokens = 0;
	#consecutiveToolUseTurns = 0;
	#synthesisRequested = false;

	constructor(settings: Settings) {
		this.#settings = settings;
	}

	/** Clear per-prompt counters when a fresh user or synthetic turn starts. */
	reset(): void {
		this.#readCalls = 0;
		this.#readTokens = 0;
		this.#consecutiveToolUseTurns = 0;
		this.#synthesisRequested = false;
	}

	/** Block read calls that exceed the current investigation budget. */
	beforeToolCall(ctx: BeforeToolCallContext): BeforeToolCallResult | undefined {
		if (ctx.toolCall.name !== READ_TOOL_NAME) return undefined;
		const limits = this.#limits();
		if (!limits.enabled) return undefined;

		const projectedReadCalls = this.#readCalls + this.#readOrdinalInAssistantMessage(ctx);
		const callLimitExceeded = projectedReadCalls > limits.maxReadCalls;
		const tokenLimitExceeded = this.#readTokens >= limits.maxReadTokens;
		if (!callLimitExceeded && !tokenLimitExceeded) return undefined;

		this.#synthesisRequested = true;
		return {
			block: true,
			reason: this.#blockReason(limits, callLimitExceeded ? "read-call" : "read-token"),
		};
	}

	/** Account for read-tool output and request synthesis once accumulated output crosses the token budget. */
	afterToolCall(ctx: AfterToolCallContext): void {
		if (ctx.toolCall.name !== READ_TOOL_NAME || ctx.isError) return;
		const limits = this.#limits();
		if (!limits.enabled) return;

		this.#readCalls++;
		const texts = ctx.result.content.filter((content): content is TextContent => content.type === "text");
		if (texts.length > 0) {
			this.#readTokens += countTokens(texts.map(content => content.text));
		}
		if (this.#readTokens >= limits.maxReadTokens || this.#readCalls >= limits.maxReadCalls) {
			this.#synthesisRequested = true;
		}
	}

	/**
	 * Record an assistant turn's stop reason. A `toolUse` stop bumps the
	 * consecutive-tool-use counter and may request synthesis; a clean `stop`
	 * (text response) is treated as the agent breaking out of investigation
	 * and clears all counters. `error`/`aborted` leaves counters untouched.
	 */
	noteAssistantStop(stopReason: AssistantMessage["stopReason"]): void {
		if (stopReason === "toolUse") {
			this.#consecutiveToolUseTurns++;
			const limits = this.#limits();
			if (limits.enabled && this.#consecutiveToolUseTurns >= limits.maxConsecutiveToolUseTurns) {
				this.#synthesisRequested = true;
			}
		} else if (stopReason === "stop") {
			this.reset();
		}
	}

	/** Consume the pending request to force the next LLM call to run without tools. */
	consumeSynthesisRequest(): boolean {
		if (!this.#synthesisRequested) return false;
		this.#synthesisRequested = false;
		return true;
	}

	#limits(): InvestigationGuardLimits {
		const maxReadCalls = Math.max(1, Math.floor(this.#settings.get("investigationGuard.maxReadCalls")));
		const maxReadTokens = Math.max(1, Math.floor(this.#settings.get("investigationGuard.maxReadTokens")));
		const maxConsecutiveToolUseTurns = Math.max(
			1,
			Math.floor(this.#settings.get("investigationGuard.maxConsecutiveToolUseTurns")),
		);
		return {
			enabled: this.#settings.get("investigationGuard.enabled"),
			maxReadCalls,
			maxReadTokens,
			maxConsecutiveToolUseTurns,
		};
	}

	#readOrdinalInAssistantMessage(ctx: BeforeToolCallContext): number {
		let ordinal = 0;
		for (const block of ctx.assistantMessage.content) {
			if (block.type !== "toolCall" || block.name !== READ_TOOL_NAME) continue;
			ordinal++;
			if (block.id === ctx.toolCall.id) return ordinal;
		}
		return 1;
	}

	#blockReason(limits: InvestigationGuardLimits, trigger: SynthesisTrigger): string {
		const detail =
			trigger === "read-call"
				? `${limits.maxReadCalls} read calls`
				: trigger === "read-token"
					? `${limits.maxReadTokens} read-output tokens`
					: `${limits.maxConsecutiveToolUseTurns} consecutive tool-use turns`;
		return `Read investigation limit reached after ${detail}. Stop reading more files and answer from the evidence already gathered; if exact missing lines are required, explain the narrow follow-up read instead of continuing the tool loop.`;
	}
}
