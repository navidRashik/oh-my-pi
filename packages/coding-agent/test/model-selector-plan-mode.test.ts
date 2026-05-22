import { afterEach, beforeEach, describe, expect, it, vi } from "bun:test";
import * as path from "node:path";
import { Agent } from "@oh-my-pi/pi-agent-core";
import { getBundledModel, type Model, modelsAreEqual } from "@oh-my-pi/pi-ai";
import { ModelRegistry } from "@oh-my-pi/pi-coding-agent/config/model-registry";
import { Settings } from "@oh-my-pi/pi-coding-agent/config/settings";
import { ModelSelectorComponent } from "@oh-my-pi/pi-coding-agent/modes/components/model-selector";
import { SelectorController } from "@oh-my-pi/pi-coding-agent/modes/controllers/selector-controller";
import { getThemeByName, setThemeInstance } from "@oh-my-pi/pi-coding-agent/modes/theme/theme";
import type { InteractiveModeContext } from "@oh-my-pi/pi-coding-agent/modes/types";
import { AgentSession } from "@oh-my-pi/pi-coding-agent/session/agent-session";
import { AuthStorage } from "@oh-my-pi/pi-coding-agent/session/auth-storage";
import { SessionManager } from "@oh-my-pi/pi-coding-agent/session/session-manager";
import type { Component } from "@oh-my-pi/pi-tui";
import { TempDir } from "@oh-my-pi/pi-utils";

function expectAnthropicModel(id: string): Model {
	const model = getBundledModel("anthropic", id);
	if (!model) throw new Error(`Expected bundled model anthropic/${id}`);
	return model;
}

async function flushModelSelection(): Promise<void> {
	await Bun.sleep(0);
	await Bun.sleep(0);
}

describe("model selector in plan mode", () => {
	let tempDir: TempDir;
	let authStorage: AuthStorage;
	let session: AgentSession;

	beforeEach(async () => {
		const testTheme = await getThemeByName("dark");
		if (!testTheme) throw new Error("Failed to load dark theme");
		setThemeInstance(testTheme);
		tempDir = TempDir.createSync("@pi-plan-model-selector-");
		authStorage = await AuthStorage.create(path.join(tempDir.path(), "testauth.db"));
		authStorage.setRuntimeApiKey("anthropic", "test-key");
	});

	afterEach(async () => {
		await session?.dispose();
		authStorage.close();
		tempDir.removeSync();
	});

	it("applies a changed plan role to the active plan-mode session immediately", async () => {
		const oldPlan = expectAnthropicModel("claude-sonnet-4-5");
		const newPlan = expectAnthropicModel("claude-opus-4-1");
		const settings = Settings.isolated({
			modelRoles: {
				plan: `${oldPlan.provider}/${oldPlan.id}`,
			},
		});
		const modelRegistry = new ModelRegistry(authStorage);
		session = new AgentSession({
			agent: new Agent({
				initialState: { model: oldPlan, systemPrompt: ["Test"], tools: [], messages: [] },
			}),
			sessionManager: SessionManager.inMemory(tempDir.path()),
			settings,
			modelRegistry,
			scopedModels: [{ model: oldPlan }, { model: newPlan }],
		});
		session.setPlanModeState({ enabled: true, planFilePath: "local://PLAN.md" });

		let selector: ModelSelectorComponent | undefined;
		const editor = {} as Component;
		const ctx = {
			editor,
			editorContainer: {
				clear: vi.fn(),
				addChild: vi.fn((component: Component) => {
					if (component instanceof ModelSelectorComponent) selector = component;
				}),
			},
			ui: {
				setFocus: vi.fn(),
				requestRender: vi.fn(),
			},
			session,
			settings,
			statusLine: { invalidate: vi.fn() },
			updateEditorBorderColor: vi.fn(),
			showStatus: vi.fn(),
			showError: vi.fn(),
			planModeEnabled: true,
		} as unknown as InteractiveModeContext;

		new SelectorController(ctx).showModelSelector();
		await Bun.sleep(0);
		if (!selector) throw new Error("Expected model selector to be mounted");

		for (const char of "opus") selector.handleInput(char);
		selector.handleInput("\n");
		for (let i = 0; i < 4; i++) selector.handleInput("\x1b[B");
		selector.handleInput("\n");
		selector.handleInput("\n");
		await flushModelSelection();

		expect(settings.getModelRole("plan")).toBe(`${newPlan.provider}/${newPlan.id}`);
		expect(modelsAreEqual(session.model, newPlan)).toBe(true);
	});
});
