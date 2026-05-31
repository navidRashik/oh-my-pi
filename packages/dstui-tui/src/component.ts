/**
 * `@oh-my-pi/pi-tui` adapter for a `@oh-my-pi/pi-dstui` instance.
 *
 * Implements the `Component` contract `pi-tui` expects so any DSL
 * module can be mounted directly into an overlay or widget slot.
 * `render` and `handleInput` delegate one-to-one; `invalidate` is a
 * no-op because the DSL runtime already redraws lazily from
 * `onRender`.
 */

import type { ComponentInstance } from "@oh-my-pi/pi-dstui";
import type { Component } from "@oh-my-pi/pi-tui";

/**
 * Focusable wrapper around a `ComponentInstance`.
 *
 * Lifecycle ownership stays with the caller: the adapter never
 * disposes the wrapped instance on its own. Tests and overlay glue
 * call `instance.dispose()` explicitly so timers stop deterministically.
 */
export class DstuiComponent implements Component {
	focused = false;

	constructor(readonly instance: ComponentInstance) {}

	render(width: number): string[] {
		return this.instance.render(width);
	}

	handleInput(data: string): void {
		this.instance.handleInput(data);
	}

	/** No cached render state; the DSL runtime re-evaluates every frame. */
	invalidate(): void {}
}
