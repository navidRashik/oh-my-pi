/**
 * Bundled default rules shipped with the coding agent.
 *
 * Each markdown source is embedded via `with { type: "text" }` so it survives
 * `bun build --compile` (the compiled binary ships no loose rule files; only
 * the embedded text). The native source/tarball installs read the same modules.
 *
 * Registered by the lowest-priority `builtin-defaults` rule provider so any
 * user/project/tool rule with the same name overrides the bundled copy.
 */
import rsBoxLeak from "./rs-box-leak.md" with { type: "text" };
import rsFuturePrelude from "./rs-future-prelude.md" with { type: "text" };
import rsLazylock from "./rs-lazylock.md" with { type: "text" };
import rsMatchErgonomics from "./rs-match-ergonomics.md" with { type: "text" };
import rsParkingLot from "./rs-parking-lot.md" with { type: "text" };
import rsResultType from "./rs-result-type.md" with { type: "text" };
import tsBareCatch from "./ts-bare-catch.md" with { type: "text" };
import tsImportType from "./ts-import-type.md" with { type: "text" };
import tsNoAny from "./ts-no-any.md" with { type: "text" };
import tsNoDeprecatedLeftovers from "./ts-no-deprecated-leftovers.md" with { type: "text" };
import tsNoDynamicImport from "./ts-no-dynamic-import.md" with { type: "text" };
import tsNoReturnType from "./ts-no-return-type.md" with { type: "text" };
import tsNoTinyFunctions from "./ts-no-tiny-functions.md" with { type: "text" };
import tsPromiseWithResolvers from "./ts-promise-with-resolvers.md" with { type: "text" };
import tsSetMap from "./ts-set-map.md" with { type: "text" };

/** Language family a bundled rule pack applies to. */
export type BuiltinRuleLanguage = "rust" | "typescript";

/** A bundled rule's stable name, language family, and raw markdown. */
export interface BuiltinRuleSource {
	name: string;
	language: BuiltinRuleLanguage;
	content: string;
}

/** All bundled default rules, ordered by name. */
export const BUILTIN_RULE_SOURCES: readonly BuiltinRuleSource[] = [
	{ name: "rs-box-leak", language: "rust", content: rsBoxLeak },
	{ name: "rs-future-prelude", language: "rust", content: rsFuturePrelude },
	{ name: "rs-lazylock", language: "rust", content: rsLazylock },
	{ name: "rs-match-ergonomics", language: "rust", content: rsMatchErgonomics },
	{ name: "rs-parking-lot", language: "rust", content: rsParkingLot },
	{ name: "rs-result-type", language: "rust", content: rsResultType },
	{ name: "ts-bare-catch", language: "typescript", content: tsBareCatch },
	{ name: "ts-import-type", language: "typescript", content: tsImportType },
	{ name: "ts-no-any", language: "typescript", content: tsNoAny },
	{ name: "ts-no-deprecated-leftovers", language: "typescript", content: tsNoDeprecatedLeftovers },
	{ name: "ts-no-dynamic-import", language: "typescript", content: tsNoDynamicImport },
	{ name: "ts-no-return-type", language: "typescript", content: tsNoReturnType },
	{ name: "ts-no-tiny-functions", language: "typescript", content: tsNoTinyFunctions },
	{ name: "ts-promise-with-resolvers", language: "typescript", content: tsPromiseWithResolvers },
	{ name: "ts-set-map", language: "typescript", content: tsSetMap },
];
