# Specifications: multi-role

# Multi-Role Juliet

## Goal

Allow multiple independent Juliet personas (roles) within a single project. Each role has its own prompt file, its own state subdirectory under `.juliet/`, and can be launched independently.

## Current Behavior

- `juliet <claude|codex>` launches a single hardcoded prompt (`prompts/juliet.md`) and writes state to `.juliet/`.
- There is no concept of roles or multiple prompt configurations.

## Target Behavior

### Role-Based State Isolation

Each role gets its own subdirectory under `.juliet/`:

```
.juliet/
  director-of-engineering/
    juliet-prompt.md
    session.md
    needs-from-operator.md
    projects.md
    processes.md
    artifacts/
  director-of-marketing/
    juliet-prompt.md
    session.md
    needs-from-operator.md
    projects.md
    processes.md
    artifacts/
```

### Role Prompt Configuration

Each role has its own prompt file under `prompts/`:

```
prompts/
  director-of-engineering.md
  director-of-marketing.md
```

When a role is initialized, its prompt file is created at `prompts/<role-name>.md`. At runtime the prompt is embedded/read from the role-specific prompt file and written to `<git_root>/.juliet/<role-name>/juliet-prompt.md`.

### CLI Changes

#### `juliet init --role <role_name>`

Initializes a new role:
1. Creates `prompts/<role_name>.md` with a starter template (role name in the heading, placeholder for operator to fill in).
2. Creates `.juliet/<role_name>/` directory structure (empty state files).
3. Prints a confirmation message: `Initialized role: <role_name>`.
4. If the role already exists (prompt file and state dir both present), print `Role already exists: <role_name>` and exit with code 0 (idempotent).

#### `juliet --role <role_name> <claude|codex>`

Launches a specific role:
1. Reads the prompt from `prompts/<role_name>.md`.
2. Writes it to `.juliet/<role_name>/juliet-prompt.md`.
3. Spawns the engine with that prompt as the initial message, same as today.

#### `juliet <claude|codex>` (no `--role`)

When `--role` is omitted:
1. Scan `prompts/` for `*.md` files (excluding any non-role files if needed — see below).
2. If exactly one role prompt exists, use it automatically.
3. If zero role prompts exist, print `No roles configured. Run: juliet init --role <name>` and exit with code 1.
4. If more than one role prompt exists, print `Multiple roles found. Specify one with --role <name>:` followed by a newline-separated list of available role names, and exit with code 1.

### Role Name Rules

- Role names are lowercase alphanumeric plus hyphens: `[a-z0-9-]+`.
- Role names must not be empty or start/end with a hyphen.
- Invalid role names produce: `Invalid role name: <name>. Use lowercase letters, numbers, and hyphens.` and exit with code 1.

### Prompt Discovery

To discover available roles, scan `prompts/` for files matching `*.md`. The role name is the filename stem (without `.md`). The existing `prompts/juliet.md` file is the prompt content that gets embedded in the binary for the default/legacy behavior — it is NOT a role prompt. Role prompts live alongside it in `prompts/`.

To distinguish role prompts from non-role files in `prompts/`: role prompts are those that have a corresponding `.juliet/<role_name>/` state directory. Alternatively, since `juliet init` creates both the prompt file and the state dir, the presence of the state dir is the authoritative signal that a name is a configured role.

Revised discovery: scan `.juliet/` for subdirectories (excluding `artifacts/` and any file that isn't a directory). Each subdirectory name is a role. The prompt file for that role is `prompts/<role_name>.md`.

### Backward Compatibility

- The `include_str!` embedded prompt (`prompts/juliet.md`) is still compiled into the binary for use as the default prompt content in `juliet init` template generation (or can be dropped if not needed).
- If a project has never run `juliet init` and has no `.juliet/<role>/` subdirectories, the CLI should guide the user: `No roles configured. Run: juliet init --role <name>`.

### Error Cases

| Scenario | Output | Exit Code |
|---|---|---|
| `juliet` (no args at all) | `Usage: juliet <command> [options]` with subcommand help | 1 |
| `juliet init` without `--role` | `Usage: juliet init --role <name>` | 1 |
| `juliet init --role ""` | Invalid role name error | 1 |
| `juliet --role missing-role codex` | `Role not found: missing-role. Run: juliet init --role missing-role` | 1 |
| `juliet codex` with 0 roles | `No roles configured. Run: juliet init --role <name>` | 1 |
| `juliet codex` with >1 role | `Multiple roles found...` list | 1 |
| `juliet codex` with 1 role | Auto-select that role, proceed normally | 0 (engine exit code) |

### Non-Goals

- Runtime role switching within a session.
- Role-specific engine preferences (the engine is always a CLI argument).
- Prompt templating or variable substitution — prompts are plain markdown.

