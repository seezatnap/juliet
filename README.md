# Juliet

A minimal Rust CLI wrapper around Claude/Codex. The CLI selects a role prompt, appends optional operator input, and hands the final prompt to an engine; workflow behavior lives in markdown prompts plus `.juliet/<role>/` state.

**Usage**
```bash
juliet init --role <name>
juliet --role <name> <claude|codex> [operator input...]
juliet <claude|codex> [operator input...]
```

- `juliet` with no args prints usage and exits with code `1`.
- `juliet init` without `--role <name>` prints `Usage: juliet init --role <name>` and exits with code `1`.

**Role Initialization**
- Create or repair a role with `juliet init --role <name>`.
- Success output: `Initialized role: <name>`.
- If both `prompts/<name>.md` and `.juliet/<name>/` already exist, init is idempotent and prints `Role already exists: <name>`.
- Init seeds `prompts/<name>.md` with a role heading, an operator placeholder, and the embedded default prompt content from `prompts/juliet.md`.

**Role Launch**
- Explicit role: `juliet --role <name> <claude|codex> [operator input...]`.
- Implicit role: `juliet <claude|codex> [operator input...]`.
- Explicit launch fails with `Role not found: <name>. Run: juliet init --role <name>` when `.juliet/<name>/` is missing.
- Each launch reads `prompts/<name>.md`, writes it to `.juliet/<name>/juliet-prompt.md`, appends optional `User input:` text, then invokes the selected engine.

**Launch Without `--role`**
- Juliet discovers configured roles from `.juliet/` subdirectories.
- `0` roles: prints `No roles configured. Run: juliet init --role <name>` and exits with code `1`.
- `1` role: auto-selects that role and launches.
- `>1` roles: prints `Multiple roles found. Specify one with --role <name>:` followed by a newline-separated list of role names, then exits with code `1`.

**Role Name Constraints**
- Allowed pattern: `[a-z0-9-]+`.
- Names must be non-empty and cannot start or end with `-`.
- Invalid names fail with: `Invalid role name: <name>. Use lowercase letters, numbers, and hyphens.`

**Project Layout**
```text
prompts/
  juliet.md                    # legacy/default seed prompt, not a configured role by itself
  <role>.md                    # role-specific prompt source

.juliet/
  <role>/
    session.md
    needs-from-operator.md
    projects.md
    processes.md
    artifacts/
    juliet-prompt.md           # runtime copy written at launch
```

Configured roles are discovered from `.juliet/<role>/` directories. A `prompts/<role>.md` file without matching state directory is not treated as configured.

**No Roles Yet?**
- In a fresh project, run:
```bash
juliet init --role director-of-engineering
```
- Then launch with either:
```bash
juliet --role director-of-engineering codex
# or, with one configured role:
juliet codex
```

The Rust CLI stays intentionally thin: prompt + state selection and engine dispatch happen in code, while workflow policy remains in prompt markdown.
