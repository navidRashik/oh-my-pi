/**
 * Builtin Defaults Provider
 *
 * Ships bundled language-specific TTSR rule packs embedded into the binary.
 * The provider defaults to workspace-aware loading so Rust/TypeScript rules do
 * not appear in unrelated projects, while project/user/tool rules with the same
 * `name` still override bundled copies through first-wins deduplication.
 *
 * Users control bundled rules three ways:
 *   - set `ttsr.builtinRuleMode: "always"` to force every bundled language pack,
 *   - set `ttsr.builtinRules: false` or `ttsr.builtinRuleMode: "off"` to drop the set,
 *   - list a name in `ttsr.disabledRules` to drop one rule after discovery.
 */
import { FileType, glob } from "@oh-my-pi/pi-natives";
import { registerProvider } from "../capability";
import { BUILTIN_DEFAULTS_PROVIDER_ID, type Rule, ruleCapability } from "../capability/rule";
import type { LoadContext, LoadResult } from "../capability/types";
import { BUILTIN_RULE_SOURCES, type BuiltinRuleLanguage } from "./builtin-rules";
import { buildRuleFromMarkdown, createSourceMeta } from "./helpers";

const DISPLAY_NAME = "Builtin Defaults";
// Lowest priority: every other rule provider wins a name conflict.
const PRIORITY = 1;
const AUTO_MODE = "auto";
const BUILTIN_LANGUAGES: readonly BuiltinRuleLanguage[] = ["rust", "typescript"];

const LANGUAGE_PATTERNS: Record<BuiltinRuleLanguage, string> = {
	rust: "**/*.rs",
	typescript: "**/*.{ts,tsx}",
};

async function hasWorkspaceFiles(ctx: LoadContext, language: BuiltinRuleLanguage): Promise<boolean> {
	const root = ctx.repoRoot ?? ctx.cwd;
	try {
		const result = await glob({
			pattern: LANGUAGE_PATTERNS[language],
			path: root,
			gitignore: true,
			hidden: false,
			fileType: FileType.File,
			maxResults: 1,
		});
		return result.matches.length > 0;
	} catch {
		return false;
	}
}

async function activeLanguages(ctx: LoadContext): Promise<Set<BuiltinRuleLanguage>> {
	const mode = ctx.builtinRuleMode ?? AUTO_MODE;
	if (mode === "off") return new Set<BuiltinRuleLanguage>();
	if (mode === "always") return new Set(BUILTIN_LANGUAGES);

	const entries = await Promise.all(
		BUILTIN_LANGUAGES.map(async language => ({
			language,
			active: await hasWorkspaceFiles(ctx, language),
		})),
	);
	return new Set(entries.filter(entry => entry.active).map(entry => entry.language));
}

async function loadRules(ctx: LoadContext): Promise<LoadResult<Rule>> {
	const languages = await activeLanguages(ctx);
	const items = BUILTIN_RULE_SOURCES.filter(({ language }) => languages.has(language)).map(({ name, content }) => {
		const virtualPath = `${BUILTIN_DEFAULTS_PROVIDER_ID}:${name}.md`;
		const source = createSourceMeta(BUILTIN_DEFAULTS_PROVIDER_ID, virtualPath, "user");
		return buildRuleFromMarkdown(name, content, virtualPath, source, { ruleName: name });
	});
	return { items };
}

registerProvider<Rule>(ruleCapability.id, {
	id: BUILTIN_DEFAULTS_PROVIDER_ID,
	displayName: DISPLAY_NAME,
	description: "Workspace-aware language rule packs shipped with the agent",
	priority: PRIORITY,
	load: loadRules,
});
