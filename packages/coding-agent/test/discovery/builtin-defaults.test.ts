/**
 * The bundled `builtin-defaults` rule provider ships workspace-aware language
 * rule packs embedded into the binary. These tests defend language gating,
 * parsing, and provider priority.
 */

import { afterEach, describe, expect, it } from "bun:test";
import * as fs from "node:fs/promises";
import * as os from "node:os";
import * as path from "node:path";
import { getCapability } from "@oh-my-pi/pi-coding-agent/capability";
import { BUILTIN_DEFAULTS_PROVIDER_ID, type Rule, ruleCapability } from "@oh-my-pi/pi-coding-agent/capability/rule";
import type { LoadContext } from "@oh-my-pi/pi-coding-agent/capability/types";
// Register all discovery providers as a side effect.
import "@oh-my-pi/pi-coding-agent/discovery";

const EXPECTED_RULE_NAMES = [
	"rs-box-leak",
	"rs-future-prelude",
	"rs-lazylock",
	"rs-match-ergonomics",
	"rs-parking-lot",
	"rs-result-type",
	"ts-bare-catch",
	"ts-import-type",
	"ts-no-any",
	"ts-no-deprecated-leftovers",
	"ts-no-dynamic-import",
	"ts-no-return-type",
	"ts-no-tiny-functions",
	"ts-promise-with-resolvers",
	"ts-set-map",
].sort();

const tempDirs: string[] = [];

async function makeWorkspace(files: readonly string[] = []): Promise<string> {
	const dir = await fs.mkdtemp(path.join(os.tmpdir(), "omp-builtin-rules-"));
	tempDirs.push(dir);
	for (const file of files) {
		const filePath = path.join(dir, file);
		await fs.mkdir(path.dirname(filePath), { recursive: true });
		await Bun.write(filePath, "");
	}
	return dir;
}

afterEach(async () => {
	const dirs = tempDirs.splice(0);
	await Promise.all(dirs.map(dir => fs.rm(dir, { recursive: true, force: true })));
});

function ruleProvider() {
	const cap = getCapability<Rule>(ruleCapability.id);
	if (!cap) throw new Error("rules capability missing");
	const provider = cap.providers.find(p => p.id === BUILTIN_DEFAULTS_PROVIDER_ID);
	if (!provider) throw new Error("builtin-defaults provider missing");
	return { cap, provider };
}

async function loadBuiltinRules(ctx: LoadContext): Promise<Rule[]> {
	const { provider } = ruleProvider();
	const result = await provider.load(ctx);
	return result.items;
}

async function loadFromWorkspace(
	files: readonly string[] = [],
	mode: LoadContext["builtinRuleMode"] = "auto",
): Promise<Rule[]> {
	const cwd = await makeWorkspace(files);
	return await loadBuiltinRules({ cwd, home: cwd, repoRoot: null, builtinRuleMode: mode });
}

describe("builtin-defaults rule provider", () => {
	it("loads every bundled rule when explicitly forced on", async () => {
		const rules = await loadFromWorkspace([], "always");
		const names = rules.map(r => r.name).sort();
		expect(names).toEqual(EXPECTED_RULE_NAMES);
		expect(rules.every(r => r._source.provider === BUILTIN_DEFAULTS_PROVIDER_ID)).toBe(true);
	});

	it("loads no bundled language rules for unrelated workspaces in auto mode", async () => {
		const rules = await loadFromWorkspace(["app/main.py"]);
		expect(rules).toEqual([]);
	});

	it("loads only language rule packs with matching workspace files in auto mode", async () => {
		const tsRules = await loadFromWorkspace(["src/index.ts"]);
		expect(tsRules.map(r => r.name).sort()).toEqual(EXPECTED_RULE_NAMES.filter(name => name.startsWith("ts-")));

		const rustRules = await loadFromWorkspace(["src/main.rs"]);
		expect(rustRules.map(r => r.name).sort()).toEqual(EXPECTED_RULE_NAMES.filter(name => name.startsWith("rs-")));
	});

	it("parses every bundled rule as a TTSR rule (non-empty condition and scope)", async () => {
		const rules = await loadFromWorkspace([], "always");
		for (const rule of rules) {
			expect(rule.condition?.length, `${rule.name} condition`).toBeGreaterThan(0);
			expect(rule.scope?.length, `${rule.name} scope`).toBeGreaterThan(0);
		}
	});

	it("parses YAML list-form conditions from the embedded text", async () => {
		const rules = await loadFromWorkspace([], "always");
		const lazylock = rules.find(r => r.name === "rs-lazylock");
		// Frontmatter declares two condition patterns as a YAML sequence.
		expect(lazylock?.condition).toHaveLength(2);
	});

	it("preserves a per-rule interruptMode override from frontmatter", async () => {
		const rules = await loadFromWorkspace([], "always");
		expect(rules.find(r => r.name === "ts-set-map")?.interruptMode).toBe("never");
	});

	it("is the lowest-priority rule provider so user/project rules override defaults", () => {
		const { cap, provider } = ruleProvider();
		const others = cap.providers.filter(p => p.id !== BUILTIN_DEFAULTS_PROVIDER_ID);
		expect(others.length).toBeGreaterThan(0);
		expect(others.every(p => p.priority > provider.priority)).toBe(true);
	});
});
