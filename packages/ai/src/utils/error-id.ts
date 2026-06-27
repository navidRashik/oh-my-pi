/**
 * Stable categorical identifier for a provider error. Used to populate
 * {@link AssistantMessage.errorId} alongside `errorStatus`/`errorMessage`
 * so consumers can branch on a structured error class without scraping
 * the human-readable message.
 *
 * The taxonomy is intentionally narrow today (auth-related vs. unknown)
 * — providers and middleware can extend it as concrete failure modes
 * surface. Returns `undefined` when no taxonomy entry applies.
 *
 * NOTE: this module ships as a stub introduced alongside commit
 * cc45148cb1 (`refactor(ai): replace 'delete' with 'stripVariant' helper
 * and use 'performance.now()' for timing`), which added the import + call
 * site without committing the file. Filled in to unblock `bun check`
 * across the workspace; expand the classifier when richer per-error
 * routing is wired up.
 */
import type { Api } from "../types";

const AUTH_PATTERN =
	/\b(401|403|unauthorized|forbidden|invalid[_ -]?api[_ -]?key|missing[_ -]?api[_ -]?key|authentication)\b/i;
const RATE_LIMIT_PATTERN = /\b(429|rate[_ -]?limit|too[_ -]?many[_ -]?requests|quota)\b/i;
const TIMEOUT_PATTERN = /\b(timeout|timed[_ -]?out|deadline)\b/i;

export function errorIdFromError(error: unknown, _api: Api): string | undefined {
	const message = error instanceof Error ? error.message : typeof error === "string" ? error : undefined;
	if (!message) return undefined;
	if (AUTH_PATTERN.test(message)) return "auth";
	if (RATE_LIMIT_PATTERN.test(message)) return "rate_limit";
	if (TIMEOUT_PATTERN.test(message)) return "timeout";
	return undefined;
}
