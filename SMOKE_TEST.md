# Juliet Smoke Test Checklist

Manual end-to-end checklist for the multi-role CLI workflow.

**Prereqs**
- [ ] `codex` in `PATH`.
- [ ] `swarm` in `PATH`.
- [ ] Optional: `claude` in `PATH` (if testing Claude launch path).
- [ ] Repo root as current directory.
- [ ] `juliet` binary available (for example: `rustc juliet.rs -o juliet`).

**Setup**
- [ ] Reset state: `rm -rf .juliet`.
- [ ] Use a test role name:

```bash
ROLE=director-of-engineering
```

**Step 1: Base Usage Errors**
- [ ] Run `./juliet`.
- [ ] Verify stderr includes:
  - `Usage: juliet <command> [options]`
  - `juliet init --role <name>`
  - `juliet --role <name> <claude|codex>`
  - `juliet <claude|codex>`
- [ ] Verify exit code is non-zero.

**Step 2: Init Usage Error**
- [ ] Run `./juliet init`.
- [ ] Verify stderr is exactly `Usage: juliet init --role <name>`.
- [ ] Verify exit code is non-zero.

**Step 3: Role Initialization**
- [ ] Run `./juliet init --role "$ROLE"`.
- [ ] Verify stdout is `Initialized role: $ROLE`.
- [ ] Verify `prompts/$ROLE.md` exists.
- [ ] Verify `.juliet/$ROLE/` exists with:
  - `session.md`
  - `needs-from-operator.md`
  - `projects.md`
  - `processes.md`
  - `artifacts/`
- [ ] Verify `.juliet/$ROLE/juliet-prompt.md` does not exist until launch.

**Step 4: Init Idempotency**
- [ ] Run `./juliet init --role "$ROLE"` again.
- [ ] Verify stdout is `Role already exists: $ROLE`.
- [ ] Verify exit code is `0`.

**Step 5: Explicit Launch**
- [ ] Run `./juliet --role "$ROLE" codex "status check"` (or `claude` if preferred).
- [ ] Verify the selected engine is invoked.
- [ ] Verify `.juliet/$ROLE/juliet-prompt.md` is written from `prompts/$ROLE.md`.
- [ ] Verify operator input is appended as:
  - `User input:`
  - `status check`

**Step 6: Implicit Launch (Single Role)**
- [ ] Ensure only one configured role exists under `.juliet/`.
- [ ] Run `./juliet codex`.
- [ ] Verify launch succeeds without `--role`.

**Step 7: No Roles Configured Guidance**
- [ ] In a clean temp directory with no `.juliet/<role>/` state, run `<path-to-juliet> codex`.
- [ ] Verify stderr is `No roles configured. Run: juliet init --role <name>`.
- [ ] Verify exit code is non-zero.

**Step 8: Multiple Roles Guidance**
- [ ] Create a second role: `./juliet init --role director-of-marketing`.
- [ ] Run `./juliet codex`.
- [ ] Verify stderr starts with `Multiple roles found. Specify one with --role <name>:` and lists both role names on separate lines.
- [ ] Verify exit code is non-zero.

**Step 9: Role Name Validation**
- [ ] Run `./juliet init --role Invalid_Name`.
- [ ] Verify stderr is `Invalid role name: Invalid_Name. Use lowercase letters, numbers, and hyphens.`
- [ ] Verify exit code is non-zero.
