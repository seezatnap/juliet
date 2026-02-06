# Tasks

## CLI Parsing & Usage

- [x] (#1) Extend `CliCommand` and argument parsing to add `ResetPrompt { role_name }`, `ClearHistory { role_name }`, and `Exec { role_name: Option<String>, engine, message }`; support `reset-prompt --role <name>`, `clear-history --role <name>`, `exec --role <name> <claude|codex> <message...>`, and `exec <claude|codex> <message...>`; join remaining args into one message string; preserve existing validation/error behavior for missing args and bad role names; and update the top-level usage text in `juliet.rs` with all new command forms [5 pts] (A)

## Reset Prompt Command

- [x] (#2) Implement `juliet reset-prompt --role <name>` command execution: validate role name via `is_valid_role_name()`, verify `.juliet/<name>/` exists (error exactly `Role '<name>' is not initialized.` if not), regenerate prompt with the same `init` template (`# {role_name}\n\n{OPERATOR_PLACEHOLDER}\n\n## Default Prompt Seed\n\n{DEFAULT_PROMPT_SEED}`), overwrite `.juliet/<name>/prompt.md`, and print `prompt reset to default for role '<name>'` [5 pts] (blocked by #1) (A)

## Clear History Command

- [x] (#3) Implement `juliet clear-history --role <name>` command execution: validate role name, verify `.juliet/<name>/` exists (same not-initialized error), empty `session.md`, `needs-from-operator.md`, `projects.md`, and `processes.md`, delete `.juliet/<name>/juliet-prompt.md` if present, clear all contents under `.juliet/<name>/artifacts/` while preserving the directory, keep `.juliet/<name>/prompt.md` unchanged, and print `history cleared for role '<name>'` [5 pts] (blocked by #1) (A)

## Exec Command & Engine Runtime

- [x] (#4) Implement `juliet exec` single-turn flow: resolve role from explicit `--role <name>` or implicit single-role discovery using existing launch rules, stage `.juliet/<role>/prompt.md` into `.juliet/<role>/juliet-prompt.md`, and build the final prompt by reusing `build_launch_prompt` behavior to append `\n\nUser input:\n<message>` [5 pts] (blocked by #1) (B)
- [ ] (#5) Add non-interactive engine execution for exec and wire exit-code propagation: implement `run_claude_print(prompt, cwd)` to call `claude --dangerously-skip-permissions -p <prompt>` with `IS_SANDBOX=1`, implement `run_codex_quiet(prompt, cwd)` to call `codex --dangerously-bypass-approvals-and-sandbox -q <prompt>`, dispatch by selected engine, and return the engine process exit code unchanged [5 pts] (blocked by #4)

## Testing

- [x] (#6) Add unit tests for parser coverage of `reset-prompt` and `clear-history`: valid argument forms, missing/invalid `--role` cases, and bad role name validation paths [5 pts] (blocked by #1) (C)
- [x] (#7) Add unit tests for parser coverage of `exec`: explicit-role and implicit-role forms, engine parsing for `claude|codex`, `<message...>` joining behavior, and missing-arg/bad-role failures [5 pts] (blocked by #1) (B)
- [ ] (#8) Add integration tests (existing `TestDir` pattern) for `reset-prompt` to verify `prompt.md` is overwritten with regenerated default template content and success output is correct [5 pts] (blocked by #2)
- [ ] (#9) Add integration tests (existing `TestDir` pattern) for `clear-history` to verify target state files are emptied, `juliet-prompt.md` is removed if present, artifacts are cleared but directory/prompt are preserved, and success output is correct [5 pts] (blocked by #3)
- [ ] (#10) Add integration tests (existing `TestDir` + `MockCodex` pattern) for `exec` to verify engine invocation uses `-p`/`-q` non-interactive flags and required safety flags/env, prompt content includes appended `User input` message, and command exit code matches engine exit code [5 pts] (blocked by #5)

## Follow-up tasks (from sprint review)
- [x] (#11) Mark task #1 as complete in tasks.md — parsing for `ResetPrompt`, `ClearHistory`, and `Exec` is fully implemented with tests on the feature branch (blocked by #1) (A)
- [ ] (#12) Mark tasks #6 and #7 as complete in tasks.md — parser unit tests for `reset-prompt`, `clear-history`, and `exec` were delivered as part of #1's implementation on the feature branch (blocked by #1)
- [x] (#13) Update tasks.md to mark #1 as complete — the sprint delivered all parsing, usage text, and stub handlers specified by #1 (C)
- [x] (#14) Update tasks.md to mark #6 and #7 as complete — comprehensive parser unit tests for `reset-prompt`, `clear-history`, and `exec` were included in the #1 implementation (B)

## Follow-up tasks (from sprint review)
- [x] (#15) Fix task #2 marker in tasks.md — it shows `[A]` (agent assignment) instead of `[x]`; the reset-prompt implementation is fully complete with unit and integration tests (B)
- [x] (#16) Mark tasks #8 and #9 as complete in tasks.md — integration tests for reset-prompt (6 tests) and clear-history (6 tests) were delivered in this sprint (C)
- [x] (#17) Mark tasks #11, #12, and #14 as complete in tasks.md — the tasks they describe (marking #1, #6, #7 complete) are already done (C)
- [ ] (#18) Update exec integration tests to verify non-interactive engine flags (`-p`/`-q`) once #5 is implemented — current tests assert interactive-mode args only (blocked by #5)

## Follow-up tasks (from sprint review)
- [ ] (#19) Fix task #2 marker in tasks.md — it still shows `[A]` instead of `[x]`; the merge at commit `1c7eb42` appears to have reverted Betty's fix
- [ ] (#20) Fix task #15 marker in tasks.md — it shows `[B]` (agent assignment) instead of `[x]`; Betty completed the task but the marker was not updated to complete
