# Juliet

A minimal Rust CLI wrapper around Codex. The CLI only selects a prompt file and passes user input to Codex; all workflow logic, state transitions, and user-facing behavior live in the prompt markdown files.

**Commands**
- `juliet ask [PRD_PATH]`: start a request; optionally provide a PRD path.
- `juliet next`: ask for the next operator need (or report status if none).
- `juliet feedback "<message>"`: provide operator feedback to move the workflow forward.

**Prompt Files**
- `prompts/ask.md`: workflow for `juliet ask`.
- `prompts/next.md`: workflow for `juliet next`.
- `prompts/feedback.md`: workflow for `juliet feedback`.

**State (.juliet)**
- `.juliet/needs-from-operator.md`: queue of operator needs; `juliet next` asks the oldest item verbatim and exits.
- `.juliet/projects.md`: active project name, PRD path, and target branch.
- `.juliet/processes.md`: active and completed long-running commands with cleanup annotations.
- `.juliet/artifacts/`: PRDs and helper files created between turns.

**Expected Response Text (Exact Phrases)**
- `Got it, i'll get going on that now.`
- `look at these tasks: <pathtofiles>. if they're good, i'll get going. how many varations  would you like to try?`
- `here's the results: <pathtofiles>. if you're happy with them, i'll move on to the next sprint. if you're not, i'll help you edit the tasks.`

**Exact Swarm Command Sequence**
Set `<engine-arg>` to the engine property flag/value shown by `swarm --help`, using the selected engine. Background runs use `tmux` to survive after the CLI exits.
1. `swarm --help`
2. `codex login status`
3. `claude -p "PRINT exactly 'CLAUDE_READY'"`
4. `swarm project init <project> --with-prd <prd_path> <engine-arg>`
5. `tmux new-session -d -s swarm-<project>-<branch-sanitized> "swarm run --project <project> --max-sprints <N> --target-branch feature/<project> --no-tui <engine-arg> > .juliet/artifacts/<project>-<branch-sanitized>-swarm.log 2>&1"`
6. If the operator requests a follow-up (e.g., "ok, add a test"), create `.juliet/artifacts/sprint-1-followups.md` and run:
7. `swarm project init sprint-1-followups --with-prd .juliet/artifacts/sprint-1-followups.md <engine-arg>`
8. `tmux new-session -d -s swarm-sprint-1-followups-<branch-sanitized> "swarm run --project sprint-1-followups --max-sprints 1 --target-branch feature/<project> --no-tui <engine-arg> > .juliet/artifacts/sprint-1-followups-<branch-sanitized>-swarm.log 2>&1"`

All workflow rules (including `.juliet` state management and exact phrasing) are encoded in the prompt files. The Rust CLI stays minimal and does not implement workflow logic.
