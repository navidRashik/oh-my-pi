import { describe, expect, it } from "bun:test";
import { checkCiWorkflowNode24OptIn } from "./check-ci-workflow";

const WORKFLOW = { path: ".github/workflows/ci.yml" };

describe("checkCiWorkflowNode24OptIn", () => {
	it("accepts affected workflow pins when the workflow has the top-level opt-in", () => {
		const result = checkCiWorkflowNode24OptIn({
			...WORKFLOW,
			contents: [
				"name: CI",
				"env:",
				"   FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true",
				"jobs:",
				"   check:",
				"      steps:",
				"         - uses: actions/checkout@v4",
				"         - name: Cache bun dependencies",
				"           uses: actions/cache@v4",
			].join("\n"),
		});

		expect(result.hasTopLevelNode24OptIn).toBe(true);
		expect(result.messages).toEqual([]);
		expect(result.totalAffected).toBe(2);
	});

	it("rejects affected workflow pins without the workflow-level opt-in", () => {
		const result = checkCiWorkflowNode24OptIn({
			...WORKFLOW,
			contents: [
				"name: CI",
				"jobs:",
				"   release:",
				"      steps:",
				"         - uses: actions/download-artifact@v4",
				"         - uses: actions/upload-artifact@v4",
			].join("\n"),
		});

		expect(result.hasTopLevelNode24OptIn).toBe(false);
		expect(result.totalAffected).toBe(2);
		expect(result.messages.length).toBe(1);
		expect(result.messages[0]).toContain("FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true");
	});

	it("flags affected pins discovered only in local composite actions", () => {
		const result = checkCiWorkflowNode24OptIn(
			{
				...WORKFLOW,
				contents: [
					"name: CI",
					"jobs:",
					"   build:",
					"      steps:",
					"         - uses: ./.github/actions/build-native",
				].join("\n"),
			},
			[
				{
					path: ".github/actions/build-native/action.yml",
					contents: [
						"runs:",
						"   using: composite",
						"   steps:",
						"      - uses: actions/cache@v4",
						"      - uses: actions/upload-artifact@v4",
					].join("\n"),
				},
			],
		);

		expect(result.hasTopLevelNode24OptIn).toBe(false);
		expect(result.totalAffected).toBe(2);
		expect(result.messages[0]).toContain(".github/actions/build-native/action.yml");
		expect(result.messages[0]).toContain("actions/cache@v4 (1)");
		expect(result.messages[0]).toContain("actions/upload-artifact@v4 (1)");
	});

	it("accepts composite-action pins when the workflow carries the opt-in", () => {
		const result = checkCiWorkflowNode24OptIn(
			{
				...WORKFLOW,
				contents: [
					"name: CI",
					"env:",
					"   FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true",
					"jobs:",
					"   build:",
					"      steps:",
					"         - uses: ./.github/actions/build-native",
				].join("\n"),
			},
			[
				{
					path: ".github/actions/build-native/action.yml",
					contents: ["runs:", "   using: composite", "   steps:", "      - uses: actions/cache@v4"].join(
						"\n",
					),
				},
			],
		);

		expect(result.hasTopLevelNode24OptIn).toBe(true);
		expect(result.messages).toEqual([]);
	});

	it("does not accept a job-level opt-in as workflow-wide coverage", () => {
		const result = checkCiWorkflowNode24OptIn({
			...WORKFLOW,
			contents: [
				"name: CI",
				"jobs:",
				"   check:",
				"      env:",
				"         FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true",
				"      steps:",
				"         - uses: actions/setup-node@v4",
			].join("\n"),
		});

		expect(result.hasTopLevelNode24OptIn).toBe(false);
		expect(result.messages.length).toBeGreaterThan(0);
	});
});
