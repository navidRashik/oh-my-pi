/**
 * Contract: eval status-event rendering keeps the MOST RECENT log()/phase()
 * events visible.
 *
 * A long orchestration streams many status events onto the cell. The collapsed
 * and expanded transcript views must show the tail (latest progress) with a
 * `… N earlier` marker on top — not freeze on the first few events. Regression
 * for #5231 ("eval logs are not scrolling up").
 */
import { beforeAll, describe, expect, it } from "bun:test";
import type { EvalStatusEvent, EvalToolDetails } from "../../eval/types";
import { getThemeByName, type Theme } from "../../modes/theme/theme";
import { evalToolRenderer } from "../eval-render";

const strip = (lines: readonly string[]): string[] => lines.map(line => line.replace(/\x1b\[[0-9;]*m/g, ""));

function renderStatusOutput(statusEvents: EvalStatusEvent[], expanded: boolean, theme: Theme): string[] {
	const details: EvalToolDetails = {
		language: "js",
		languages: ["js"],
		cells: [{ index: 0, code: "for (…) log(…)", language: "js", output: "", status: "complete", statusEvents }],
	};
	const component = evalToolRenderer.renderResult(
		{ content: [{ type: "text", text: "" }], details },
		{ expanded, isPartial: false },
		theme,
	) as { render(width: number): readonly string[] };
	return strip(component.render(80));
}

describe("eval status-event rendering", () => {
	let uiTheme: Theme;
	beforeAll(async () => {
		const loaded = await getThemeByName("dark");
		if (!loaded) throw new Error("dark theme unavailable");
		uiTheme = loaded;
	});

	const manyEvents = (): EvalStatusEvent[] =>
		Array.from({ length: 20 }, (_, i) => ({ op: "log", message: `step ${i + 1} of workflow` }));

	it("collapsed view shows the latest events, not the first few", () => {
		const output = renderStatusOutput(manyEvents(), false, uiTheme).join("\n");

		expect(output).toContain("step 20 of workflow");
		expect(output).toContain("step 18 of workflow");
		expect(output).not.toContain("step 1 of workflow");
		expect(output).toContain("17 earlier");
	});

	it("expanded view widens the tail window toward the newest events", () => {
		const output = renderStatusOutput(manyEvents(), true, uiTheme).join("\n");

		expect(output).toContain("step 20 of workflow");
		expect(output).toContain("step 11 of workflow");
		expect(output).not.toContain("step 10 of workflow");
		expect(output).toContain("10 earlier");
	});

	it("shows all events without an elision marker when they fit", () => {
		const output = renderStatusOutput([{ op: "log", message: "only step" }], false, uiTheme).join("\n");

		expect(output).toContain("only step");
		expect(output).not.toContain("earlier");
	});
});
