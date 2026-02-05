# Tasks

## CLI Input Handling

- [ ] (#1) Implement `main` argument parsing using the Rust standard library to accept exactly one positional engine argument (`claude` or `codex`), print a short usage line for missing/unknown values, and exit with code `1` for invalid input [5 pts]
- [ ] (#2) Add a minimal dispatch layer that keeps CLI scope limited to engine selection and handoff (no workflow logic), with centralized usage/error message constants for consistent operator output [5 pts] (blocked by #1)

## Git Root & Prompt Preparation

- [ ] (#3) Implement git root discovery by running `git rev-parse --show-toplevel`, trimming stdout to a usable path, and returning a non-zero error path when the user is not inside a git repo [5 pts] (blocked by #1)
- [ ] (#4) Embed `prompts/juliet.md` at compile time via `include_str!("../../prompts/juliet.md")` (from `src/main.rs`), ensure `<git_root>/.juliet/` exists, and write `.juliet/juliet-prompt.md` on each invocation [5 pts] (blocked by #3)

## Engine Exec Handoff

- [ ] (#5) Implement engine command construction with exact flags (`claude --dangerously-skip-permissions` for claude, `codex --dangerously-bypass-approvals-and-sandbox` for codex). For claude, set the `IS_SANDBOX=1` environment variable on the spawned child process. Pass prompt content (read from the just-written prompt file) as the initial message argument [5 pts] (blocked by #2, #4)
- [ ] (#6) Spawn the engine as a child process (not exec) so the user lands in the engine's interactive/conversational mode with the prompt as the first message. Wait for the child process to exit and propagate its exit code. Let OS-level spawn failures (including missing binary on `PATH`) surface naturally [5 pts] (blocked by #5)

## Validation & Release Readiness

- [ ] (#7) Add unit tests for argument validation, usage output, engine mapping, and required non-zero exits for no-arg/unknown-engine scenarios using standard-library-compatible test setup [5 pts] (blocked by #2, #5)
- [ ] (#8) Add integration/smoke validation for git-repo detection and prompt file generation in a temporary repo, plus a QA checklist covering all PRD error cases and expected crate/file layout assumptions [5 pts] (blocked by #6, #7)
