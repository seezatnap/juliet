# Juliet Smoke Test Checklist

This checklist exercises the full workflow using a real PRD path and verifies the expected `.juliet` state plus `swarm` command sequence. It is intentionally manual so the Rust CLI remains a thin wrapper around Codex.

**Prereqs**
- [ ] `codex` is installed and available in `PATH`.
- [ ] `swarm` is installed and available in `PATH`.
- [ ] You are in the repo root.
- [ ] You have a `juliet` binary available (for example: `rustc juliet.rs -o juliet`).

**Setup**
- [ ] Remove any existing state: `rm -rf .juliet`.
- [ ] Create a PRD file and set the project name.

```bash
cat > prds/foo.md <<'EOF'
# Foo

Goal: verify Juliet's end-to-end workflow.
EOF

PRD_PATH=./prds/foo.md
PROJECT=foo
```

**Step 1: Init From PRD Path**
- [ ] Run `./juliet "start a project from $PRD_PATH"`.
- [ ] Verify the first command executed is `swarm --help`.
- [ ] Verify `swarm project init $PROJECT --with-prd $PRD_PATH` executes.
- [ ] Verify the response includes the exact phrase `Got it, i'll get going on that now.` followed by the tasks prompt.
- [ ] Verify `.juliet/needs-from-operator.md` contains a task review + variation count request.
- [ ] Verify `.juliet/projects.md` lists the project name, PRD path, and target branch `feature/$PROJECT`.
- [ ] Verify `.juliet/processes.md` has `Active` and `Completed` sections and the init command is annotated with a cleanup outcome.
- [ ] Verify `.juliet/artifacts/` exists.

**Step 2: Task Review + Variation Count**
- [ ] Review the tasks file path from Step 1 and edit if desired.
- [ ] Run `./juliet "just one variation please"`.
- [ ] Verify the first command executed is `swarm --help`.
- [ ] Verify `tmux new-session -d -s swarm-$PROJECT-feature-$PROJECT "swarm run --project $PROJECT --max-sprints 1 --target-branch feature/$PROJECT --no-tui"` executes.
- [ ] Verify `.juliet/processes.md` records the sprint command with PID, target branch, log path, and start time.

**Step 3: Check Status**
- [ ] Run `./juliet` (no arguments).
- [ ] If needs exist, verify it echoes the oldest need verbatim and exits.
- [ ] If processes are active, verify it checks PIDs and reports status.
- [ ] If results are available, verify the appropriate results phrase is used.

**Step 4: Results Review + Follow-Up Sprint**
- [ ] Run `./juliet "ok, add a test"`.
- [ ] Verify the first command executed is `swarm --help`.
- [ ] Verify `.juliet/artifacts/sprint-1-followups.md` exists.
- [ ] Verify `swarm project init sprint-1-followups --with-prd .juliet/artifacts/sprint-1-followups.md` executes.
- [ ] Verify `tmux new-session -d -s swarm-sprint-1-followups-feature-$PROJECT "swarm run --project sprint-1-followups --max-sprints 1 --target-branch feature/$PROJECT --no-tui"` executes.
- [ ] Verify the results phrase is shown again and `.juliet/needs-from-operator.md` requests another review.

**Thin Wrapper Validation**
- [ ] Confirm `juliet.rs` still only dispatches prompts to `codex` and does not implement workflow logic.
