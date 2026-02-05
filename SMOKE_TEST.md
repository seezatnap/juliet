# Juliet Smoke Test Checklist

Manual end-to-end checklist for prompt-driven workflow behavior.

**Prereqs**
- [ ] `codex` in `PATH`.
- [ ] `swarm` in `PATH`.
- [ ] `tmux` in `PATH`.
- [ ] Repo root as current directory.
- [ ] `juliet` binary available (for example: `rustc juliet.rs -o juliet`).

**Setup**
- [ ] Reset state: `rm -rf .juliet`.
- [ ] Create PRD:

```bash
cat > prds/foo.md <<'EOF2'
# Foo

Goal: verify Juliet boot/resume workflow.
EOF2

PRD_PATH=./prds/foo.md
PROJECT=foo
```

**Step 1: Idle Boot Prompt**
- [ ] Run `./juliet`.
- [ ] Verify conversation-start discovery runs (`swarm --help`, `codex login status`, `claude -p ...`).
- [ ] Verify response is exactly `Hi, I'm juliet. what do you want to work on today?` when no pending work exists.
- [ ] Verify `.juliet/session.md` exists and stores available/default engine info.

**Step 2: Init Project**
- [ ] Run `./juliet "start a project from $PRD_PATH"`.
- [ ] Verify `swarm project init $PROJECT --with-prd $PRD_PATH` executes.
- [ ] Verify the response includes `Got it, i'll get going on that now.` then the tasks review phrase.
- [ ] Verify `.juliet/projects.md` records project + target branch + tasks path (and specs path when known).

**Step 3: Start Sprint**
- [ ] Run `./juliet "just one variation please"`.
- [ ] Verify startup discovery does not re-run in this same conversation.
- [ ] If multiple engines are available, verify Juliet asks for per-sprint model choice before launching.
- [ ] Verify `tmux new-session ... swarm run --project $PROJECT --max-sprints 1 --target-branch feature/$PROJECT --no-tui ...` executes.
- [ ] Verify `.juliet/processes.md` records PID, command, branch, log path, start time.

**Step 4: Resume On Boot**
- [ ] Run `./juliet` while sprint is still running (or shortly after).
- [ ] Verify Juliet resumes from `.juliet/processes.md` instead of greeting.
- [ ] Verify running jobs produce `i'm still working`.
- [ ] When completed, verify results phrase is emitted plus explicit feedback request with feature-branch checkout guidance.

**Step 5: Feedback Reconciliation**
- [ ] Make edits on the feature branch.
- [ ] Run `./juliet "I changed X and want Y next"`.
- [ ] Verify Juliet updates out-of-date future tasks/specs only to reflect user feedback/edits.
- [ ] Verify Juliet does not invent extra scope without asking.

**Thin Wrapper Validation**
- [ ] Confirm `juliet.rs` still only dispatches prompts to `codex` with optional user input.
