# manage_skill

> Create, update, or delete isolated managed skills used by the auto-learn workflow.

## Source
- Entry: `packages/coding-agent/src/tools/manage-skill.ts`
- Model-facing prompt: `packages/coding-agent/src/prompts/tools/manage-skill.md`
- Key collaborators:
  - `packages/coding-agent/src/autolearn/managed-skills.ts` — managed skill directory, name validation, create/update/delete primitives.
  - `packages/coding-agent/src/extensibility/skills.ts` — authored-skill collision checks.

## Inputs

| Field | Type | Required | Description |
|---|---|---:|---|
| `action` | `"create" | "update" | "delete"` | Yes | Operation to perform. |
| `name` | `string` | Yes | Kebab-case skill name. |
| `description` | `string` | Create/update | One-line description of when to use the skill. |
| `body` | `string` | Create/update | `SKILL.md` body in Markdown, without frontmatter. |

## Outputs
- Delete: `Deleted managed skill "<name>".` with `{ action: "delete", name }`.
- Create/update: `Created/Updated managed skill "<name>" (managed-skills/<path>).` with `{ action, name }`.
- Authored-skill name collision on `create`: error text explains that managed skills cannot override authored skills and asks for a different name.

## Flow
1. `ManageSkillTool.createIf(...)` exposes the tool only when `autolearn.enabled=true`.
2. Validation requires `description` and `body` for `create` and `update`; `delete` needs only `name`.
3. `create` refuses names already claimed by authored skills, because managed skills resolve below authored skills and would never surface.
4. Writes target `~/.omp/agent/managed-skills`; managed skills are surfaced like normal skills in future sessions.

## Limits & Caps
- This is a write-tier tool.
- Managed skills are separate from user-authored skills.
- `body` must omit frontmatter; it is generated from `name` and `description`.

## Errors
- Validation rejects create/update without both `description` and `body`.
- Create fails when the managed skill already exists or an authored skill owns the name.
- Update/delete fail when the managed skill does not exist.
