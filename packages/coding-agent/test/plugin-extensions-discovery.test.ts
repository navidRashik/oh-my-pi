import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { discoverAndLoadExtensions } from "@oh-my-pi/pi-coding-agent/extensibility/extensions/loader";
import { getAgentDir, getPluginsDir, setAgentDir, TempDir } from "@oh-my-pi/pi-utils";

const currentPiCodingAgentPath = Bun.resolveSync("@oh-my-pi/pi-coding-agent", import.meta.dir);
const currentPiExtensionsPath = Bun.resolveSync("@oh-my-pi/pi-coding-agent/extensibility/extensions", import.meta.dir);

describe("plugin extension discovery", () => {
	let projectDir: TempDir;
	let tempXdgDataHome = "";
	let originalXdgDataHome: string | undefined;
	const originalAgentDir = getAgentDir();

	beforeEach(() => {
		projectDir = TempDir.createSync("@pi-plugin-ext-");
		originalXdgDataHome = process.env.XDG_DATA_HOME;
		tempXdgDataHome = fs.mkdtempSync(path.join(os.tmpdir(), "pi-plugin-data-"));
		fs.mkdirSync(path.join(tempXdgDataHome, "omp"), { recursive: true });
		process.env.XDG_DATA_HOME = tempXdgDataHome;
		// Rebuild path caches after changing XDG env so plugin discovery resolves into the temp root.
		setAgentDir(originalAgentDir);

		const pluginsDir = getPluginsDir();
		const pluginDir = path.join(pluginsDir, "node_modules", "@demo", "plugin");
		fs.mkdirSync(path.join(pluginDir, "dist"), { recursive: true });
		fs.writeFileSync(
			path.join(pluginsDir, "package.json"),
			JSON.stringify({
				name: "omp-plugins",
				private: true,
				dependencies: {
					"@demo/plugin": "1.0.0",
				},
			}),
		);
		fs.writeFileSync(
			path.join(pluginDir, "package.json"),
			JSON.stringify({
				name: "@demo/plugin",
				version: "1.0.0",
				omp: {
					extensions: ["./dist/extension.ts"],
				},
			}),
		);
		fs.writeFileSync(
			path.join(pluginDir, "dist", "extension.ts"),
			`
				export default function(pi) {
					pi.registerCommand("plugin-ext", { handler: async () => {} });
				}
			`,
		);
	});

	afterEach(() => {
		projectDir.removeSync();
		fs.rmSync(tempXdgDataHome, { recursive: true, force: true });
		if (originalXdgDataHome === undefined) {
			delete process.env.XDG_DATA_HOME;
		} else {
			process.env.XDG_DATA_HOME = originalXdgDataHome;
		}
		setAgentDir(originalAgentDir);
	});

	it("loads installed plugin extensions declared in package.json", async () => {
		const result = await discoverAndLoadExtensions([], projectDir.path());
		const extension = result.extensions.find(ext => ext.path.endsWith(path.join("dist", "extension.ts")));

		expect(result.errors).toHaveLength(0);
		expect(extension).toBeDefined();
		expect(extension?.commands.has("plugin-ext")).toBe(true);
	});

	it("loads installed legacy Pi plugin extensions from Windows drive-letter paths", async () => {
		const pluginsDir = getPluginsDir();
		const pluginDir = path.join(pluginsDir, "node_modules", "legacy-pi-plugin");
		const extensionPath = path.join(pluginDir, "dist", "extension.ts");
		fs.rmSync(path.join(pluginsDir, "node_modules"), { recursive: true, force: true });
		fs.mkdirSync(path.dirname(extensionPath), { recursive: true });
		fs.writeFileSync(
			path.join(pluginsDir, "package.json"),
			JSON.stringify({
				name: "omp-plugins",
				private: true,
				dependencies: {
					"legacy-pi-plugin": "1.0.0",
				},
			}),
		);
		fs.writeFileSync(
			path.join(pluginDir, "package.json"),
			JSON.stringify({
				name: "legacy-pi-plugin",
				version: "1.0.0",
				pi: {
					extensions: ["./dist/extension.ts"],
				},
			}),
		);
		fs.writeFileSync(
			extensionPath,
			[
				'import * as nodePath from "path";',
				'if (false) import("./optional-missing.js");',
				'import { isToolCallEventType as legacyRoot } from "@mariozechner/pi-coding-agent";',
				'import { isToolCallEventType as legacyExtensions } from "@mariozechner/pi-coding-agent/extensibility/extensions";',
				`import { isToolCallEventType as modernRoot } from ${JSON.stringify(currentPiCodingAgentPath)};`,
				`import { isToolCallEventType as modernExtensions } from ${JSON.stringify(currentPiExtensionsPath)};`,
				"",
				'if (legacyRoot !== modernRoot) throw new Error("legacy root import did not remap");',
				'if (legacyExtensions !== modernExtensions) throw new Error("legacy extension import did not remap");',
				'if (typeof nodePath.join !== "function") throw new Error("node builtin import did not resolve");',
				"",
				"export default function(pi) {",
				"\tconst { Type } = pi.typebox;",
				"\tpi.registerTool({",
				'\t\tname: "legacy-pi-ext",',
				'\t\tdescription: "Legacy Pi extension smoke test",',
				"\t\tparameters: Type.Object({}),",
				'\t\texecute: async () => ({ content: [{ type: "text", text: "ok" }] }),',
				"\t});",
				"}",
			].join("\n"),
		);

		const result = await discoverAndLoadExtensions([], projectDir.path());
		const extension = result.extensions.find(ext => ext.path === extensionPath);

		if (process.platform === "win32") {
			expect(extensionPath).toMatch(/^[A-Za-z]:\\/);
		}
		expect(result.errors).toHaveLength(0);
		expect(extension).toBeDefined();
		expect(extension?.tools.has("legacy-pi-ext")).toBe(true);
	});

	it("loads installed plugin extensions whose manifest entry points at a directory with index.ts", async () => {
		const pluginsDir = getPluginsDir();
		const pluginDir = path.join(pluginsDir, "node_modules", "dir-entry-plugin");
		const extensionDir = path.join(pluginDir, ".pi", "extensions", "dir-entry");
		const extensionPath = path.join(extensionDir, "index.ts");
		fs.rmSync(path.join(pluginsDir, "node_modules"), { recursive: true, force: true });
		fs.mkdirSync(extensionDir, { recursive: true });
		fs.writeFileSync(
			path.join(pluginsDir, "package.json"),
			JSON.stringify({
				name: "omp-plugins",
				private: true,
				dependencies: {
					"dir-entry-plugin": "1.0.0",
				},
			}),
		);
		fs.writeFileSync(
			path.join(pluginDir, "package.json"),
			JSON.stringify({
				name: "dir-entry-plugin",
				version: "1.0.0",
				pi: {
					// Directory entry — loader must resolve to the directory's index file.
					extensions: [".pi/extensions/dir-entry"],
				},
			}),
		);
		fs.writeFileSync(
			extensionPath,
			[
				"export default function(pi) {",
				'\tpi.registerCommand("dir-entry-ext", { handler: async () => {} });',
				"}",
			].join("\n"),
		);

		const result = await discoverAndLoadExtensions([], projectDir.path());
		const extension = result.extensions.find(ext => ext.path === extensionPath);
		const pluginError = result.errors.find(err => err.path.includes(path.join("dir-entry-plugin", ".pi")));

		expect(pluginError).toBeUndefined();
		expect(extension).toBeDefined();
		expect(extension?.commands.has("dir-entry-ext")).toBe(true);
	});
	it("loads installed plugin extensions whose manifest entry points at a directory of subdirectory extensions", async () => {
		// `pi-autoresearch` layout (issue #1292): `pi.extensions: ["./extensions"]`
		// where `extensions/` has no top-level `index.{ts,js,...}` but contains
		// `extensions/<name>/index.ts` per nested extension. Loader must expand
		// the directory to those nested entry points instead of silently dropping
		// the plugin to zero extensions.
		const pluginsDir = getPluginsDir();
		const pluginDir = path.join(pluginsDir, "node_modules", "pi-autoresearch");
		const nestedExtensionPath = path.join(pluginDir, "extensions", "pi-autoresearch", "index.ts");
		const siblingExtensionPath = path.join(pluginDir, "extensions", "sibling", "index.ts");
		fs.rmSync(path.join(pluginsDir, "node_modules"), { recursive: true, force: true });
		fs.mkdirSync(path.dirname(nestedExtensionPath), { recursive: true });
		fs.mkdirSync(path.dirname(siblingExtensionPath), { recursive: true });
		fs.writeFileSync(
			path.join(pluginsDir, "package.json"),
			JSON.stringify({
				name: "omp-plugins",
				private: true,
				dependencies: {
					"pi-autoresearch": "1.0.0",
				},
			}),
		);
		fs.writeFileSync(
			path.join(pluginDir, "package.json"),
			JSON.stringify({
				name: "pi-autoresearch",
				version: "1.0.0",
				pi: {
					// Directory entry without top-level index — the loader must
					// expand it to the nested subdirectory extensions.
					extensions: ["./extensions"],
				},
			}),
		);
		fs.writeFileSync(
			nestedExtensionPath,
			[
				"export default function(pi) {",
				'\tpi.registerCommand("autoresearch-ext", { handler: async () => {} });',
				"}",
			].join("\n"),
		);
		fs.writeFileSync(
			siblingExtensionPath,
			[
				"export default function(pi) {",
				'\tpi.registerCommand("autoresearch-sibling", { handler: async () => {} });',
				"}",
			].join("\n"),
		);

		const result = await discoverAndLoadExtensions([], projectDir.path());
		const nested = result.extensions.find(ext => ext.path === nestedExtensionPath);
		const sibling = result.extensions.find(ext => ext.path === siblingExtensionPath);

		expect(result.errors).toEqual([]);
		expect(nested).toBeDefined();
		expect(nested?.commands.has("autoresearch-ext")).toBe(true);
		expect(sibling).toBeDefined();
		expect(sibling?.commands.has("autoresearch-sibling")).toBe(true);
	});
});
