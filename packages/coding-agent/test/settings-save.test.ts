import { afterEach, describe, expect, it } from "bun:test";
import * as fs from "node:fs/promises";
import * as os from "node:os";
import * as path from "node:path";
import { resetSettingsForTest, Settings } from "../src/config/settings";

const tempDirs: string[] = [];

async function makeTempDir(): Promise<string> {
	const dir = await fs.mkdtemp(path.join(os.tmpdir(), "omp-settings-save-test-"));
	tempDirs.push(dir);
	return dir;
}

afterEach(async () => {
	resetSettingsForTest();
	await Promise.all(tempDirs.splice(0).map(dir => fs.rm(dir, { recursive: true, force: true })));
});

describe("settings config.yml persistence", () => {
	it("writes nested root settings without trailing spaces", async () => {
		const agentDir = await makeTempDir();
		const settings = await Settings.init({ agentDir, cwd: agentDir });

		settings.set("theme.dark", "titanium");
		settings.set("edit.mode", "hashline");
		await settings.flush();

		const content = await Bun.file(path.join(agentDir, "config.yml")).text();

		expect(content).toContain("theme:\n  dark: titanium\n");
		expect(content).toContain("edit:\n  mode: hashline");
		expect(content).not.toMatch(/[ \t]+$/m);
	});
});
