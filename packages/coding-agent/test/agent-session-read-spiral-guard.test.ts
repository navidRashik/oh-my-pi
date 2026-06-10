import { afterEach, expect, it } from "bun:test";
import * as path from "node:path";
import { Agent, type AgentTool } from "@oh-my-pi/pi-agent-core";
import { type ToolResultMessage, z } from "@oh-my-pi/pi-ai";
import { createMockModel, type MockModel, type MockResponse } from "@oh-my-pi/pi-ai/providers/mock";
import { ModelRegistry } from "@oh-my-pi/pi-coding-agent/config/model-registry";
import { Settings } from "@oh-my-pi/pi-coding-agent/config/settings";
import { AgentSession } from "@oh-my-pi/pi-coding-agent/session/agent-session";
import { AuthStorage } from "@oh-my-pi/pi-coding-agent/session/auth-storage";
import { convertToLlm } from "@oh-my-pi/pi-coding-agent/session/messages";
import { SessionManager } from "@oh-my-pi/pi-coding-agent/session/session-manager";
import { TempDir } from "@oh-my-pi/pi-utils";

const readSchema = z.object({ path: z.string() });
const searchSchema = z.object({ pattern: z.string() });

interface Harness {
	session: AgentSession;
	tempDir: TempDir;
	authStorage: AuthStorage;
	mock: MockModel;
	executedReads: string[];
	executedSearches: string[];
}

interface HarnessOptions {
	settings?: Record<string, unknown>;
	responses: MockResponse[];
}

let harness: Harness | undefined;

async function createHarness({ settings: settingsOverrides = {}, responses }: HarnessOptions): Promise<Harness> {
	const tempDir = TempDir.createSync("@pi-read-spiral-guard-");
	const authStorage = await AuthStorage.create(path.join(tempDir.path(), "auth.db"));
	authStorage.setRuntimeApiKey("mock", "test-key");
	const modelRegistry = new ModelRegistry(authStorage, path.join(tempDir.path(), "models.yml"));
	const settings = Settings.isolated({
		"compaction.enabled": false,
		"retry.enabled": false,
		"todo.enabled": false,
		"todo.eager": false,
		"todo.reminders": false,
		...settingsOverrides,
	});

	const executedReads: string[] = [];
	const executedSearches: string[] = [];
	const readTool: AgentTool<typeof readSchema> = {
		name: "read",
		label: "Read",
		description: "Mock read tool",
		parameters: readSchema,
		approval: "read",
		execute: async (_toolCallId, params) => {
			executedReads.push(params.path);
			return { content: [{ type: "text", text: `contents for ${params.path}\n${"x".repeat(256)}` }] };
		},
	};
	const searchTool: AgentTool<typeof searchSchema> = {
		name: "search",
		label: "Search",
		description: "Mock search tool",
		parameters: searchSchema,
		approval: "read",
		execute: async (_toolCallId, params) => ({
			content: [{ type: "text", text: `no hits for ${params.pattern}` }],
		}),
	};
	searchTool.execute = async (_toolCallId, params) => {
		executedSearches.push(params.pattern);
		return { content: [{ type: "text", text: `no hits for ${params.pattern}` }] };
	};

	const mock = createMockModel({ responses });

	let session: AgentSession;
	const agent = new Agent({
		getApiKey: () => "test-key",
		initialState: {
			model: mock,
			systemPrompt: ["Test"],
			tools: [readTool, searchTool],
			messages: [],
		},
		convertToLlm,
		getToolChoice: () => session.nextToolChoice(),
		streamFn: mock.stream,
	});

	session = new AgentSession({
		agent,
		sessionManager: SessionManager.inMemory(tempDir.path()),
		settings,
		modelRegistry,
		toolRegistry: new Map<string, AgentTool>([
			[readTool.name, readTool as AgentTool],
			[searchTool.name, searchTool as AgentTool],
		]),
	});

	return { session, tempDir, authStorage, mock, executedReads, executedSearches };
}

afterEach(async () => {
	if (!harness) return;
	await harness.session.dispose();
	harness.authStorage.close();
	harness.tempDir.removeSync();
	harness = undefined;
});

it("forces a no-tool synthesis turn after the per-prompt read budget is exhausted", async () => {
	harness = await createHarness({
		settings: { "investigationGuard.maxReadCalls": 5 },
		responses: [
			{
				content: Array.from({ length: 6 }, (_, index) => ({
					type: "toolCall" as const,
					id: `read-${index}`,
					name: "read",
					arguments: { path: `engine-${index}.cs` },
				})),
				stopReason: "toolUse",
			},
			{ content: ["I have enough context to answer now."], stopReason: "stop" },
		],
	});

	await harness.session.prompt("investigate engine clipping");
	await harness.session.waitForIdle();

	expect(harness.executedReads).toEqual(["engine-0.cs", "engine-1.cs", "engine-2.cs", "engine-3.cs", "engine-4.cs"]);
	expect(harness.mock.calls).toHaveLength(2);
	expect(harness.mock.calls[1]?.options?.toolChoice).toBe("none");

	const blockedResult = harness.session.agent.state.messages.find(
		(message): message is ToolResultMessage => message.role === "toolResult" && message.toolCallId === "read-5",
	);
	expect(blockedResult?.isError).toBe(true);
	expect(JSON.stringify(blockedResult?.content)).toContain("Read investigation limit reached");
});

it("forces a no-tool synthesis turn after consecutive tool-use turns hit the cap", async () => {
	const spiralResponse = (turn: number): MockResponse => ({
		content: [
			{
				type: "toolCall" as const,
				id: `search-${turn}`,
				name: "search",
				arguments: { pattern: `term-${turn}` },
			},
		],
		stopReason: "toolUse",
	});
	harness = await createHarness({
		settings: { "investigationGuard.maxConsecutiveToolUseTurns": 3 },
		responses: [
			spiralResponse(0),
			spiralResponse(1),
			spiralResponse(2),
			{ content: ["I have enough context to answer now."], stopReason: "stop" },
		],
	});

	await harness.session.prompt("look around");
	await harness.session.waitForIdle();

	expect(harness.executedSearches).toEqual(["term-0", "term-1", "term-2"]);
	expect(harness.mock.calls).toHaveLength(4);
	expect(harness.mock.calls[0]?.options?.toolChoice).toBeUndefined();
	expect(harness.mock.calls[1]?.options?.toolChoice).toBeUndefined();
	expect(harness.mock.calls[2]?.options?.toolChoice).toBeUndefined();
	expect(harness.mock.calls[3]?.options?.toolChoice).toBe("none");
});
