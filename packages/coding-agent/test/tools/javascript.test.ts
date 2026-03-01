import { describe, expect, it } from "bun:test";
import { Settings } from "@oh-my-pi/pi-coding-agent/config/settings";
import type { ToolSession } from "@oh-my-pi/pi-coding-agent/tools";
import { JavaScriptTool } from "@oh-my-pi/pi-coding-agent/tools/javascript";
import { TempDir } from "@oh-my-pi/pi-utils";

function createSession(cwd: string): ToolSession {
	return {
		cwd,
		hasUI: false,
		getSessionFile: () => `${cwd}/session-file.jsonl`,
		getSessionSpawns: () => "*",
		settings: Settings.isolated(),
	};
}

describe("javascript tool", () => {
	it("exposes expected schema", () => {
		const tempDir = TempDir.createSync("@javascript-tool-");
		const tool = new JavaScriptTool(createSession(tempDir.path()));
		const schema = tool.parameters as {
			type: string;
			properties: Record<string, { type: string }>;
			required?: string[];
		};

		expect(schema.type).toBe("object");
		expect(schema.properties.cells.type).toBe("array");
		expect(schema.properties.timeout.type).toBe("number");
		expect(schema.properties.cwd.type).toBe("string");
		expect(schema.properties.reset.type).toBe("boolean");
		expect(schema.required).toEqual(["cells"]);
		tempDir.removeSync();
	});

	it("persists VM state, exports, and status notifications", async () => {
		const tempDir = TempDir.createSync("@javascript-tool-");
		const tool = new JavaScriptTool(createSession(tempDir.path()));

		const first = await tool.execute("call-1", {
			cells: [
				{
					title: "define",
					code: "const x = 2; setExport('answer', x + 40); notify('phase', { step: 'define' }); x",
				},
				{ title: "use", code: "exports.answer + 1" },
			],
		});
		const firstText = first.content.find(item => item.type === "text")?.text ?? "";
		expect(firstText).toContain("[1/2] define");
		expect(firstText).toContain("[2/2] use");
		expect(firstText).toContain("43");
		expect(first.details?.exports).toContain("answer");
		expect(first.details?.statusEvents?.[0]).toEqual(expect.objectContaining({ op: "phase", step: "define" }));

		const second = await tool.execute("call-2", { cells: [{ code: "getExport('answer')" }] });
		const secondText = second.content.find(item => item.type === "text")?.text ?? "";
		expect(secondText).toContain("42");

		const reset = await tool.execute("call-3", { reset: true, cells: [{ code: "typeof exports.answer" }] });
		const resetText = reset.content.find(item => item.type === "text")?.text ?? "";
		expect(resetText).toContain("undefined");
		tempDir.removeSync();
	});
});
