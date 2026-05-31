import { describe, expect, test } from "bun:test";
import type { Component } from "@oh-my-pi/pi-tui";
import { mountDstuiOverlay, type OverlayMount } from "../src/overlay";

/**
 * Tiny in-memory mount that mimics `ExtensionUIContext.custom`. The
 * factory is invoked synchronously, the test then drives `handleInput`
 * and `done` exactly as the real TUI would.
 */
function makeMount(): {
	mount: OverlayMount;
	drive: (input: string) => void;
	rendered(width: number): string[];
	options(): { overlay?: boolean } | undefined;
	customCalls(): number;
} {
	let active: Component | undefined;
	let opts: { overlay?: boolean } | undefined;
	let calls = 0;
	const mount: OverlayMount = {
		custom: async <T>(
			factory: (
				tui: unknown,
				theme: unknown,
				keybindings: unknown,
				done: (result: T) => void,
			) => Component | Promise<Component>,
			options?: { overlay?: boolean },
		): Promise<T> => {
			calls += 1;
			opts = options;
			const { promise, resolve } = Promise.withResolvers<T>();
			active = await factory(null, null, null, value => resolve(value));
			return promise;
		},
	};
	return {
		mount,
		drive: input => active?.handleInput?.(input),
		rendered: width => active?.render(width) ?? [],
		options: () => opts,
		customCalls: () => calls,
	};
}

function stripAnsi(line: string): string {
	return line.replace(/\x1b\[[0-9;]*m/g, "");
}

describe("mountDstuiOverlay", () => {
	test("resolves on emit and disposes the instance", async () => {
		const harness = makeMount();
		const promise = mountDstuiOverlay(harness.mount, {
			source: `
				(defcomponent picker ()
					(state (sel 0))
					(view (text (str sel)))
					(bind :right (set! sel (+ sel 1)))
					(bind :enter (emit sel)))
			`,
			overlay: true,
		});
		// Allow `custom` to run the factory.
		await Promise.resolve();
		expect(harness.options()).toEqual({ overlay: true });
		expect(stripAnsi(harness.rendered(10)[0] ?? "")).toBe("0");
		harness.drive("\u001b[C");
		harness.drive("\u001b[C");
		expect(stripAnsi(harness.rendered(10)[0] ?? "")).toBe("2");
		harness.drive("\r");
		const settle = await promise;
		expect(settle).toEqual({ reason: "emit", value: 2 });
	});

	test("resolves on cancel and forwards onSettle", async () => {
		const harness = makeMount();
		const settles: unknown[] = [];
		const promise = mountDstuiOverlay(harness.mount, {
			source: `(defcomponent t () (view (text "x")) (bind :enter (emit 1)))`,
			onSettle: ev => settles.push(ev),
		});
		await Promise.resolve();
		harness.drive("\u001b");
		const settle = await promise;
		expect(settle).toEqual({ reason: "cancel", value: null });
		expect(settles).toEqual([{ reason: "cancel", value: null }]);
	});

	test("componentName picks a specific declaration", async () => {
		const harness = makeMount();
		const promise = mountDstuiOverlay(harness.mount, {
			source: `
				(defcomponent foo () (view (text "foo")) (bind :enter (emit "foo")))
				(defcomponent bar () (view (text "bar")) (bind :enter (emit "bar")))
			`,
			componentName: "bar",
		});
		await Promise.resolve();
		expect(stripAnsi(harness.rendered(10)[0] ?? "")).toBe("bar");
		harness.drive("\r");
		await expect(promise).resolves.toEqual({ reason: "emit", value: "bar" });
	});

	test("rejects when neither source nor module is provided", async () => {
		const harness = makeMount();
		await expect(mountDstuiOverlay(harness.mount, {})).rejects.toThrow(/source.*module/);
	});

	test("rejects when component name does not exist", async () => {
		const harness = makeMount();
		await expect(
			mountDstuiOverlay(harness.mount, {
				source: `(defcomponent foo () (view (text "foo")))`,
				componentName: "missing",
			}),
		).rejects.toThrow(/missing/);
	});

	test("rejects instantiation errors before entering custom", async () => {
		const harness = makeMount();
		await expect(
			mountDstuiOverlay(harness.mount, {
				source: `(defcomponent t () (state (x (missing-fn))) (view (text "x")))`,
			}),
		).rejects.toThrow(/missing-fn/);
		expect(harness.customCalls()).toBe(0);
	});

	test("aborts the overlay via AbortSignal", async () => {
		const harness = makeMount();
		const controller = new AbortController();
		const promise = mountDstuiOverlay(harness.mount, {
			source: `(defcomponent t () (view (text "waiting")) (bind :enter (emit 1)))`,
			signal: controller.signal,
		});
		await Promise.resolve();
		expect(stripAnsi(harness.rendered(20)[0] ?? "")).toBe("waiting");
		controller.abort();
		await expect(promise).rejects.toMatchObject({ name: "AbortError" });
		expect(harness.rendered(20)).toEqual([]);
	});
});
