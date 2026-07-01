import { describe, expect, test } from "bun:test";
import * as path from "node:path";
import { RpcClient } from "@oh-my-pi/pi-coding-agent/modes/rpc/rpc-client";
import { TempDir } from "@oh-my-pi/pi-utils";

describe("RpcClient.start", () => {
	test("rejects when RPC process exits immediately", async () => {
		using client = new RpcClient({
			cliPath: path.join(import.meta.dir, "..", "src", "cli.ts"),
			cwd: path.join(import.meta.dir, ".."),
			provider: "__missing_provider__",
			model: "claude-sonnet-4-5",
			env: { PI_NO_TITLE: "1" },
		});

		await expect(client.start()).rejects.toThrow(/Unknown provider.*__missing_provider__/);
	});

	test("can start again after stop", async () => {
		using tempDir = TempDir.createSync("@omp-rpc-client-restart-");
		const cliPath = tempDir.join("fake-rpc.ts");
		await Bun.write(
			cliPath,
			[
				'process.stdout.write(JSON.stringify({ type: "ready" }) + "\\n");',
				"for await (const _chunk of Bun.stdin.stream()) {}",
			].join("\n"),
		);
		using client = new RpcClient({
			cliPath,
			cwd: path.join(import.meta.dir, ".."),
			env: { PI_NO_TITLE: "1" },
		});

		await client.start();
		client.stop();

		await expect(client.start()).resolves.toBeUndefined();
	});
});
