/**
 * Provider-facing identity for independent background/side model requests,
 * isolated from the foreground provider session so they cannot advance it and
 * reject a waiting foreground turn (issues #10619, #10865).
 *
 * The Anthropic Messages backend orders a conversation by
 * `metadata.user_id.session_id` (a JSON envelope built by
 * {@link buildSessionMetadata}) and by the `X-Claude-Code-Session-Id` request
 * header — both derived from the request's session id. A background request
 * that surfaces the foreground session id while a foreground request is in
 * flight advances the same provider session, and the foreground request then
 * fails with HTTP 400 (`This session advanced while the request was waiting`).
 *
 * Automatic title requests already isolate their identity this way (PR #10621);
 * this module generalizes that pattern for every other automatic side request
 * (memory extraction/consolidation, speech rewriting, sharpshooter, learn
 * capture, auto-thinking, unexpected-stop, branch summary, edit auto-repair).
 *
 * Each call to {@link sideRequestIdentity} mints a fresh id, so two side
 * requests of the same kind that run concurrently (e.g. several rollout-memory
 * jobs, overlapping speech rewrites) never advance one another. Derive the
 * identity when a logical request starts — passing the *current* foreground
 * session id — so a long-lived component does not keep authenticating a later
 * session against an earlier one's account after `newSession`/`switchSession`.
 */

import type { AuthStorage } from "./auth-storage";
import { buildSessionMetadata } from "./session-metadata";

/** A provider identity for one logical side request, isolated from the foreground. */
export interface SideRequestIdentity {
	/**
	 * Fresh provider session id for this request. Pass as the request's
	 * `sessionId` option and to the model registry's `resolver`/`getApiKey` so
	 * both the metadata envelope and the session header differ from the
	 * foreground turn (and from any concurrent side request). Stable across the
	 * request's own retries because it is captured once, before the retry loop.
	 */
	readonly sessionId: string;
	/**
	 * Isolated `metadata` payload for the request's target provider. On the first
	 * call for a provider it seeds the foreground session's active OAuth account
	 * as this session's initial preference, so the request authenticates and
	 * attributes to the same account; it never re-pins afterward, so a later
	 * credential rotation on this session (blocked/exhausted account) is
	 * preserved and reflected in `account_uuid`.
	 */
	metadata(provider: string): Record<string, unknown>;
}

/**
 * Seed the foreground session's active OAuth account onto `isolatedSessionId` as
 * its initial credential preference, so a side request authenticates and
 * attributes to the same account while ordering under a distinct id. No-op when
 * there is nothing to isolate, no auth storage, or no active foreground OAuth
 * account (single-key/API-key setups resolve the same credential regardless of
 * session id). Does not overwrite an existing preference on the isolated
 * session, so a rotation already recorded there survives.
 */
export function seedSideRequestCredential(
	authStorage: AuthStorage | undefined,
	provider: string,
	isolatedSessionId: string,
	foregroundSessionId: string,
): void {
	if (!authStorage || isolatedSessionId === foregroundSessionId) return;
	const active = authStorage.listOAuthAccounts(provider, foregroundSessionId).find(account => account.active);
	if (!active) return;
	const alreadyPinned = authStorage.listOAuthAccounts(provider, isolatedSessionId).some(account => account.active);
	if (alreadyPinned) return;
	authStorage.pinSessionOAuthAccount(provider, isolatedSessionId, active.credentialId);
}

/**
 * Mint an isolated {@link SideRequestIdentity} for one logical side request,
 * derived from the *current* foreground session id. Call it once per request
 * (before any retry wrapper) so the id is stable for that request's retries but
 * unique across separate requests. Pass `authStorage` (from
 * `modelRegistry.authStorage`) so OAuth credential affinity is preserved.
 */
export function sideRequestIdentity(
	authStorage: AuthStorage | undefined,
	foregroundSessionId: string,
): SideRequestIdentity {
	const sessionId = Bun.randomUUIDv7();
	const seededProviders = new Set<string>();
	return {
		sessionId,
		metadata(provider: string): Record<string, unknown> {
			if (!seededProviders.has(provider)) {
				seededProviders.add(provider);
				seedSideRequestCredential(authStorage, provider, sessionId, foregroundSessionId);
			}
			return buildSessionMetadata(sessionId, provider, authStorage);
		},
	};
}
