import { describe, expect, it } from "bun:test";
import { resolveDaemonSpawnOptions } from "./spawn-options";

describe("resolveDaemonSpawnOptions", () => {
	it("hides and isolates Windows launch processes", () => {
		expect(resolveDaemonSpawnOptions("win32")).toEqual({ detached: true, windowsHide: true });
	});

	it("keeps POSIX daemons in their own session", () => {
		expect(resolveDaemonSpawnOptions("linux")).toEqual({ detached: true });
	});
});
