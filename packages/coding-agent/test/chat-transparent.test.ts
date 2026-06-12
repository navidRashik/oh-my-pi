import { afterAll, beforeAll, describe, expect, it } from "bun:test";
import { resetSettingsForTest, Settings } from "@oh-my-pi/pi-coding-agent/config/settings";
import { UserMessageComponent } from "@oh-my-pi/pi-coding-agent/modes/components/user-message";
import { initTheme, setChatTransparent, setTheme, theme } from "@oh-my-pi/pi-coding-agent/modes/theme/theme";
import { getProjectDir, setProjectDir } from "@oh-my-pi/pi-utils";

const originalProjectDir = getProjectDir();

beforeAll(async () => {
	resetSettingsForTest();
	await Settings.init({ inMemory: true });
	await initTheme();
	// Anchor the fixture on a theme whose chat surfaces are painted with explicit
	// hex bgs (titanium is the theme used in the original repro), so the
	// transparency toggle has a non-vacuous off->on transition to assert.
	const result = await setTheme("titanium");
	if (!result.success) throw new Error(`setTheme failed: ${result.error}`);
});

afterAll(async () => {
	await setChatTransparent(false);
	resetSettingsForTest();
	setProjectDir(originalProjectDir);
});

const CHAT_SURFACES = ["userMessageBg", "customMessageBg", "toolPendingBg", "toolSuccessBg", "toolErrorBg"] as const;
const TRANSPARENT_BG_ANSI = "\x1b[49m";

describe("chat.transparent appearance setting", () => {
	it("paints chat-surface backgrounds from the theme when disabled (default)", async () => {
		await setChatTransparent(false);
		for (const key of CHAT_SURFACES) {
			// The fixture relies on every chat surface having a real bg fill — otherwise
			// the negative case in the transparent test would be vacuous.
			expect(theme.getBgAnsi(key)).toMatch(/\x1b\[48;/);
		}

		const userBg = theme.getBgAnsi("userMessageBg");
		const rendered = new UserMessageComponent("hello chat surfaces").render(80).join("\n");
		expect(rendered).toContain(userBg);
	});

	it("drops chat-surface bg ANSI in favor of the terminal default when enabled", async () => {
		await setChatTransparent(true);
		for (const key of CHAT_SURFACES) {
			expect(theme.getBgAnsi(key)).toBe(TRANSPARENT_BG_ANSI);
		}

		// User message ANSI no longer contains any `\x1b[48;…` background escape —
		// every emitted bg is the terminal-default reset.
		const rendered = new UserMessageComponent("hello chat surfaces").render(80).join("\n");
		expect(rendered).not.toMatch(/\x1b\[48;/);
		expect(rendered).toContain(TRANSPARENT_BG_ANSI);

		// Unrelated theme surfaces still paint normally — only the listed chat keys flip.
		expect(theme.getBgAnsi("statusLineBg")).not.toBe(TRANSPARENT_BG_ANSI);
		expect(theme.getBgAnsi("selectedBg")).not.toBe(TRANSPARENT_BG_ANSI);
	});

	it("restores the theme's chat-surface fills when toggled back off", async () => {
		await setChatTransparent(true);
		await setChatTransparent(false);
		for (const key of CHAT_SURFACES) {
			expect(theme.getBgAnsi(key)).toMatch(/\x1b\[48;/);
		}
	});
});
