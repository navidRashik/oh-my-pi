import { afterEach, describe, expect, it, vi } from "bun:test";
import * as childProcess from "node:child_process";
import * as fs from "node:fs/promises";
import * as os from "node:os";
import * as path from "node:path";
import { getEditorCommand, openInEditor } from "../src/utils/external-editor";

interface MutableProcess {
	platform: NodeJS.Platform;
}

interface SpawnCall {
	editor: string;
	args: string[];
	options: childProcess.SpawnOptions;
}

function setPlatform(value: NodeJS.Platform): void {
	(process as unknown as MutableProcess).platform = value;
}

function spyEditorSpawn(calls: SpawnCall[]) {
	return vi.spyOn(childProcess, "spawn").mockImplementation(((
		editor: string,
		args?: readonly string[],
		options?: childProcess.SpawnOptions,
	) => {
		calls.push({ editor, args: [...(args ?? [])], options: options ?? {} });
		const child = new childProcess.ChildProcess();
		queueMicrotask(() => child.emit("exit", 0, null));
		return child;
	}) as typeof childProcess.spawn);
}

const originalPlatform = process.platform;
const originalVisual = Bun.env.VISUAL;
const originalEditor = Bun.env.EDITOR;

afterEach(() => {
	setPlatform(originalPlatform);
	if (originalVisual === undefined) delete Bun.env.VISUAL;
	else Bun.env.VISUAL = originalVisual;
	if (originalEditor === undefined) delete Bun.env.EDITOR;
	else Bun.env.EDITOR = originalEditor;
	vi.restoreAllMocks();
});

describe("getEditorCommand", () => {
	it("prefers $VISUAL over $EDITOR and the platform default", () => {
		Bun.env.VISUAL = "nvim";
		Bun.env.EDITOR = "nano";
		setPlatform("win32");
		expect(getEditorCommand()).toBe("nvim");
	});

	it("falls back to $EDITOR when $VISUAL is unset", () => {
		delete Bun.env.VISUAL;
		Bun.env.EDITOR = "nano";
		expect(getEditorCommand()).toBe("nano");
	});

	it("trims whitespace so an accidentally padded value still works", () => {
		Bun.env.VISUAL = "  code --wait  ";
		delete Bun.env.EDITOR;
		expect(getEditorCommand()).toBe("code --wait");
	});

	it("treats a whitespace-only $VISUAL as unset and consults $EDITOR", () => {
		Bun.env.VISUAL = "   ";
		Bun.env.EDITOR = "vim";
		expect(getEditorCommand()).toBe("vim");
	});

	it("defaults to notepad on Windows when neither variable is set", () => {
		delete Bun.env.VISUAL;
		delete Bun.env.EDITOR;
		setPlatform("win32");
		expect(getEditorCommand()).toBe("notepad");
	});

	it("returns undefined on POSIX when neither variable is set", () => {
		delete Bun.env.VISUAL;
		delete Bun.env.EDITOR;
		setPlatform("linux");
		expect(getEditorCommand()).toBeUndefined();
	});
});

describe("openInEditor", () => {
	it("launches built-in Notepad without the Windows shell so spaced temp paths stay intact", async () => {
		const calls: SpawnCall[] = [];
		spyEditorSpawn(calls);
		setPlatform("win32");
		const tempRoot = path.join(os.tmpdir(), "omp editor Jane Doe");
		await fs.rm(tempRoot, { force: true, recursive: true });
		vi.spyOn(os, "tmpdir").mockReturnValue(tempRoot);

		const result = await openInEditor("notepad", "draft");

		expect(result).toBe("draft");
		expect(calls).toHaveLength(1);
		expect(calls[0]?.editor).toBe("notepad");
		expect(calls[0]?.options.shell).toBe(false);
		expect(calls[0]?.args.at(-1)).toStartWith(tempRoot);
		await fs.rm(tempRoot, { force: true, recursive: true });
	});
});
