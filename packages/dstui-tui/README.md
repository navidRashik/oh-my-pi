# `@oh-my-pi/pi-dstui-tui`

`@oh-my-pi/pi-tui` adapter for `@oh-my-pi/pi-dstui`. Wraps a
`ComponentInstance` so the TUI can render it as a focusable overlay
and forwards keyboard input through the DSL `bind` table.

## Usage

```ts
import { compileModule, instantiate } from "@oh-my-pi/pi-dstui";
import { DstuiComponent, mountDstuiOverlay } from "@oh-my-pi/pi-dstui-tui";

// Manual mount inside any host that exposes a pi-tui `Component` slot.
const module = compileModule(source);
const instance = instantiate(module.components[0], config, module.views);
const component = new DstuiComponent(instance);

// Or, inside the agent extension surface, mount as an overlay and await settle:
const settle = await mountDstuiOverlay(ui, { source, config });
if (settle.reason === "emit") doSomethingWith(settle.value);
```

## Safety

- All safety properties of `@oh-my-pi/pi-dstui` (parser caps, evaluator
  fuel/depth, prototype-key denial, idempotent settle, capped output)
  apply unchanged. The adapter is a passthrough.
- `DstuiComponent.invalidate()` triggers a re-render only; it does not
  reset the instance state, timers, or budgets.
- The overlay helper always calls `instance.dispose()` when the
  `custom(...)` factory resolves, so timers do not outlive the
  overlay even when the user dismisses with `Esc`.

## Attribution

The DSL shape this package adapts is derived from
[`unitdhda/pi-dstui`](https://github.com/unitdhda/pi-dstui) (MIT).
The adapter and overlay glue are written from scratch against
`@oh-my-pi/pi-tui`.
