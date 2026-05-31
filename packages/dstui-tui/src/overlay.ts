/**
 * `mountDstuiOverlay` mounts a compiled DSL module inside any host
 * that exposes a `custom<T>(factory, options)` overlay slot (the
 * shape `ExtensionUIContext` in `@oh-my-pi/pi-coding-agent` provides).
 *
 * The helper does **not** depend on `@oh-my-pi/pi-coding-agent`: it
 * declares a minimal structural interface (`OverlayMount`) so any TUI
 * shell that follows the same protocol can mount DSL components.
 */

import {
	type ComponentDef,
	compileModule,
	type DstuiLimits,
	instantiate,
	type ModuleDef,
	type SettleEvent,
} from "@oh-my-pi/pi-dstui";
import type { Component } from "@oh-my-pi/pi-tui";
import { DstuiComponent } from "./component";

/**
 * The slice of `ExtensionUIContext` we depend on. Anything that
 * exposes a `custom<T>(factory, options)` overlay slot works.
 */
export interface OverlayMount {
	custom<T>(
		factory: (
			tui: unknown,
			theme: unknown,
			keybindings: unknown,
			done: (result: T) => void,
		) => Component | Promise<Component>,
		options?: { overlay?: boolean },
	): Promise<T>;
}

/** Options accepted by {@link mountDstuiOverlay}. */
export interface OverlayOptions {
	/** Raw DSL source. Mutually exclusive with `module`. */
	source?: string;
	/** Pre-compiled module. Mutually exclusive with `source`. */
	module?: ModuleDef;
	/** Component to instantiate. Defaults to the first declared component. */
	componentName?: string;
	/** Initial component config. */
	config?: Record<string, unknown>;
	/** Optional limit overlay forwarded to compile + instantiate. */
	limits?: Partial<DstuiLimits>;
	/** Forwarded to `ui.custom(factory, { overlay })`. */
	overlay?: boolean;
	/** Callback invoked on every DSL `(emit ...)` or `(cancel)`. */
	onSettle?: (event: SettleEvent) => void;
	/** Callback invoked on every DSL evaluation error. */
	onError?: (error: unknown) => void;
}

function pickComponent(module: ModuleDef, name: string | undefined): ComponentDef {
	if (name === undefined) {
		const first = module.components[0];
		if (!first) throw new Error("dstui module declares no components");
		return first;
	}
	const found = module.components.find(c => c.name === name);
	if (!found) throw new Error(`dstui module has no component "${name}"`);
	return found;
}

/**
 * Compile (if needed), instantiate, and mount a DSL module as an
 * overlay. Resolves to the {@link SettleEvent} the component emitted
 * (or `{ reason: "cancel", value: null }` if dismissed). Guarantees
 * that `instance.dispose()` runs before the promise resolves so
 * timers do not outlive the overlay even on the cancel path.
 */
export async function mountDstuiOverlay(mount: OverlayMount, options: OverlayOptions): Promise<SettleEvent> {
	if (!options.source && !options.module) {
		throw new Error("mountDstuiOverlay requires either `source` or `module`");
	}
	const module = options.module ?? compileModule(options.source as string, { limits: options.limits });
	const def = pickComponent(module, options.componentName);
	return mount.custom<SettleEvent>(
		(_tui, _theme, _keybindings, done) => {
			const instance = instantiate(def, options.config ?? {}, module.views, {
				limits: options.limits,
				onError: options.onError,
				onSettled: event => {
					try {
						options.onSettle?.(event);
					} finally {
						instance.dispose();
						done(event);
					}
				},
			});
			return new DstuiComponent(instance);
		},
		{ overlay: options.overlay ?? true },
	);
}
