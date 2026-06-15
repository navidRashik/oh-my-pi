# learn

> Capture a reusable lesson to long-term memory, optionally creating or updating a managed skill in the same call.

## Source
- Entry: `packages/coding-agent/src/tools/learn.ts`
- Model-facing prompt: `packages/coding-agent/src/prompts/tools/learn.md`
- Key collaborators:
  - `packages/coding-agent/src/autolearn/managed-skills.ts` — managed skill path/name validation and `SKILL.md` writes.
  - `packages/coding-agent/src/memory-backend/local-backend.ts` — local file-backed lesson storage.
  - `packages/coding-agent/src/hindsight/state.ts` and `packages/coding-agent/src/mnemopi/state.ts` — remote/local memory retention.

## Inputs

| Field | Type | Required | Description |
|---|---|---:|---|
| `memory` | `string` | Yes | Durable, self-contained lesson. Include what, when, and why. |
| `context` | `string` | No | Source context for the lesson. |
| `skill` | `{ action, name, description, body }` | No | Also create or update a managed skill. `action` is `create` or `update`; `name` is kebab-case; `body` is Markdown without frontmatter. |

## Outputs
- Memory only: `Lesson stored.` or `Lesson queued for retention.` with `details.skill = null`.
- Memory plus skill: `<lesson result>. Created/Updated managed skill "<name>".` with `details.skill = <name>`.
- Authored-skill name collision on `create`: error text explains that managed skills cannot override authored skills; the memory lesson was already stored or queued.

## Flow
1. `LearnTool.createIf(...)` exposes the tool only when `autolearn.enabled=true` and `memory.backend` is `hindsight`, `mnemopi`, or `local`.
2. `execute(...)` stores the lesson through the active backend:
   - `mnemopi` writes a fact with source `coding-agent-learn` and fails if no memory id is returned;
   - `local` appends through the local backend and fails when sanitization leaves nothing to store;
   - `hindsight` queues the lesson for asynchronous retention.
3. If `skill` is present, the tool validates the managed skill name, refuses authored-skill shadowing on `create`, then writes the managed skill under `~/.omp/agent/managed-skills`.

## Limits & Caps
- Availability requires both `autolearn.enabled` and an active memory backend.
- The skill side never edits user-authored skills; authored skills keep precedence.
- `skill.body` must omit frontmatter; frontmatter is generated from `name` and `description`.

## Errors
- `Mnemopi backend is not initialised for this session.`
- `Hindsight backend is not initialised for this session.`
- `Lesson was empty after sanitization; nothing stored.`
- Managed skill write failures surface after the memory outcome, because the lesson may already be stored or queued.
