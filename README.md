# Juliet

A minimal Rust CLI wrapper around Codex. The CLI reads a single prompt file and passes optional user input to Codex; all workflow logic, state transitions, and user-facing behavior live in the prompt markdown file.

**Usage**
```
juliet [message]
```

- `juliet` with no arguments: Juliet reads state and decides what to do (check needs, report status, etc.).
- `juliet "start a project from ~/prds/foo.md"`: Juliet receives the message and decides how to act (init a project, handle feedback, etc.).

**Prompt File**
- `prompts/juliet.md`: unified prompt containing all workflow logic. Juliet reads `.juliet/` state and the operator's input to decide what to do.

**State (.juliet)**
- `.juliet/needs-from-operator.md`: queue of operator needs; when run with no input, Juliet asks the oldest item verbatim and exits.
- `.juliet/projects.md`: active project name, PRD path, and target branch.
- `.juliet/processes.md`: active and completed long-running commands with cleanup annotations.
- `.juliet/artifacts/`: PRDs and helper files created between turns.

**Expected Response Text (Exact Phrases)**
- `Got it, i'll get going on that now.`
- `look at these tasks: <pathtofiles>. if they're good, i'll get going. how many varations  would you like to try?`
- `i'm still working`
- (more sprints remain) `here's the results: <pathtofiles>. if you're happy with them, i'll move on to the next sprint. if you're not, i'll help you edit the tasks.`
- (project complete) `here's the results: <pathtofiles>. looks like everything's done — let me know if you'd like any changes.`

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

All workflow rules (including `.juliet` state management and exact phrasing) are encoded in the prompt file. The Rust CLI stays minimal and does not implement workflow logic.
