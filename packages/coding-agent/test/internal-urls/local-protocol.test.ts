import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import * as fs from "node:fs/promises";
import * as os from "node:os";
import * as path from "node:path";
import {
	InternalUrlRouter,
	LocalProtocolHandler,
	resolveLocalRoot,
	resolveLocalUrlToPath,
} from "@oh-my-pi/pi-coding-agent/internal-urls";

async function withTempDir<T>(fn: (dir: string) => Promise<T>): Promise<T> {
	const dir = await fs.mkdtemp(path.join(os.tmpdir(), "local-protocol-"));
	try {
		return await fn(dir);
	} finally {
		await fs.rm(dir, { recursive: true, force: true });
	}
}

describe("LocalProtocolHandler", () => {
	beforeEach(() => {
		LocalProtocolHandler.resetOverrideForTests();
		InternalUrlRouter.resetForTests();
	});

	afterEach(() => {
		LocalProtocolHandler.resetOverrideForTests();
		InternalUrlRouter.resetForTests();
	});

	it("lists files at local://", async () => {
		await withTempDir(async tempDir => {
			const artifactsDir = path.join(tempDir, "artifacts");
			await fs.mkdir(path.join(artifactsDir, "local"), { recursive: true });
			await Bun.write(path.join(artifactsDir, "local", "handoff.json"), '{"ok":true}');

			LocalProtocolHandler.setOverride({
				getArtifactsDir: () => artifactsDir,
				getSessionId: () => "session-a",
			});
			const router = InternalUrlRouter.instance();
			const resource = await router.resolve("local://");

			expect(resource.contentType).toBe("text/markdown");
			expect(resource.content).toContain("handoff.json");
		});
	});

	it("reads a local file from session local root", async () => {
		await withTempDir(async tempDir => {
			const artifactsDir = path.join(tempDir, "artifacts");
			const localFile = path.join(artifactsDir, "local", "subtasks", "trace.txt");
			await fs.mkdir(path.dirname(localFile), { recursive: true });
			await Bun.write(localFile, "trace");

			LocalProtocolHandler.setOverride({
				getArtifactsDir: () => artifactsDir,
				getSessionId: () => "session-b",
			});
			const router = InternalUrlRouter.instance();
			const resource = await router.resolve("local://subtasks/trace.txt");

			expect(resource.content).toBe("trace");
			expect(resource.contentType).toBe("text/plain");
		});
	});

	it("blocks path traversal attempts", async () => {
		await withTempDir(async tempDir => {
			LocalProtocolHandler.setOverride({
				getArtifactsDir: () => path.join(tempDir, "artifacts"),
				getSessionId: () => "session-c",
			});
			const router = InternalUrlRouter.instance();
			await expect(router.resolve("local://../secret.txt")).rejects.toThrow(
				"Path traversal (..) is not allowed in local:// URLs",
			);
			await expect(router.resolve("local://%2E%2E/secret.txt")).rejects.toThrow(
				"Path traversal (..) is not allowed in local:// URLs",
			);
		});
	});

	it("uses session id fallback root when artifacts dir is unavailable", async () => {
		const root = resolveLocalRoot({ getSessionId: () => "session-fallback", getArtifactsDir: () => null });
		expect(root).toContain(path.join("omp-local", "session-fallback"));
		expect(resolveLocalUrlToPath("local://memo.txt", { getSessionId: () => "session-fallback" })).toBe(
			path.join(root, "memo.txt"),
		);
	});

	it("uses a stable short temp root for long Windows artifact paths", async () => {
		const longArtifactsDir = path.join(os.tmpdir(), "a".repeat(220), "artifacts");
		const expectedRoot = path.join(os.tmpdir(), "omp-local", "session_long");
		const options = {
			getArtifactsDir: () => longArtifactsDir,
			getSessionId: () => "session:long",
		};
		const root = resolveLocalRoot(options, "win32");
		const resolved = resolveLocalUrlToPath("local://memo.txt", options, "win32");

		expect(root).toBe(expectedRoot);
		expect(resolved).toBe(path.join(expectedRoot, "memo.txt"));

		// The short root must survive moves of the artifact directory so
		// `local://PLAN.md` and handoff files written pre-move stay reachable
		// after `SessionManager.moveTo()` updates `getArtifactsDir()`.
		const movedOptions = {
			getArtifactsDir: () => path.join(os.tmpdir(), "b".repeat(220), "artifacts"),
			getSessionId: () => "session:long",
		};
		expect(resolveLocalRoot(movedOptions, "win32")).toBe(expectedRoot);
	});

	it("blocks symlink escapes outside local root", async () => {
		if (process.platform === "win32") return;

		await withTempDir(async tempDir => {
			const artifactsDir = path.join(tempDir, "artifacts");
			const localRoot = path.join(artifactsDir, "local");
			const outsideDir = path.join(tempDir, "outside");
			await fs.mkdir(localRoot, { recursive: true });
			await fs.mkdir(outsideDir, { recursive: true });
			await Bun.write(path.join(outsideDir, "secret.txt"), "secret");
			await fs.symlink(outsideDir, path.join(localRoot, "linked"));

			LocalProtocolHandler.setOverride({
				getArtifactsDir: () => artifactsDir,
				getSessionId: () => "session-d",
			});
			const router = InternalUrlRouter.instance();
			await expect(router.resolve("local://linked/secret.txt")).rejects.toThrow("local:// URL escapes local root");
		});
	});

	it("prefers caller-supplied context.localProtocolOptions over the installed override", async () => {
		await withTempDir(async tempDir => {
			const overrideArtifactsDir = path.join(tempDir, "override-artifacts");
			const callerArtifactsDir = path.join(tempDir, "caller-artifacts");
			await fs.mkdir(path.join(overrideArtifactsDir, "local"), { recursive: true });
			await fs.mkdir(path.join(callerArtifactsDir, "local"), { recursive: true });
			await Bun.write(path.join(overrideArtifactsDir, "local", "PLAN.md"), "# wrong session");
			await Bun.write(path.join(callerArtifactsDir, "local", "PLAN.md"), "# caller session");

			// Process-global override points at the WRONG session (simulates a
			// stale override leaked from a prior subagent, or the multi-`main`
			// AgentRegistry case in cmux/ACP where "first one wins" lookup
			// picks a sibling session's artifacts dir — issue #1608).
			LocalProtocolHandler.setOverride({
				getArtifactsDir: () => overrideArtifactsDir,
				getSessionId: () => "stale-session",
			});

			const router = InternalUrlRouter.instance();
			const resource = await router.resolve("local://PLAN.md", {
				localProtocolOptions: {
					getArtifactsDir: () => callerArtifactsDir,
					getSessionId: () => "caller-session",
				},
			});

			const expectedSourcePath = await fs.realpath(path.join(callerArtifactsDir, "local", "PLAN.md"));

			expect(resource.content).toBe("# caller session");
			// `sourcePath` is canonicalized by the handler after symlink escape checks.
			// On macOS this may turn `/var/...` into `/private/var/...`.
			expect(resource.sourcePath).toBe(expectedSourcePath);
		});
	});

	it("surfaces ENOENT against the caller's local root when the file is missing in that session", async () => {
		await withTempDir(async tempDir => {
			const overrideArtifactsDir = path.join(tempDir, "override-artifacts");
			const callerArtifactsDir = path.join(tempDir, "caller-artifacts");
			await fs.mkdir(path.join(overrideArtifactsDir, "local"), { recursive: true });
			await fs.mkdir(path.join(callerArtifactsDir, "local"), { recursive: true });
			// PLAN.md exists only in the override-pointed session.
			await Bun.write(path.join(overrideArtifactsDir, "local", "PLAN.md"), "# wrong session");

			LocalProtocolHandler.setOverride({
				getArtifactsDir: () => overrideArtifactsDir,
				getSessionId: () => "stale-session",
			});

			const router = InternalUrlRouter.instance();
			await expect(
				router.resolve("local://PLAN.md", {
					localProtocolOptions: {
						getArtifactsDir: () => callerArtifactsDir,
						getSessionId: () => "caller-session",
					},
				}),
			).rejects.toThrow("Local file not found: local://PLAN.md");
		});
	});

	it("returns a notice instead of decoding a binary local file (issue #3449)", async () => {
		await withTempDir(async tempDir => {
			const artifactsDir = path.join(tempDir, "artifacts");
			const localFile = path.join(artifactsDir, "local", "attachment-1.mp4");
			await fs.mkdir(path.dirname(localFile), { recursive: true });
			// 1 MiB of structured non-text bytes (NUL in the head triggers the sniff).
			const buf = Buffer.alloc(1024 * 1024);
			for (let i = 0; i < buf.length; i += 4) {
				buf[i] = 0xff;
				buf[i + 1] = 0xfe;
				buf[i + 2] = 0x00;
				buf[i + 3] = i & 0xff;
			}
			await Bun.write(localFile, buf);

			LocalProtocolHandler.setOverride({
				getArtifactsDir: () => artifactsDir,
				getSessionId: () => "session-binary",
			});
			const router = InternalUrlRouter.instance();
			const resource = await router.resolve("local://attachment-1.mp4");

			// Notice replaces the file content; size matches the notice, not the
			// 1 MiB source — proves the binary was never decoded into mojibake.
			expect(resource.content).toContain("Cannot read binary local:// file");
			expect(resource.content).toContain("local://attachment-1.mp4");
			expect(resource.content).toContain("NUL bytes");
			expect(resource.size).toBe(Buffer.byteLength(resource.content, "utf-8"));
			expect(resource.size).toBeLessThan(1024);
			// `sourcePath` stays populated so find/search/path-utils still resolve.
			expect(resource.sourcePath).toBe(await fs.realpath(localFile));
			expect(resource.contentType).toBe("text/plain");
		});
	});

	it("returns a notice instead of materializing an oversized text local file (issue #3449)", async () => {
		await withTempDir(async tempDir => {
			const artifactsDir = path.join(tempDir, "artifacts");
			const localFile = path.join(artifactsDir, "local", "huge.log");
			await fs.mkdir(path.dirname(localFile), { recursive: true });
			// 11 MiB of plain ASCII — passes the binary sniff but trips the size cap.
			const chunk = "a".repeat(1024);
			const handle = await fs.open(localFile, "w");
			try {
				const lineBytes = Buffer.from(`${chunk}\n`, "utf-8");
				for (let i = 0; i < 11 * 1024; i++) {
					await handle.write(lineBytes);
				}
			} finally {
				await handle.close();
			}

			LocalProtocolHandler.setOverride({
				getArtifactsDir: () => artifactsDir,
				getSessionId: () => "session-large",
			});
			const router = InternalUrlRouter.instance();
			const resource = await router.resolve("local://huge.log");

			expect(resource.content).toContain("Cannot inline local:// file");
			expect(resource.content).toContain("local://huge.log");
			expect(resource.content).toContain("exceeds");
			expect(resource.content).toContain("inline limit");
			// Notice mentions a range-selector workaround so the agent knows what to try next.
			expect(resource.content).toMatch(/local:\/\/huge\.log:1-200/);
			expect(resource.size).toBeLessThan(1024);
			expect(resource.sourcePath).toBe(await fs.realpath(localFile));
		});
	});
});
