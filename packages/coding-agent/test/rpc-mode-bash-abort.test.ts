import { describe, expect, test } from "bun:test";
import * as path from "node:path";
import type { ReadableStream as NodeReadableStream, ReadableStreamDefaultReader } from "node:stream/web";
import { TempDir } from "@oh-my-pi/pi-utils";
import type { FileSink } from "bun";

type RpcFrame = Record<string, unknown>;

class RpcJsonlReader {
	#buffer = "";
	#decoder = new TextDecoder();
	#reader: ReadableStreamDefaultReader<Uint8Array>;

	constructor(stream: NodeReadableStream<Uint8Array>) {
		this.#reader = stream.getReader();
	}

	async nextFrame(timeoutMs: number): Promise<RpcFrame | null> {
		const deadline = Date.now() + timeoutMs;
		while (Date.now() < deadline) {
			const newline = this.#buffer.indexOf("\n");
			if (newline >= 0) {
				const line = this.#buffer.slice(0, newline);
				this.#buffer = this.#buffer.slice(newline + 1);
				if (line.trim().length === 0) continue;
				const parsed: unknown = JSON.parse(line);
				if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
					throw new Error(`RPC frame is not an object: ${line}`);
				}
				return parsed as RpcFrame;
			}

			const remainingMs = Math.max(1, deadline - Date.now());
			const chunk = await Promise.race([this.#reader.read(), Bun.sleep(remainingMs).then(() => null)]);
			if (!chunk || chunk.done) return null;
			this.#buffer += this.#decoder.decode(chunk.value, { stream: true });
		}
		return null;
	}
}

describe("RPC mode bash abort", () => {
	test("processes abort_bash while a bash command is still pending", async () => {
		using tempDir = TempDir.createSync("@omp-rpc-mode-bash-abort-");
		const harnessPath = tempDir.join("rpc-mode-harness.ts");
		const rpcModePath = path.join(import.meta.dir, "..", "src", "modes", "rpc", "rpc-mode.ts");
		const agentSessionPath = path.join(import.meta.dir, "..", "src", "session", "agent-session.ts");
		const bashExecutorPath = path.join(import.meta.dir, "..", "src", "exec", "bash-executor.ts");
		await Bun.write(
			harnessPath,
			`
import type { BashResult } from ${JSON.stringify(bashExecutorPath)};
import { runRpcMode } from ${JSON.stringify(rpcModePath)};
import type { AgentSession } from ${JSON.stringify(agentSessionPath)};

let finishBash: ((result: BashResult) => void) | undefined;
const abortedResult: BashResult = {
	output: "aborted",
	exitCode: 1,
	cancelled: true,
	truncated: false,
	totalLines: 1,
	totalBytes: 7,
	outputLines: 1,
	outputBytes: 7,
};
const session = {
	customCommands: [],
	skills: [],
	skillsSettings: {},
	sessionManager: { getCwd: () => ${JSON.stringify(tempDir.path())} },
	setSlashCommands: () => {},
	refreshSshTool: async () => {},
	subscribe: () => {},
	subscribeCommandMetadataChanged: () => {},
	executeBash: async () => {
		return await new Promise<BashResult>(resolve => {
			finishBash = resolve;
		});
	},
	abortBash: () => {
		finishBash?.(abortedResult);
	},
} as unknown as AgentSession;

await runRpcMode(session);
`,
		);
		const proc = Bun.spawn(["bun", harnessPath], {
			stdin: "pipe",
			stdout: "pipe",
			stderr: "pipe",
			cwd: path.join(import.meta.dir, ".."),
		});
		const reader = new RpcJsonlReader(proc.stdout as NodeReadableStream<Uint8Array>);
		const stdin = proc.stdin as FileSink;
		try {
			await expect(reader.nextFrame(2_000)).resolves.toMatchObject({ type: "ready" });
			stdin.write(`${JSON.stringify({ id: "b1", type: "bash", command: "block" })}\n`);
			await stdin.flush();
			await Bun.sleep(50);
			stdin.write(`${JSON.stringify({ id: "a1", type: "abort_bash" })}\n`);
			await stdin.flush();

			let abortResponse: RpcFrame | null = null;
			let bashResponse: RpcFrame | null = null;
			const deadline = Date.now() + 2_000;
			while (Date.now() < deadline && (!abortResponse || !bashResponse)) {
				const frame = await reader.nextFrame(Math.max(1, deadline - Date.now()));
				if (!frame) break;
				if (frame.type !== "response") continue;
				if (frame.id === "a1") abortResponse = frame;
				if (frame.id === "b1") bashResponse = frame;
			}

			expect(abortResponse).toMatchObject({ id: "a1", type: "response", command: "abort_bash", success: true });
			expect(bashResponse).toMatchObject({
				id: "b1",
				type: "response",
				command: "bash",
				success: true,
				data: { cancelled: true },
			});
		} finally {
			proc.kill();
			await proc.exited.catch(() => undefined);
		}
	}, 10_000);
});
