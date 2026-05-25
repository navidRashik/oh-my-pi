/**
 * Plugin loader - discovers and loads manifest entry points from installed plugins.
 *
 * Reads enabled plugins from the runtime config and loads their
 * tools/hooks/extensions/commands based on manifest entries and enabled features.
 */
import * as fs from "node:fs";
import * as path from "node:path";
import { getPluginsLockfile, getPluginsNodeModules, getPluginsPackageJson, isEnoent } from "@oh-my-pi/pi-utils";
import { getConfigDirPaths } from "../../config";
import { installLegacyPiSpecifierShim } from "./legacy-pi-compat";
import type { InstalledPlugin, PluginManifest, PluginRuntimeConfig, ProjectPluginOverrides } from "./types";

installLegacyPiSpecifierShim();

// =============================================================================
// Runtime Config Loading
// =============================================================================

/**
 * Load plugin runtime config from lock file.
 */
async function loadRuntimeConfig(): Promise<PluginRuntimeConfig> {
	const lockPath = getPluginsLockfile();
	try {
		return await Bun.file(lockPath).json();
	} catch (err) {
		if (isEnoent(err)) return { plugins: {}, settings: {} };
		throw err;
	}
}

/**
 * Load project-local plugin overrides (checks .omp and .pi directories).
 */
async function loadProjectOverrides(cwd: string): Promise<ProjectPluginOverrides> {
	for (const overridesPath of getConfigDirPaths("plugin-overrides.json", { user: false, cwd })) {
		try {
			return await Bun.file(overridesPath).json();
		} catch (err) {
			if (isEnoent(err)) continue;
			// JSON parse error - continue to next path
		}
	}
	return {};
}
/**
 * Get list of enabled plugins with their resolved configurations.
 * Respects both global runtime config and project overrides.
 */
export async function getEnabledPlugins(cwd: string): Promise<InstalledPlugin[]> {
	const pkgJsonPath = getPluginsPackageJson();
	let pkg: { dependencies?: Record<string, string> };
	try {
		pkg = await Bun.file(pkgJsonPath).json();
	} catch (err) {
		if (isEnoent(err)) return [];
		throw err;
	}

	const nodeModulesPath = getPluginsNodeModules();
	if (!fs.existsSync(nodeModulesPath)) {
		return [];
	}

	const deps = pkg.dependencies || {};
	const runtimeConfig = await loadRuntimeConfig();
	const projectOverrides = await loadProjectOverrides(cwd);
	const plugins: InstalledPlugin[] = [];
	for (const [name] of Object.entries(deps)) {
		const pluginPkgPath = path.join(nodeModulesPath, name, "package.json");
		let pluginPkg: { version: string; omp?: PluginManifest; pi?: PluginManifest };
		try {
			pluginPkg = await Bun.file(pluginPkgPath).json();
		} catch (err) {
			if (isEnoent(err)) continue;
			throw err;
		}

		const manifest: PluginManifest | undefined = pluginPkg.omp || pluginPkg.pi;

		if (!manifest) {
			// Not an omp plugin, skip
			continue;
		}

		manifest.version = pluginPkg.version;

		const runtimeState = runtimeConfig.plugins[name];

		// Check if disabled globally
		if (runtimeState && !runtimeState.enabled) {
			continue;
		}

		// Check if disabled in project
		if (projectOverrides.disabled?.includes(name)) {
			continue;
		}

		// Resolve enabled features (project overrides take precedence)
		const enabledFeatures = projectOverrides.features?.[name] ?? runtimeState?.enabledFeatures ?? null;
		plugins.push({
			name,
			version: pluginPkg.version,
			path: path.join(nodeModulesPath, name),
			manifest,
			enabledFeatures,
			enabled: true,
		});
	}

	return plugins;
}

// =============================================================================
// Path Resolution
// =============================================================================

const MANIFEST_ENTRY_INDEX_NAMES = ["index.ts", "index.js", "index.mjs", "index.cjs"];

/**
 * Resolve a plugin manifest entry to one or more concrete loadable file paths.
 *
 * - A file entry resolves to itself.
 * - A directory entry resolves to its `index.{ts,js,mjs,cjs}` when present.
 * - When `expandDirectory` is set and the directory has no top-level index,
 *   the directory's immediate children are scanned for entry points: direct
 *   `.ts`/`.js`/`.mjs`/`.cjs` files and subdirectories containing an index
 *   file. Mirrors the agent's own `discoverExtensionsInDir` rules so that
 *   plugins declaring `pi.extensions: ["./extensions"]` and laying out
 *   concrete extensions under `./extensions/<name>/index.ts` resolve to
 *   those nested entries instead of silently dropping to zero.
 *
 * Returns an empty array when no entry exists at the joined path.
 */
