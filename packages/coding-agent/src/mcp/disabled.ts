import type { Settings } from "../config/settings";
import type { MCPServerConfig } from "./types";

/** Returns the disabledExtensions entry used to persist MCP server state. */
export function mcpDisabledExtensionId(name: string): string {
	return `mcp:${name}`;
}

/** Returns MCP server names disabled through the shared extension settings. */
export function getDisabledMcpServerNames(settings: Settings): Set<string> {
	const disabledExtensions = settings.get("disabledExtensions") ?? [];
	const names = new Set<string>();
	for (const id of disabledExtensions) {
		if (id.startsWith("mcp:")) {
			names.add(id.slice(4));
		}
	}
	return names;
}

/** Removes the legacy inline enabled flag before persisting an MCP server config. */
export function withoutInlineMcpEnabled(config: MCPServerConfig): MCPServerConfig {
	const { enabled: _enabled, ...stripped } = config;
	return stripped;
}

/** Checks whether a server is disabled through the shared extension settings. */
export function isMcpServerDisabled(settings: Settings, name: string): boolean {
	return settings.get("disabledExtensions")?.includes(mcpDisabledExtensionId(name)) ?? false;
}

/** Persists MCP server enabled state in disabledExtensions and reports whether it changed. */
export function setMcpServerDisabled(settings: Settings, name: string, disabled: boolean): boolean {
	const id = mcpDisabledExtensionId(name);
	const disabledExtensions = settings.get("disabledExtensions") ?? [];
	const isDisabled = disabledExtensions.includes(id);
	if (isDisabled === disabled) return false;

	const next = disabled
		? [...disabledExtensions, id].sort()
		: disabledExtensions.filter(disabledId => disabledId !== id);
	settings.set("disabledExtensions", next);
	return true;
}
