import { afterEach, beforeAll, describe, expect, it } from "bun:test";
import { Settings, settings } from "../../../config/settings";
import type { AgentSession } from "../../../session/agent-session";
import { getThemeByName, setThemeInstance, theme } from "../../theme/theme";
import { StatusLineComponent } from "./component";

const TERMINAL_DEFAULT_BG_ANSI = "\x1b[49m";
const STATUS_LINE_WIDTH = 80;

let originalTransparentSetting = false;

function makeSessionWithLastMessage(lastMessage: unknown) {
	const messages = [lastMessage];
	const model = { id: "test-model", name: "Test Model", contextWindow: 128000 };
	return {
		messages,
		state: { messages, model },
		model,
		contextUsageRevision: 0,
		systemPrompt: [],
		agent: { state: { tools: [] } },
		modelRegistry: { isUsingOAuth: () => false },
		skills: [],
		isStreaming: false,
		isAutoThinking: false,
		autoResolvedThinkingLevel: () => undefined,
		isFastModeActive: () => false,
		isAdvisorActive: () => false,
		getContextUsage: () => ({ tokens: 42, contextWindow: 128000 }),
		getAsyncJobSnapshot: () => ({ running: [] }),
	};
}

beforeAll(async () => {
	await Settings.init({ inMemory: true });
	const loaded = await getThemeByName("dark");
	if (!loaded) throw new Error("theme unavailable");
	setThemeInstance(loaded);
	originalTransparentSetting = settings.get("statusLine.transparent");
});

afterEach(() => {
	settings.set("statusLine.transparent", originalTransparentSetting);
});

function getStatusLineContent(): string {
	const statusLine = new StatusLineComponent(
		makeSessionWithLastMessage({ role: "assistant", timestamp: 1, content: [] }) as unknown as AgentSession,
	);
	return statusLine.getTopBorder(STATUS_LINE_WIDTH).content;
}
describe("StatusLineComponent", () => {
	it("fingerprints tool-call arguments containing bigint values", () => {
		const statusLine = new StatusLineComponent(
			makeSessionWithLastMessage({
				role: "assistant",
				timestamp: 1,
				content: [
					{
						type: "toolCall",
						name: "read",
						arguments: { offset: 1n, nested: { limit: 2n } },
					},
				],
			}) as unknown as AgentSession,
		);

		expect(statusLine.getCachedContextBreakdown()).toEqual({ usedTokens: 42, contextWindow: 128000 });
	});

	it("uses the terminal default background for the top border by default", () => {
		const content = getStatusLineContent();

		expect(content).toContain(TERMINAL_DEFAULT_BG_ANSI);
		expect(content).not.toContain(theme.getBgAnsi("statusLineBg"));
	});

	it("uses the theme status-line background when transparency is disabled", () => {
		settings.set("statusLine.transparent", false);

		const content = getStatusLineContent();

		expect(content).toContain(theme.getBgAnsi("statusLineBg"));
		expect(content).not.toContain(TERMINAL_DEFAULT_BG_ANSI);
	});
});
