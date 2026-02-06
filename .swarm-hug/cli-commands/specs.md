# Specifications: cli-commands

# CLI Commands: reset-prompt, clear-history, exec

## Overview

Add three new commands to the Juliet Rust CLI to improve operator workflow: resetting a role's prompt to the bin default, clearing a role's state/history, and executing a single non-interactive turn.

## Context

Juliet is a minimal Rust CLI (`juliet.rs` + `role_name.rs` + `role_state.rs`) that parses commands, stages prompts, and spawns an engine (`claude` or `codex`). Currently it supports `init` and `launch` (explicit/implicit). All files reference the project root at `/root/Sites/juliet/`.

The prompt for each role lives at `.juliet/<role>/prompt.md`. The bin default prompt seed is embedded at compile time via `include_str!("prompts/juliet.md")` as `DEFAULT_PROMPT_SEED`.

Role state lives under `.juliet/<role>/` and includes: `session.md`, `needs-from-operator.md`, `projects.md`, `processes.md`, `artifacts/`, `prompt.md`, and `juliet-prompt.md`.

## Feature 1: `juliet reset-prompt --role <name>`

Reset a role's prompt back to the bin default.

### Behavior

1. Validate `<name>` with `is_valid_role_name()`.
2. Verify `.juliet/<name>/` exists (error if not: "Role '<name>' is not initialized.").
3. Regenerate the prompt using the same template as `init`: `# {role_name}\n\n{OPERATOR_PLACEHOLDER}\n\n## Default Prompt Seed\n\n{DEFAULT_PROMPT_SEED}`.
4. Overwrite `.juliet/<name>/prompt.md` with the regenerated content.
5. Print: `prompt reset to default for role '<name>'`.

### CLI parsing

Add a `ResetPrompt { role_name: String }` variant to `CliCommand`. Parse when `args[0] == "reset-prompt"`, expecting `["reset-prompt", "--role", "<name>"]`.

## Feature 2: `juliet clear-history --role <name>`

Clear all state/history for a role, preserving the role directory and its prompt.

### Behavior

1. Validate `<name>` with `is_valid_role_name()`.
2. Verify `.juliet/<name>/` exists (error if not: "Role '<name>' is not initialized.").
3. Reset these files to empty/default content:
   - `.juliet/<name>/session.md` → empty string
   - `.juliet/<name>/needs-from-operator.md` → empty string
   - `.juliet/<name>/projects.md` → empty string
   - `.juliet/<name>/processes.md` → empty string
   - `.juliet/<name>/juliet-prompt.md` → delete if exists (it's a runtime artifact)
4. Clear the `.juliet/<name>/artifacts/` directory (remove all files inside it, keep the directory).
5. Print: `history cleared for role '<name>'`.

### CLI parsing

Add a `ClearHistory { role_name: String }` variant to `CliCommand`. Parse when `args[0] == "clear-history"`, expecting `["clear-history", "--role", "<name>"]`.

## Feature 3: `juliet exec --role <name> <claude|codex> <message>`

Execute a single non-interactive turn. Instead of opening the engine's interactive TUI, this appends the message to the prompt using a `USER COMMAND:` marker and passes it through to the engine via its non-interactive/print flag.

### Behavior

1. Validate role name (explicit `--role <name>` required, OR implicit single-role discovery — same rules as current launch).
2. Stage the prompt: read `.juliet/<role>/prompt.md`, write to `.juliet/<role>/juliet-prompt.md`.
3. Build the final prompt by appending: `\n\nUser input:\n<message>` (reusing `build_launch_prompt`).
4. Run the engine in **print mode** (non-interactive, single response):
   - For claude: `claude -p <prompt>` with `--dangerously-skip-permissions` and `IS_SANDBOX=1` env var (i.e., `claude --dangerously-skip-permissions -p <prompt>`).
   - For codex: `codex --dangerously-bypass-approvals-and-sandbox -q <prompt>` (the `-q` / `--quiet` flag runs non-interactively).
5. Return the engine's exit code.

### CLI parsing

Add an `Exec` variant to `CliCommand`:
```rust
Exec {
    role_name: Option<String>,
    engine: Engine,
    message: String,
}
```

Parse when `args[0] == "exec"`. Support both forms:
- `juliet exec --role <name> <claude|codex> <message...>`
- `juliet exec <claude|codex> <message...>` (implicit single-role)

The `<message...>` is all remaining args joined by spaces (same pattern as current operator input).

### Engine functions

Add `run_claude_print(prompt, cwd)` and `run_codex_quiet(prompt, cwd)` (or a flag parameter on existing functions) that invoke the engine in non-interactive mode.

## Testing

Each feature needs:
- Unit tests for CLI parsing (valid args, missing args, bad role name).
- Integration tests (using the existing `TestDir` + `MockCodex` pattern):
  - `reset-prompt`: verify prompt.md is overwritten with default content.
  - `clear-history`: verify state files are emptied, artifacts cleared, prompt.md preserved.
  - `exec`: verify engine is called with `-p` / `-q` flag and correct prompt content.

## Usage update

Update the usage string at the top of `juliet.rs` to include:
```
juliet reset-prompt --role <name>
juliet clear-history --role <name>
juliet exec --role <name> <claude|codex> <message...>
juliet exec <claude|codex> <message...>
```

