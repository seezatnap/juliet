# Specifications: juliet-cli

# PRD: juliet CLI

## Summary

A minimal Rust CLI binary (`juliet`) that takes an engine name as its sole argument, prepares the juliet prompt at the current git project root, and launches the selected engine with that prompt.

## Usage

```
juliet claude
juliet codex
```

## Behavior

1. **Accept engine argument** — The first positional argument is the engine name. Valid values: `claude`, `codex`. Any other value prints a short usage message and exits non-zero.

2. **Find git root** — Run `git rev-parse --show-toplevel` to locate the root of the git repository the user is currently inside. If not in a git repo, print an error and exit non-zero.

3. **Prepare prompt file** — At the git root, ensure the `.juliet/` directory exists, then write the juliet prompt to `.juliet/juliet-prompt.md`. The prompt text is embedded in the binary at compile time via `include_str!("../../prompts/juliet.md")` (path relative to `src/main.rs` in the juliet repo).

4. **Launch the engine in interactive mode** — Spawn the engine as a child process (not exec) so the user lands in the engine's interactive/conversational mode with the juliet prompt as the initial message:
   - `claude`: Set env var `IS_SANDBOX=1`, then run `claude --dangerously-skip-permissions` with the prompt content as the initial message argument. The user should end up in claude's interactive session.
   - `codex`: Run `codex --dangerously-bypass-approvals-and-sandbox` with the prompt content as the initial message argument. The user should end up in codex's interactive session.

   The prompt content is read from the just-written `.juliet/juliet-prompt.md` file and passed as the first message. The juliet CLI should wait for the child process to exit and propagate its exit code.

## Constraints

- The binary must remain minimal: select engine, write prompt, exec. No workflow logic in the CLI.
- Use only the Rust standard library (no external crates) unless absolutely necessary. `std::process::Command` and `std::fs` should suffice.
- The prompt source file (`prompts/juliet.md`) is embedded at compile time so the binary is self-contained and works from any directory.
- The project is set up as a Cargo binary crate at the repo root (i.e., `Cargo.toml` + `src/main.rs`).

## File layout (expected after build)

```
juliet/
├── Cargo.toml
├── src/
│   └── main.rs
├── prompts/
│   └── juliet.md          ← existing prompt (embedded at compile)
└── ...
```

## Error cases

| Condition | Behavior |
|---|---|
| No argument given | Print usage, exit 1 |
| Unknown engine name | Print usage, exit 1 |
| Not inside a git repo | Print error, exit 1 |
| Engine binary not found on PATH | Let exec fail naturally (OS error) |

## Out of scope

- Interactive mode, TUI, or multi-turn orchestration (lives in the prompt, not the CLI).
- Config files or runtime prompt overrides (future work if needed).
- Installing or managing engine binaries.

