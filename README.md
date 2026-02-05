# Juliet

A minimal Rust CLI wrapper around Codex. The CLI reads a single prompt file and passes optional user input to Codex; all workflow logic, state transitions, and user-facing behavior live in prompt markdown.

**Usage**
```bash
juliet [message]
```

- `juliet` with no arguments: Juliet resumes from `.juliet/` state and takes the next logical action.
- `juliet "start a project from ~/prds/foo.md"`: Juliet handles new project initialization.

**Prompt File**
- `prompts/juliet.md`: unified workflow instructions.

**State (.juliet)**
- `.juliet/session.md`: per-conversation bootstrap cache (available engines, default engine, swarm engine syntax).
- `.juliet/needs-from-operator.md`: queue of unresolved operator needs.
- `.juliet/projects.md`: active project metadata (PRD/tasks/specs/branches).
- `.juliet/processes.md`: active and completed long-running `swarm run` jobs.
- `.juliet/artifacts/`: helper artifacts and generated PRDs.

**Boot/Resume Rules**
- On every turn, Juliet first rehydrates context from `.juliet/needs-from-operator.md`, `.juliet/projects.md`, and `.juliet/processes.md`.
- Environment discovery (`swarm --help`, engine checks) runs only at conversation start and is cached in `.juliet/session.md`.
- Discovery re-runs only if `.juliet/session.md` is missing, marked reset-required, or the operator asks to refresh.

**Expected Response Text (Exact Phrases)**
- `Hi, I'm juliet. what do you want to work on today?`
- `Got it, i'll get going on that now.`
- `look at these tasks: <pathtofiles>. if they're good, i'll get going. how many varations  would you like to try?`
- `i'm still working`
- (more sprints remain) `here's the results: <pathtofiles>. if you're happy with them, i'll move on to the next sprint. if you're not, i'll help you edit the tasks.`
- (project complete) `here's the results: <pathtofiles>. looks like everything's done - let me know if you'd like any changes.`

**Swarm Execution Rules**
1. At conversation start: run `swarm --help`, `codex login status`, and `claude -p "PRINT exactly 'CLAUDE_READY'"`.
2. For project setup: run `swarm project init <project> --with-prd <prd_path> <engine-arg>`.
3. For sprint runs: run in background via `tmux` and always include `--no-tui` and `--target-branch`.
4. For each sprint, ask model choice only if multiple engines are available; if one engine is available, use it automatically.
5. After sprint results, ask for feedback, instruct the user to check out the feature branch and edit directly if desired, then reconcile tasks/specs only to reflect approved feedback/user edits.

The Rust CLI remains intentionally thin: it dispatches prompt + optional user input to Codex and does not implement workflow behavior.
