import { afterEach, describe, expect, it, vi } from "bun:test";
import * as kimiOauth from "../src/registry/oauth/kimi";
import type { UsageFetchContext } from "../src/usage";
import { kimiUsageProvider } from "../src/usage/kimi";

const KIMI_HEADERS = Object.freeze({
	"User-Agent": "KimiCLI/0.0.0",
	"X-Msh-Platform": "kimi_cli",
	"X-Msh-Version": "0.0.0",
	"X-Msh-Device-Name": "test",
	"X-Msh-Device-Model": "test",
	"X-Msh-Os-Version": "test",
	"X-Msh-Device-Id": "test",
});

afterEach(() => {
	vi.restoreAllMocks();
});

describe("Kimi usage provider", () => {
	it("preserves reset timestamps when normalizing usage windows", async () => {
		vi.spyOn(kimiOauth, "getKimiCommonHeaders").mockReturnValue(KIMI_HEADERS);
		const summaryReset = "2026-07-23T17:45:55.083587Z";
		const detailReset = "2026-07-17T18:45:55.083587Z";
		const windowReset = "2026-07-18T18:45:55.083587Z";
		const ignoredDetailReset = "2026-07-19T18:45:55.083587Z";
		const payload = {
			usage: { limit: "100", used: "20", remaining: "80", resetTime: summaryReset },
			limits: [
				{
					window: { duration: 300, timeUnit: "TIME_UNIT_MINUTE" },
					detail: { limit: "100", used: "100", resetTime: detailReset },
				},
				{
					window: { duration: 1, timeUnit: "TIME_UNIT_HOUR", resetTime: windowReset },
					detail: { limit: "100", used: "50", resetTime: ignoredDetailReset },
				},
				{
					window: { duration: 1, timeUnit: "TIME_UNIT_DAY" },
					detail: { limit: "100", used: "25" },
				},
			],
		};
		const ctx: UsageFetchContext = {
			fetch: async () => new Response(JSON.stringify(payload)),
		};

		const report = await kimiUsageProvider.fetchUsage(
			{
				provider: "kimi-code",
				credential: { type: "oauth", accessToken: "test-token" },
			},
			ctx,
		);

		expect(report).not.toBeNull();
		if (!report) throw new Error("Expected a Kimi usage report");
		expect(report.limits[0].window?.resetsAt).toBe(Date.parse(summaryReset));
		expect(report.limits[1].window?.resetsAt).toBe(Date.parse(detailReset));
		expect(report.limits[2].window?.resetsAt).toBe(Date.parse(windowReset));
		expect(report.limits[3].window?.resetsAt).toBeUndefined();
	});
});
