Runs JavaScript cells sequentially in a persistent Node.js VM context.

<instruction>
Context persists across calls and cells; **variables, functions, and module imports survive—use this.**
**Work incrementally:**
- You **SHOULD** use one logical step per cell (imports, define function, test it, use it)
- You **SHOULD** pass multiple small cells in one call
- You **SHOULD** define small reusable helpers and test them immediately
- You **MUST** put explanations in assistant message or cell title, **MUST NOT** put them in code
**When something fails:**
- Errors identify the failing cell (e.g., "Cell 2 failed")
- You **SHOULD** resubmit only the fixed cell (or fixed cell + remaining cells)
</instruction>

<prelude>
System prelude APIs available in every cell:

```javascript
notify(op, data?)            // emit structured status events for renderer
setExport(name, value)       // persist named exports across calls
getExport(name)              // read persisted export
listExports()                // list persisted export names
exports                      // mutable object for custom user exports
ctx                          // runtime context (cwd, sessionKey, toolCallId, hasUI, toolNames)
```

You **SHOULD** use `notify()` for concise progress/status signals when useful.
You **SHOULD** use `setExport()` or `exports.foo = …` to persist reusable values.
</prelude>
<output>
You get merged console output and evaluated value for each cell.
- `console.log(…)` writes to output
- Final expression value is shown when it is not `undefined`
</output>

<caution>
- Use `reset: true` to clear VM state before execution
- Top-level `await` is not supported; wrap async work in an async function and call it
</caution>

<critical>
- Use this tool for JavaScript/TypeScript evaluation and data shaping inside the agent run
- Prefer plain data values or JSON-like objects for readable output
</critical>

<example name="good">
```javascript
cells: [
  {"title": "imports", "code": "const fs = require('node:fs');"},
  {"title": "helper", "code": "function parse(text) { return JSON.parse(text); }"},
  {"title": "test", "code": "parse('{\"ok\":true}')"},
  {"title": "use", "code": "const data = parse('{\"n\":3}'); data.n * 2"}
]
```
</example>