- Swarm project init leaves a placeholder `tasks.md`; populate it from the PRD before asking the operator for variation count.
- Keep PRDs and task lists scoped to the user request; avoid injecting the Rust CLI constraint into unrelated content tasks.

- `swarm project init projectname --with-prd` can fail if the default engine (claude) is unavailable; it falls back to the default `tasks.md` and prints a warning.
- Detect engine availability at the start of each run with `codex login status` (look for `Logged in using`) and `claude -p "PRINT exactly 'CLAUDE_READY'"` (expects `CLAUDE_READY`), then pass the selected engine via the `swarm` engine property.
- Background `swarm run` jobs can terminate when Juliet exits; use `tmux` sessions and record the pane PID so they survive after the CLI finishes.
- In `juliet next`, report completed results once per process (track with `reported_on` in `.juliet/processes.md`) even if other runs are still active, and avoid adding a needs entry until all runs finish.
- The `claude` CLI is not installed here (command not found); use `codex` for `swarm` engine selection.
