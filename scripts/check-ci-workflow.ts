import { Glob } from "bun";

const WORKFLOW_PATH = ".github/workflows/ci.yml";
const COMPOSITE_GLOB = ".github/actions/**/action.{yml,yaml}";
const NODE24_OPT_IN = "FORCE_JAVASCRIPT_ACTIONS_TO_NODE24";

/** Action pins still on the Node 20 JavaScript runtime that need the workflow Node 24 opt-in. */
export type AffectedAction =
	| "actions/cache@v4"
	| "actions/checkout@v4"
	| "actions/download-artifact@v4"
	| "actions/setup-node@v4"
	| "actions/upload-artifact@v4";

const AFFECTED_ACTIONS: readonly AffectedAction[] = [
	"actions/cache@v4",
	"actions/checkout@v4",
	"actions/download-artifact@v4",
	"actions/setup-node@v4",
	"actions/upload-artifact@v4",
];

/** Count of affected pins found at a particular path. */
export interface AffectedActionCount {
	action: AffectedAction;
	count: number;
}

/** Affected pins discovered in a single workflow or composite-action YAML file. */
export interface FileAffectedPins {
	path: string;
	counts: AffectedActionCount[];
}

/** YAML file that should be scanned for affected pins. */
export interface ScannedYamlFile {
	path: string;
	contents: string;
}

/** Validation result for the CI workflow Node 24 JavaScript action opt-in. */
export interface CiWorkflowNode24Check {
	totalAffected: number;
	perFile: FileAffectedPins[];
	hasTopLevelNode24OptIn: boolean;
	messages: string[];
}

/**
 * Verifies that every affected JavaScript action pin reachable from `workflow`
 * — directly or via a local composite action — is protected by a workflow-level
 * `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` env opt-in.
 *
 * `composites` defaults to no extra files so callers can scan a single workflow
 * in isolation; production callers pass every local composite action because a
 * composite's `uses:` lines execute under the calling workflow's env.
 */
export function checkCiWorkflowNode24OptIn(
	workflow: ScannedYamlFile,
	composites: readonly ScannedYamlFile[] = [],
): CiWorkflowNode24Check {
	const perFile = collectAffectedPins([workflow, ...composites]);
	const totalAffected = perFile.reduce(
		(sum, file) => sum + file.counts.reduce((acc, entry) => acc + entry.count, 0),
		0,
	);
	const hasTopLevelNode24OptIn = findTopLevelNode24OptIn(workflow.contents);
	const messages: string[] = [];

	if (totalAffected > 0 && !hasTopLevelNode24OptIn) {
		const summary = perFile
			.filter((file) => file.counts.length > 0)
			.map(
				(file) =>
					`${file.path}: ${file.counts.map(({ action, count }) => `${action} (${count})`).join(", ")}`,
			)
			.join("; ");
		messages.push(
			`Affected JavaScript action pins still present (${summary}); add a top-level env block to ${workflow.path} with ${NODE24_OPT_IN}: true.`,
		);
	}

	return { totalAffected, perFile, hasTopLevelNode24OptIn, messages };
}

function collectAffectedPins(files: readonly ScannedYamlFile[]): FileAffectedPins[] {
	return files.map((file) => ({ path: file.path, counts: countAffectedActions(file.contents) }));
}

function countAffectedActions(contents: string): AffectedActionCount[] {
	const counts: Record<AffectedAction, number> = {
		"actions/cache@v4": 0,
		"actions/checkout@v4": 0,
		"actions/download-artifact@v4": 0,
		"actions/setup-node@v4": 0,
		"actions/upload-artifact@v4": 0,
	};

	for (const line of contents.split(/\r?\n/)) {
		const action = parseUsesAction(line);
		if (action === null) continue;
		counts[action]++;
	}

	const result: AffectedActionCount[] = [];
	for (const action of AFFECTED_ACTIONS) {
		const count = counts[action];
		if (count > 0) result.push({ action, count });
	}
	return result;
}

function parseUsesAction(line: string): AffectedAction | null {
	const trimmed = line.trim();
	if (trimmed.startsWith("- uses: ")) return toAffectedAction(trimmed.slice(8));
	if (trimmed.startsWith("uses: ")) return toAffectedAction(trimmed.slice(6));
	return null;
}

function toAffectedAction(value: string): AffectedAction | null {
	const action = readActionReference(value);
	switch (action) {
		case "actions/cache@v4":
		case "actions/checkout@v4":
		case "actions/download-artifact@v4":
		case "actions/setup-node@v4":
		case "actions/upload-artifact@v4":
			return action;
		default:
			return null;
	}
}

function readActionReference(value: string): string {
	const separator = value.search(/\s/);
	return separator === -1 ? value : value.slice(0, separator);
}

function findTopLevelNode24OptIn(workflow: string): boolean {
	const lines = workflow.split(/\r?\n/);
	for (let index = 0; index < lines.length; index++) {
		if (lines[index] !== "env:") continue;
		if (topLevelEnvContainsOptIn(lines, index + 1)) return true;
	}
	return false;
}

function topLevelEnvContainsOptIn(lines: readonly string[], startIndex: number): boolean {
	for (let index = startIndex; index < lines.length; index++) {
		const line = lines[index];
		if (line.length === 0 || line.trimStart().startsWith("#")) continue;
		if (!line.startsWith(" ")) return false;
		if (line.trim() === `${NODE24_OPT_IN}: true`) return true;
	}
	return false;
}

async function loadYaml(path: string): Promise<ScannedYamlFile> {
	return { path, contents: await Bun.file(path).text() };
}

if (import.meta.main) {
	const workflow = await loadYaml(WORKFLOW_PATH);
	const compositePaths: string[] = [];
	for await (const path of new Glob(COMPOSITE_GLOB).scan(".")) {
		compositePaths.push(path);
	}
	compositePaths.sort();
	const composites = await Promise.all(compositePaths.map(loadYaml));
	const result = checkCiWorkflowNode24OptIn(workflow, composites);
	if (result.messages.length > 0) {
		process.stderr.write(`${result.messages.join("\n")}\n`);
		process.exit(1);
	}
}
