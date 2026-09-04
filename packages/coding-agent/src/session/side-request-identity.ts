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
 * capture, auto-thinking, unexpected-stop, branch summary).
 */

import type { AuthStorage } from "./auth-storage";
import { buildSessionMetadata } from "./session-metadata";
/** A provider identity for one side-request scope, isolated from the foreground. */
export interface SideRequestIdentity {
	/**
	 * Distinct, stable provider session id for this scope. Pass as the request's
	 * `sessionId` option and to the model registry's `resolver`/`getApiKey` so
	 * both the metadata envelope and the session header differ from the foreground.
	 */
	readonly sessionId: string;
	/**
	 * Isolated `metadata` payload for the request's target provider. Seeds the
	 * foreground session's active OAuth account onto the isolated session first,
	 * so auth/attribution/billing stay on the same account while the ordering
	 * identity is separated. Call it before resolving the request's API key.
	 */
	metadata(provider: string): Record<string, unknown>;
}

/** Namespace prefix so a derived side id can never coincide with a real id. */
const SIDE_REQUEST_NAMESPACE = "omp.side-request";

/**
 * Derive a stable, foreground-distinct provider session id for `scope`.
 *
 * Deterministic (a hash of the foreground id + scope) so repeated requests of
 * one kind reuse a single side identity — matching the cached title/advisor
 * identities — without threading a shared cache through every call path. The
 * result is shaped like an RFC 4122 v8 UUID so it is indistinguishable from a
 * normal provider session id on the wire.
 */
export function deriveSideRequestSessionId(foregroundSessionId: string, scope: string): string {
	const digest = new Bun.CryptoHasher("sha256")
		.update(`${SIDE_REQUEST_NAMESPACE}\u0000${scope}\u0000${foregroundSessionId}`)
		.digest("hex");
	const hex = digest.slice(0, 32).split("");
	// version 8 (custom) in the 13th nibble; RFC 4122 variant (10xx) in the 17th.
	hex[12] = "8";
	hex[16] = "89ab"[Number.parseInt(digest[16], 16) & 0x3];
	const s = hex.join("");
	return `${s.slice(0, 8)}-${s.slice(8, 12)}-${s.slice(12, 16)}-${s.slice(16, 20)}-${s.slice(20, 32)}`;
}

/**
 * Copy the foreground session's active OAuth account onto `isolatedSessionId`
 * so a side request authenticates and attributes to the same account while
 * ordering under a distinct id. No-op when there is nothing to isolate, no auth
 * storage, or no active foreground OAuth account (single-key/API-key setups
 * resolve the same credential regardless of session id).
 */
export function seedSideRequestCredential(
	authStorage: AuthStorage | undefined,
	provider: string,
	isolatedSessionId: string,
	foregroundSessionId: string,
): void {
	if (!authStorage || isolatedSessionId === foregroundSessionId) return;
	const active = authStorage.listOAuthAccounts(provider, foregroundSessionId).find(account => account.active);
	if (active) authStorage.pinSessionOAuthAccount(provider, isolatedSessionId, active.credentialId);
}

/**
 * Build an isolated {@link SideRequestIdentity} for `scope` derived from the
 * foreground session id. Pass `authStorage` (from `modelRegistry.authStorage`)
 * so OAuth credential affinity is preserved.
 */
export function sideRequestIdentity(
	authStorage: AuthStorage | undefined,
	foregroundSessionId: string,
	scope: string,
): SideRequestIdentity {
	const sessionId = deriveSideRequestSessionId(foregroundSessionId, scope);
	return {
		sessionId,
		metadata(provider: string): Record<string, unknown> {
			seedSideRequestCredential(authStorage, provider, sessionId, foregroundSessionId);
			return buildSessionMetadata(sessionId, provider, authStorage);
		},
	};
}