function resolveManifestEntries(joined: string, options: { expandDirectory: boolean }): string[] {
	let stats: fs.Stats;
	try {
		stats = fs.statSync(joined);
	} catch {
		return [];
	}
	if (!stats.isDirectory()) {
		return [joined];
	}
	for (const name of MANIFEST_ENTRY_INDEX_NAMES) {
		const candidate = path.join(joined, name);
		if (fs.existsSync(candidate)) return [candidate];
	}
	if (!options.expandDirectory) return [];

	let entries: fs.Dirent[];
	try {
		entries = fs.readdirSync(joined, { withFileTypes: true });
	} catch {
		return [];
	}
	const results: string[] = [];
	for (const entry of entries) {
		const entryPath = path.join(joined, entry.name);
		if ((entry.isFile() || entry.isSymbolicLink()) && isExtensionEntryFile(entry.name)) {
			results.push(entryPath);
			continue;
		}
		if (entry.isDirectory() || entry.isSymbolicLink()) {
			for (const name of MANIFEST_ENTRY_INDEX_NAMES) {
				const candidate = path.join(entryPath, name);
				if (fs.existsSync(candidate)) {
					results.push(candidate);
					break;
				}
			}
		}
	}
	return results;
}

function isExtensionEntryFile(name: string): boolean {
	return name.endsWith(".ts") || name.endsWith(".js") || name.endsWith(".mjs") || name.endsWith(".cjs");
}

/**
 * Generic path resolver for plugin manifest entries (tools, hooks, commands, extensions).
 * Handles both single-string and string[] base entries, plus feature-specific entries.
 */
function resolvePluginPaths(plugin: InstalledPlugin, key: "tools" | "hooks" | "commands" | "extensions"): string[] {
	const paths: string[] = [];
	const manifest = plugin.manifest;
	// Directory-of-extensions expansion is extensions-only; tools/hooks/commands
	// are addressed by single explicit files in the manifest.
	const expandDirectory = key === "extensions";

	// Base entry (always included if exists)
	const base = manifest[key];
	if (base) {
		const entries = Array.isArray(base) ? base : [base];
		for (const entry of entries) {
			paths.push(...resolveManifestEntries(path.join(plugin.path, entry), { expandDirectory }));
		}
	}

	// Feature-specific entries
	if (manifest.features && plugin.enabledFeatures) {
		const enabledSet = new Set(plugin.enabledFeatures);

		for (const [featName, feat] of Object.entries(manifest.features)) {
			if (!enabledSet.has(featName)) continue;

			if (feat[key]) {
				for (const entry of feat[key]) {
					paths.push(...resolveManifestEntries(path.join(plugin.path, entry), { expandDirectory }));
				}
			}
		}
	} else if (manifest.features && plugin.enabledFeatures === null) {
		// null means use defaults - enable features with default: true
		for (const [_featName, feat] of Object.entries(manifest.features)) {
			if (!feat.default) continue;

			if (feat[key]) {
				for (const entry of feat[key]) {
					paths.push(...resolveManifestEntries(path.join(plugin.path, entry), { expandDirectory }));
				}
			}
		}
	}

	return paths;
}

export function resolvePluginToolPaths(plugin: InstalledPlugin): string[] {
	return resolvePluginPaths(plugin, "tools");
}

export function resolvePluginHookPaths(plugin: InstalledPlugin): string[] {
	return resolvePluginPaths(plugin, "hooks");
}

export function resolvePluginCommandPaths(plugin: InstalledPlugin): string[] {
	return resolvePluginPaths(plugin, "commands");
}

export function resolvePluginExtensionPaths(plugin: InstalledPlugin): string[] {
	return resolvePluginPaths(plugin, "extensions");
}

// =============================================================================
// Aggregated Discovery
// =============================================================================

/**
 * Get all tool paths from all enabled plugins.
 */
export async function getAllPluginToolPaths(cwd: string): Promise<string[]> {
	const plugins = await getEnabledPlugins(cwd);
	const paths: string[] = [];

	for (const plugin of plugins) {
		paths.push(...resolvePluginToolPaths(plugin));
	}

	return paths;
}

/**
 * Get all hook paths from all enabled plugins.
 */
export async function getAllPluginHookPaths(cwd: string): Promise<string[]> {
	const plugins = await getEnabledPlugins(cwd);
	const paths: string[] = [];

	for (const plugin of plugins) {
		paths.push(...resolvePluginHookPaths(plugin));
	}

	return paths;
}

/**
 * Get all command paths from all enabled plugins.
 */
export async function getAllPluginCommandPaths(cwd: string): Promise<string[]> {
	const plugins = await getEnabledPlugins(cwd);
	const paths: string[] = [];

	for (const plugin of plugins) {
		paths.push(...resolvePluginCommandPaths(plugin));
	}

	return paths;
}

/**
 * Get all extension module paths from all enabled plugins.
 */
export async function getAllPluginExtensionPaths(cwd: string): Promise<string[]> {
	const plugins = await getEnabledPlugins(cwd);
	const paths: string[] = [];

	for (const plugin of plugins) {
		paths.push(...resolvePluginExtensionPaths(plugin));
	}

	return paths;
}

/**
 * Get plugin settings for use in tool/hook contexts.
 * Merges global settings with project overrides.
 */
export async function getPluginSettings(pluginName: string, cwd: string): Promise<Record<string, unknown>> {
	const runtimeConfig = await loadRuntimeConfig();
	const projectOverrides = await loadProjectOverrides(cwd);

	const global = runtimeConfig.settings[pluginName] || {};
	const project = projectOverrides.settings?.[pluginName] || {};

	return { ...global, ...project };
}
