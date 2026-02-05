# Tasks

## Core Role Infrastructure

- [x] (#1) Implement a reusable role-name validation module that enforces `[a-z0-9-]+`, rejects empty names and leading/trailing hyphens, and returns the exact invalid-name error text: `Invalid role name: <name>. Use lowercase letters, numbers, and hyphens.` [5 pts] (A)
- [x] (#2) Build role state filesystem helpers to create `.juliet/<role>/` with `session.md`, `needs-from-operator.md`, `projects.md`, `processes.md`, `artifacts/`, and support for runtime `juliet-prompt.md`, plus existence checks used for idempotent init behavior [5 pts] (blocked by #1) (A)
- [x] (#3) Implement role discovery by scanning `.juliet/` subdirectories (ignoring non-directories and `artifacts/`), deriving role names from directory names, and resolving each role’s prompt as `prompts/<role>.md` while ensuring legacy `prompts/juliet.md` is never treated as a configured role [5 pts] (blocked by #1) (A)

## CLI Command Flows

- [x] (#4) Refactor CLI parsing/routing to support `juliet init --role <name>`, `juliet --role <name> <claude|codex>`, and `juliet <claude|codex>`, including required usage errors for missing arguments (`juliet` with no args and `juliet init` without `--role`) [5 pts] (A)
- [x] (#5) Implement `juliet init --role <role_name>` end-to-end: validate role name, create `prompts/<role>.md` starter template (role heading + operator placeholder, optionally seeded from embedded default prompt), create role state structure, print `Initialized role: <role_name>`, and return `Role already exists: <role_name>` with exit code 0 when prompt file and state dir both already exist [5 pts] (blocked by #2, #4) (A)
- [x] (#6) Implement explicit-role launch `juliet --role <role_name> <claude|codex>`: verify role exists via `.juliet/<role>/`, read `prompts/<role>.md`, write prompt content to `.juliet/<role>/juliet-prompt.md`, and launch engine with the same initial-message behavior as current implementation [5 pts] (blocked by #3, #4) (A)
- [x] (#7) Implement implicit-role launch `juliet <claude|codex>` (no `--role`): discover roles from `.juliet/`, auto-select when exactly one role exists, fail with `No roles configured. Run: juliet init --role <name>` for zero roles, and fail with `Multiple roles found. Specify one with --role <name>:` followed by newline-separated role names when multiple roles exist [5 pts] (blocked by #3, #4, #6) (A)

## Error Handling & Compatibility

- [x] (#8) Normalize output text and exit codes for all PRD-defined scenarios (invalid name, missing role, missing args, zero/multiple roles, idempotent init) and ensure successful run paths return the spawned engine’s exit code [5 pts] (blocked by #5, #6, #7) (A)

## Testing

- [x] (#9) Add unit tests for role validation, state scaffolding helpers, and role discovery filtering/mapping behavior (including edge cases for invalid names, non-directory entries, and excluded directories) [5 pts] (blocked by #1, #2, #3) (B)
- [x] (#10) Add integration/CLI tests that cover the full scenario matrix and exact message/exit-code expectations from the PRD, including `init` idempotency, explicit role launch, implicit single-role auto-selection, and all specified failures [5 pts] (blocked by #5, #6, #7, #8, #9)

## Documentation

- [x] (#11) Update README/help documentation for the multi-role workflow, including role initialization, launching with/without `--role`, naming constraints, `.juliet/<role>/` layout, prompt file locations, and guidance for projects with no configured roles [5 pts] (blocked by #5, #7, #8)

## Follow-up tasks (from sprint review)
- [x] (#12) Restore launch-time user input passthrough (removed in the CLI refactor) so explicit and implicit role runs append operator input to the prompt before invoking the engine (blocked by #6, #7) (B)

## Follow-up tasks (from sprint review)
- [x] (#13) Validate `juliet --role <name> <claude|codex>` with the existing role-name rules before building paths so invalid or traversal inputs (for example `../...`) are rejected with the invalid-name error instead of reading/writing outside `.juliet/<role>/`. (A)

## Follow-up tasks (from sprint review)
- [x] (#14) Resolve inconsistent handling of role name `juliet`: `juliet init --role juliet` succeeds, but implicit launch ignores `.juliet/juliet` and returns `No roles configured`; either reserve `juliet` in role validation/init or include it in role discovery, and add regression coverage. (A)
