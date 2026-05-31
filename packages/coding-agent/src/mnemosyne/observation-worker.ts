import type { ThinkingLevel } from "@oh-my-pi/pi-agent-core";
import type { Api, AssistantMessage, Effort, Message, Model, SimpleStreamOptions } from "@oh-my-pi/pi-ai";
import { completeSimple } from "@oh-my-pi/pi-ai";
import { prompt } from "@oh-my-pi/pi-utils";
import observerSystemPrompt from "../prompts/mnemosyne/observer-system.md" with { type: "text" };
import observerUserPrompt from "../prompts/mnemosyne/observer-user.md" with { type: "text" };
import { toReasoningEffort } from "../thinking";
import type { ObservationInput, ObservationRelevance } from "./observations";

const DEFAULT_MAX_TOKENS = 1_500;
const OBSERVATION_CONTENT_LIMIT = 1_000;
const OBSERVATION_TIMESTAMP_PATTERN = /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/;

/** Source-labelled transcript chunk passed to the observation worker. */
export interface ObservationChunk {
	text: string;
	sourceEntryIds: readonly string[];
}

/** Injectable completion seam for observation-worker tests. */
export type ObservationCompleteFn = (
	model: Model<Api>,
	context: { systemPrompt: string[]; messages: Message[] },
	options: SimpleStreamOptions,
) => Promise<AssistantMessage>;

/** Model invocation dependencies for {@link runObservationWorker}. */
export interface ObservationWorkerInput {
	model: Model<Api>;
	apiKey: string;
	thinkingLevel?: ThinkingLevel;
	chunk: ObservationChunk;
	existingObservations?: readonly string[];
	currentTime: string;
	maxTokens?: number;
	signal?: AbortSignal;
	complete?: ObservationCompleteFn;
}

interface RawObservationPayload {
	observations?: unknown;
}

interface RawObservationRecord {
	content?: unknown;
	timestamp?: unknown;
	relevance?: unknown;
	sourceEntryIds?: unknown;
}

/** Run the smol-model observer over one source-labelled transcript chunk. */
export async function runObservationWorker(input: ObservationWorkerInput): Promise<ObservationInput[]> {
	if (input.chunk.sourceEntryIds.length === 0 || !input.chunk.text.trim()) return [];
	const systemPrompt = prompt.render(observerSystemPrompt);
	const userPrompt = prompt.render(observerUserPrompt, {
		current_time: input.currentTime,
		existing_observations: input.existingObservations?.length ? input.existingObservations.join("\n") : "(none yet)",
		chunk: input.chunk.text,
	});
	const reasoning: Effort | undefined = toReasoningEffort(input.thinkingLevel);
	const complete = input.complete ?? completeSimple;
	const options: SimpleStreamOptions = {
		apiKey: input.apiKey,
		maxTokens: Math.max(1, Math.floor(input.maxTokens ?? DEFAULT_MAX_TOKENS)),
		signal: input.signal,
	};
	if (reasoning !== undefined) options.reasoning = reasoning;
	const message = await complete(
		input.model,
		{
			systemPrompt: [systemPrompt],
			messages: [{ role: "user", content: userPrompt, timestamp: Date.now() }],
		},
		options,
	);
	return parseObservationWorkerResponse(message, input.chunk.sourceEntryIds, input.currentTime);
}

/** Parse and validate a worker response against the source ids shown to it. */
export function parseObservationWorkerResponse(
	message: AssistantMessage,
	allowedSourceEntryIds: readonly string[],
	fallbackTimestamp: string,
): ObservationInput[] {
	const text = message.content
		.filter(block => block.type === "text")
		.map(block => block.text)
		.join("\n")
		.trim();
	if (!text) return [];
	const raw = parseJsonPayload(text);
	if (!raw || typeof raw !== "object") return [];
	const payload = raw as RawObservationPayload;
	if (!Array.isArray(payload.observations)) return [];
	const allowed = new Set(allowedSourceEntryIds);
	const out: ObservationInput[] = [];
	const seen = new Set<string>();
	for (const item of payload.observations) {
		const observation = normalizeObservation(item, allowed, fallbackTimestamp);
		if (!observation) continue;
		const key = `${observation.content}\u0000${observation.timestamp}\u0000${observation.sourceEntryIds.join("\u0000")}`;
		if (seen.has(key)) continue;
		seen.add(key);
		out.push(observation);
	}
	return out;
}

function parseJsonPayload(text: string): unknown {
	try {
		return JSON.parse(text);
	} catch {
		const fenced = /```(?:json)?\s*([\s\S]*?)\s*```/i.exec(text);
		if (fenced) {
			try {
				return JSON.parse(fenced[1]);
			} catch {
				return undefined;
			}
		}
		const start = text.indexOf("{");
		const end = text.lastIndexOf("}");
		if (start < 0 || end <= start) return undefined;
		try {
			return JSON.parse(text.slice(start, end + 1));
		} catch {
			return undefined;
		}
	}
}

function normalizeObservation(
	item: unknown,
	allowed: ReadonlySet<string>,
	fallbackTimestamp: string,
): ObservationInput | undefined {
	if (!item || typeof item !== "object") return undefined;
	const record = item as RawObservationRecord;
	const content = typeof record.content === "string" ? normalizeContent(record.content) : undefined;
	if (!content) return undefined;
	const timestamp = typeof record.timestamp === "string" && OBSERVATION_TIMESTAMP_PATTERN.test(record.timestamp)
		? record.timestamp
		: fallbackTimestamp;
	if (!isObservationRelevance(record.relevance)) return undefined;
	if (!Array.isArray(record.sourceEntryIds)) return undefined;
	const sourceEntryIds = normalizeSourceEntryIds(record.sourceEntryIds, allowed);
	if (sourceEntryIds.length === 0) return undefined;
	return { content, timestamp, relevance: record.relevance, sourceEntryIds };
}

function normalizeContent(content: string): string | undefined {
	const normalized = content.replace(/[\r\n\t]+/g, " ").replace(/\s{2,}/g, " ").trim();
	if (!normalized) return undefined;
	return normalized.length > OBSERVATION_CONTENT_LIMIT ? `${normalized.slice(0, OBSERVATION_CONTENT_LIMIT - 1).trimEnd()}…` : normalized;
}

function normalizeSourceEntryIds(values: readonly unknown[], allowed: ReadonlySet<string>): string[] {
	const out: string[] = [];
	const seen = new Set<string>();
	for (const value of values) {
		if (typeof value !== "string" || !allowed.has(value) || seen.has(value)) continue;
		seen.add(value);
		out.push(value);
	}
	return out;
}

function isObservationRelevance(value: unknown): value is ObservationRelevance {
	return value === "low" || value === "medium" || value === "high" || value === "critical";
}
