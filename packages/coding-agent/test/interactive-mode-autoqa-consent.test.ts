import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "bun:test";
import * as path from "node:path";
import { Agent } from "@oh-my-pi/pi-agent-core";
import { TempDir } from "@oh-my-pi/pi-utils";
import { ModelRegistry } from "../src/config/model-registry";
import { resetSettingsForTest, Settings } from "../src/config/settings";
import { InteractiveMode } from "../src/modes/interactive-mode";
import { initTheme } from "../src/modes/theme/theme";
import { AgentSession } from "../src/session/agent-session";
import { AuthStorage } from "../src/session/auth-storage";
import { SessionManager } from "../src/session/session-manager";
import { __resetAutoQaConsentForTests, resolveAutoQaConsent } from "../src/tools/report-tool-issue";

describe("InteractiveMode AutoQA consent prompt", () => {
	let authStorage: AuthStorage;
	let mode: InteractiveMode;
	let session: AgentSession;
	let tempDir: TempDir;

	beforeAll(() => {
		initTheme();
	});

	beforeEach(async () => {
		vi.spyOn(process.stdout, "write").mockReturnValue(true);
		vi.spyOn(process.stdin, "resume").mockReturnValue(process.stdin);
		vi.spyOn(process.stdin, "pause").mockReturnValue(process.stdin);
		vi.spyOn(process.stdin, "setEncoding").mockReturnValue(process.stdin);
		if (typeof process.stdin.setRawMode === "function") {
			vi.spyOn(process.stdin, "setRawMode").mockReturnValue(process.stdin);
		}

		resetSettingsForTest();
		tempDir = TempDir.createSync("@pi-autoqa-interactive-");
		await Settings.init({ inMemory: true, cwd: tempDir.path() });
		authStorage = await AuthStorage.create(path.join(tempDir.path(), "testauth.db"));
		const modelRegistry = new ModelRegistry(authStorage);
		const model = modelRegistry.find("anthropic", "claude-sonnet-4-5");
		if (!model) throw new Error("Expected claude-sonnet-4-5 to exist in registry");

		session = new AgentSession({
			agent: new Agent({ initialState: { model, systemPrompt: ["Test"], tools: [], messages: [] } }),
			sessionManager: SessionManager.create(tempDir.path(), tempDir.path()),
			settings: Settings.isolated({ "dev.autoqa": true }),
			modelRegistry,
		});
		mode = new InteractiveMode(session, "test");
	});

	afterEach(async () => {
		mode?.stop();
		vi.restoreAllMocks();
		__resetAutoQaConsentForTests();
		await session?.dispose();
		authStorage?.close();
		tempDir?.removeSync();
		resetSettingsForTest();
	});

	it("does not persist a decision when the AutoQA prompt is ignored", async () => {
		const selector = vi.spyOn(mode, "showHookSelector").mockResolvedValue(undefined);
		await mode.init();

		expect(await resolveAutoQaConsent(session.settings)).toBe(false);

		expect(selector).toHaveBeenCalledWith(expect.any(String), ["Yes", "No"], { initialIndex: 1 });
		expect(session.settings.get("dev.autoqaConsent")).toBe("unset");
		expect(Settings.instance.get("dev.autoqaConsent")).toBe("unset");
		expect(Settings.instance.get("dev.autoqa")).toBe(false);
	});

	it("persists denial without enabling AutoQA when the user chooses No", async () => {
		const selector = vi.spyOn(mode, "showHookSelector").mockResolvedValue("No");
		await mode.init();

		expect(await resolveAutoQaConsent(session.settings)).toBe(false);
		expect(selector).toHaveBeenCalled();

		expect(Settings.instance.get("dev.autoqaConsent")).toBe("denied");
		expect(Settings.instance.get("dev.autoqa")).toBe(false);
	});
});
