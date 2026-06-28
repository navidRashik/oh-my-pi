import { describe, expect, it } from "bun:test";
import { loadEntriesFromFile } from "@oh-my-pi/pi-coding-agent/session/session-loader";
import { MemorySessionStorage, type SessionStorageStat } from "@oh-my-pi/pi-coding-agent/session/session-storage";

class OversizedMemorySessionStorage extends MemorySessionStorage {
	readTextCalls = 0;

	statSync(path: string): SessionStorageStat {
		return { ...super.statSync(path), size: 1024 * 1024 * 1024 + 1 };
	}

	readText(path: string): Promise<string> {
		this.readTextCalls++;
		return super.readText(path);
	}
}

describe("loadEntriesFromFile", () => {
	it("rejects oversized session files before materializing their full text", async () => {
		const storage = new OversizedMemorySessionStorage();
		const filePath = "/sessions/oversized.jsonl";
		await storage.writeText(
			filePath,
			`${JSON.stringify({ type: "session", version: 1, id: "session-1", timestamp: "2026-01-01T00:00:00.000Z" })}\n`,
		);

		await expect(loadEntriesFromFile(filePath, storage)).rejects.toThrow("Session file is too large to load safely");
		expect(storage.readTextCalls).toBe(0);
	});
});
