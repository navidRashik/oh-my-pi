Mounts a safe `pi-dstui` DSL component as an interactive TUI overlay and returns its settle value.

<conditions>
- A short, bespoke interactive UI is the cleanest way to gather a decision (e.g. a custom selector, a value picker, a confirmation with side info)
- The component is small, side-effect-free, and can be expressed in the `pi-dstui` DSL
</conditions>

<instruction>
- Pass `source` for inline DSL, or `store` for a persisted module (mutually exclusive — but if both are passed and `save: true`, the source is persisted under `store` first)
- `config` is forwarded to the component's parameter list (kebab/snake/camel keys all resolve)
- `componentName` selects which `defcomponent` to instantiate; defaults to the first declaration
- `saveState: true` persists the instance's last visible state under `store` after settle (requires `store`)
- The overlay blocks until the component calls `(emit value)` or the user cancels with `Esc`
</instruction>

<critical>
- This tool is gated behind `dstui.enabled = true` and requires an interactive UI; do not call it in print/headless mode
- DSL source is parsed under strict safety caps (nodes, depth, eval steps, output cells, timers); oversized or syntactically invalid source is rejected before execution
- Prototype keys (`__proto__`, `prototype`, `constructor`) are denied in all dynamic access paths
- The component cannot reach the filesystem, network, or any host API — only the bound DSL builtins
</critical>

<examples>
# Inline picker
source: "(defcomponent pick (items)\n  (state (idx 0))\n  (view (each it items (text (str (if (= idx __index__) \"> \" \"  \") it))))\n  (bind :down (set! idx (+ idx 1)))\n  (bind :up (set! idx (- idx 1)))\n  (bind :enter (emit idx)))", config: {"items": ["apple","banana","cherry"]}

# Reuse a stored module and persist its state on settle
store: "inbox-picker", saveState: true, config: {"selected_index": 2}
</examples>
