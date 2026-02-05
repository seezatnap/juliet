- Swarm project init leaves a placeholder `tasks.md`; populate it from the PRD before asking the operator for variation count.
- Keep PRDs and task lists scoped to the user request; avoid injecting the Rust CLI constraint into unrelated content tasks.

- `swarm project init --with-prd` can fail if the default engine (claude) is unavailable; it falls back to the default `tasks.md` and prints a warning.
- Detect engine availability at the start of each run with `codex login status` (look for `Logged in using`) and `claude -p "PRINT exactly 'CLAUDE_READY'"` (expects `CLAUDE_READY`), then pass the selected engine via the `swarm` engine property.
- Background `swarm run` jobs can terminate when Juliet exits; use `nohup` so they survive after the CLI finishes.
- Created project: --with-prd
  Directory: .swarm-hug/--with-prd
  Tasks:     .swarm-hug/--with-prd/tasks.md
  Specs:     .swarm-hug/--with-prd/specs.md
  Chat:      .swarm-hug/--with-prd/chat.md
  Logs:      .swarm-hug/--with-prd/loop
  Worktrees: .swarm-hug/--with-prd/worktrees

To work on this project, use:
  swarm --project --with-prd run
  swarm -p --with-prd status reports the tasks file path (typically ) in its output; capture that for operator review.
- `swarm project init --with-prd` reports the tasks file path (typically `.swarm-hug/<project>/tasks.md`) in its output; capture that for operator review.
