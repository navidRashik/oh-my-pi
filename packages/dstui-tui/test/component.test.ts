import { describe, expect, test } from "bun:test";
import { compileModule, instantiate } from "@oh-my-pi/pi-dstui";
import { DstuiComponent } from "../src/component";

function stripAnsi(line: string): string {
	return line.replace(/\x1b\[[0-9;]*m/g, "");
}

describe("DstuiComponent", () => {
	test("delegates render and handleInput to the wrapped instance", () => {
		const module = compileModule(`
			(defcomponent counter ()
				(state (n 0))
				(view (text (str n)))
				(bind :right (set! n (+ n 1))))
		`);
		const instance = instantiate(module.components[0], {}, module.views);
		const component = new DstuiComponent(instance);
		expect(stripAnsi(component.render(10)[0] ?? "")).toBe("0");
		component.handleInput("\u001b[C");
		expect(stripAnsi(component.render(10)[0] ?? "")).toBe("1");
		instance.dispose();
	});

	test("invalidate is a no-op and does not reset state", () => {
		const module = compileModule(`
			(defcomponent t ()
				(state (n 5))
				(view (text (str n))))
		`);
		const instance = instantiate(module.components[0], {}, module.views);
		const component = new DstuiComponent(instance);
		component.invalidate();
		expect(stripAnsi(component.render(5)[0] ?? "")).toBe("5");
		instance.dispose();
	});

	test("focused flag is mutable for pi-tui focus tracking", () => {
		const module = compileModule(`(defcomponent t () (view (text "x")))`);
		const instance = instantiate(module.components[0], {}, module.views);
		const component = new DstuiComponent(instance);
		expect(component.focused).toBe(false);
		component.focused = true;
		expect(component.focused).toBe(true);
		instance.dispose();
	});
});
