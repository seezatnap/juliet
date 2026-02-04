# Tasks

## CLI Commands

- [ ] (#1) Implement `juliet ask`, `juliet next`, and `juliet feedback "<msg>"` command handlers in `juliet.rs`, wiring each to the appropriate prompt file and ensuring each command returns the specified user-facing confirmations and behaviors [5 pts]
- [ ] (#2) Add startup behavior so `juliet` always runs `swarm --help` before doing anything else, and ensure the dangerous-mode/no-confirmations execution context is enforced for codex runs [5 pts]

## Prompt System

- [ ] (#3) Create the core prompt markdown files under `prompts/` that encode the full swarm workflow: PRD intake, `swarm project init`, task review request, variations question, `swarm run`, results review, and follow-up sprint creation for feedback like “add a test” [5 pts]
- [ ] (#4) Implement the prompt logic to be state-driven by `.juliet` folder contents, including next-action selection and one-turn-at-a-time behavior [5 pts]

## Juliet State Files

- [ ] (#5) Define and integrate `.juliet/needs-from-operator.md`, `.juliet/projects.md`, `.juliet/processes.md`, and `.juliet/artifacts/` usage so `juliet next` reads needs, `juliet ask`/`feedback` update state, and `processes.md` is annotated and cleaned up when processes finish [5 pts]

## Swarm Workflow Integration

- [ ] (#6) Implement the end-to-end project flow: PRD file creation when needed, `swarm project init <name> --with-prd <path>`, response with task file path and variations prompt, then `swarm run --project <name> --max-sprints 1 --target-branch feature/<name>` and results review [5 pts]

## Follow-Up Sprint Handling

- [ ] (#7) Add follow-up sprint creation flow after user feedback (e.g., “ok, add a test”): generate a small PRD in `.juliet/artifacts/`, run `swarm project init sprint-1-followups --with-prd ...`, then `swarm run --project sprint-1-followups --max-sprints 1 --target-branch feature/foo`, and request review for next sprint [5 pts]
