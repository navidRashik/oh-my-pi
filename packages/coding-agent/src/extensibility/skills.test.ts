import { afterEach, describe, expect, it } from "bun:test";
import * as fs from "node:fs/promises";
import * as os from "node:os";
import * as path from "node:path";
import { buildSkillCommandPrompt } from "../modes/skill-command";
import { buildSkillPromptMessage, type Skill } from "./skills";

const tempDirs: string[] = [];

afterEach(async () => {
	for (const dir of tempDirs.splice(0)) {
		await fs.rm(dir, { recursive: true, force: true });
	}
});

describe("buildSkillPromptMessage", () => {
	it("frames user-invoked skills and exposes their directory", async () => {
		const baseDir = await fs.mkdtemp(path.join(os.tmpdir(), "omp-skill-user-"));
		tempDirs.push(baseDir);
		const filePath = path.join(baseDir, "SKILL.md");
		await Bun.write(
			filePath,
			"---\ndescription: Deploy helpers\n---\n\nRun `scripts/deploy.js` with `templates/config.yaml`.",
		);
		const skill: Skill = { name: "deploy", description: "Deploy helpers", filePath, baseDir, source: "test" };

		const built = await buildSkillPromptMessage(skill, "prod");

		expect(built.message).toStartWith(
			'[IMPORTANT: The user has invoked the "deploy" skill, indicating they want you to follow its instructions. The full skill content is loaded below.]',
		);
		expect(built.message).toContain("Run `scripts/deploy.js` with `templates/config.yaml`.");
		expect(built.message).toContain(`[Skill directory: ${baseDir}]`);
		expect(built.message).toContain("Resolve any relative paths in this skill");
		expect(built.message).toContain("User: prod");
		expect(built.message).not.toContain("description: Deploy helpers");
	});

	it("keeps autoload skills on the non-user metadata format", async () => {
		const baseDir = await fs.mkdtemp(path.join(os.tmpdir(), "omp-skill-autoload-"));
		tempDirs.push(baseDir);
		const filePath = path.join(baseDir, "SKILL.md");
		await Bun.write(filePath, "Hidden instructions.");
		const skill: Skill = { name: "hidden", description: "Hidden", filePath, baseDir, source: "test" };

		const built = await buildSkillPromptMessage(skill, "", "autoload");

		expect(built.message).toBe(`Hidden instructions.\n\n---\n\nSkill: ${filePath}`);
	});
});

describe("buildSkillCommandPrompt", () => {
	it("uses the registered skill object for slash-command prompts", async () => {
		const baseDir = await fs.mkdtemp(path.join(os.tmpdir(), "omp-skill-command-"));
		tempDirs.push(baseDir);
		const filePath = path.join(baseDir, "SKILL.md");
		await Bun.write(filePath, "Read `scripts/check.js` before responding.");
		const skill: Skill = { name: "check", description: "Check", filePath, baseDir, source: "test" };

		const built = await buildSkillCommandPrompt(
			{ skillCommands: new Map([["skill:check", skill]]) },
			"/skill:check now",
			"followUp",
		);

		expect(built?.message.content).toContain('[IMPORTANT: The user has invoked the "check" skill');
		expect(built?.message.content).toContain(`[Skill directory: ${baseDir}]`);
		expect(built?.message.details).toEqual({
			name: "check",
			path: filePath,
			args: "now",
			lineCount: 1,
		});
	});
});
