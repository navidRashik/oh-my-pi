import { Database } from "bun:sqlite";
import { describe, expect, it } from "bun:test";
import { AuthStorage, SqliteAuthCredentialStore } from "@oh-my-pi/pi-coding-agent/session/auth-storage";
import { buildSessionMetadata } from "@oh-my-pi/pi-coding-agent/session/session-metadata";
import {
	deriveSideRequestSessionId,
	sideRequestIdentity,
} from "@oh-my-pi/pi-coding-agent/session/side-request-identity";

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

/** Read the `metadata.user_id` JSON envelope without an unchecked cast. */
function readUserId(metadata: Record<string, unknown>): { session_id: string; account_uuid?: string } {
	const raw = metadata.user_id;
	if (typeof raw !== "string") throw new Error("expected metadata.user_id string");
	const parsed: unknown = JSON.parse(raw);
	if (!parsed || typeof parsed !== "object") throw new Error("expected user_id object");
	if (!("session_id" in parsed) || typeof parsed.session_id !== "string") {
		throw new Error("expected user_id.session_id");
	}
	const accountUuid =
		"account_uuid" in parsed && typeof parsed.account_uuid === "string" ? parsed.account_uuid : undefined;
	return { session_id: parsed.session_id, account_uuid: accountUuid };
}

describe("side-request identity (issue #10865)", () => {
	it("derives a foreground-distinct, stable, per-scope provider session id", () => {
		const foreground = "01J8FOREGROUND0000000000000000";
		const a = deriveSideRequestSessionId(foreground, "mnemopi");
		const b = deriveSideRequestSessionId(foreground, "mnemopi");
		const other = deriveSideRequestSessionId(foreground, "sharpshooter");

		// UUID-shaped so it is indistinguishable from a real provider session id.
		expect(a).toMatch(UUID_RE);
		// Never the foreground id: a background request must not order under it.
		expect(a).not.toBe(foreground);
		// Stable per (foreground, scope): repeated requests reuse one side identity.
		expect(b).toBe(a);
		// Distinct scopes never collide with each other.
		expect(other).not.toBe(a);
		// Distinct foreground sessions never collide.
		expect(deriveSideRequestSessionId("other-foreground", "mnemopi")).not.toBe(a);
	});

	it("isolates the metadata ordering identity from the foreground session", () => {
		const foreground = "provider-session-foreground";
		const identity = sideRequestIdentity(undefined, foreground, "mnemopi");
		const metadata = identity.metadata("anthropic");

		const foregroundSessionId = readUserId(buildSessionMetadata(foreground, "anthropic", undefined)).session_id;
		const sideSessionId = readUserId(metadata).session_id;

		// The provider orders on metadata.user_id.session_id; the side request must
		// carry a different one so it cannot advance the foreground provider session.
		expect(sideSessionId).toBe(identity.sessionId);
		expect(sideSessionId).not.toBe(foregroundSessionId);
	});

	it("keeps the foreground's active OAuth account while isolating the session id", async () => {
		const store = new SqliteAuthCredentialStore(new Database(":memory:"));
		store.saveOAuth("anthropic", {
			access: "account-a-token",
			refresh: "account-a-refresh",
			expires: Date.now() + 60_000,
			accountId: "account-a",
		});
		store.saveOAuth("anthropic", {
			access: "account-b-token",
			refresh: "account-b-refresh",
			expires: Date.now() + 60_000,
			accountId: "account-b",
		});
		const storage = new AuthStorage(store);
		try {
			await storage.reload();
			storage.clearConfigApiKeys();
			const foreground = "provider-session-foreground";
			const accountB = storage.listOAuthAccounts("anthropic").find(account => account.accountId === "account-b");
			if (!accountB) throw new Error("expected account-b credential");
			// Foreground turn resolved onto account-b.
			expect(storage.pinSessionOAuthAccount("anthropic", foreground, accountB.credentialId)).toBe(true);

			const identity = sideRequestIdentity(storage, foreground, "mnemopi");
			const sideMeta = readUserId(identity.metadata("anthropic"));
			const foregroundMeta = readUserId(buildSessionMetadata(foreground, "anthropic", storage));

			// Same account (attribution/billing preserved)...
			expect(sideMeta.account_uuid).toBe("account-b");
			expect(sideMeta.account_uuid).toBe(foregroundMeta.account_uuid);
			// ...but a distinct ordering identity (the isolation).
			expect(sideMeta.session_id).toBe(identity.sessionId);
			expect(sideMeta.session_id).not.toBe(foregroundMeta.session_id);
			// The isolated session now resolves to the same account for auth.
			expect(storage.getOAuthAccountId("anthropic", identity.sessionId)).toBe("account-b");
		} finally {
			storage.close();
		}
	});
});
