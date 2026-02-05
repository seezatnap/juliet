# Tasks

## Global Constraint — The CLI must be a minimal Rust wrapper around Codex, with all workflow logic living in prompts

## Prompting & Workflow

- [x] (#1) Author `prompts/` markdown files (`ask.md`, `next.md`, `feedback.md`) that fully encode the PRD flow, including exact user-facing phrases, required `swarm` commands (`swarm project init ...`, `swarm run ...`), variation prompt, sprint results prompt, follow-up sprint creation for “add a test,” and the rule to start every run with `swarm --help`; end constraint: keep the Rust CLI as a minimal prompt dispatcher to Codex. [5 pts] (A)
- [A] (#2) Extend the prompts with `.juliet` state rules: read/write `.juliet/needs-from-operator.md`, `.juliet/projects.md`, `.juliet/processes.md`, and `.juliet/artifacts/`, ensure `juliet next` behavior when needs exist vs not, require process cleanup annotations, and store follow-up PRDs in `.juliet/artifacts/sprint-1-followups.md`; end constraint: the Rust CLI stays thin and offloads logic to prompts. [5 pts] (blocked by #1)

## CLI Implementation

- [x] (#3) Implement `juliet.rs` as a minimal Rust CLI with subcommands `ask`, `next`, `feedback` that only load the corresponding prompt file and invoke `codex` in dangerous mode, passing through the user’s PRD path or feedback message and working directory; end constraint: `juliet` remains a minimal Rust wrapper around Codex, nothing more. [5 pts] (A)

## Integration & Validation

- [ ] (#4) Create a smoke-test script or checklist that exercises the full scenario (init from PRD path, tasks review + variation count, sprint run, results review, “add a test” follow-up sprint) and verifies expected `.juliet` files and `swarm` commands; end constraint: validate behavior without expanding the Rust CLI beyond a thin wrapper. [5 pts] (blocked by #1, #2, #3)

## Documentation

- [A] (#5) Add concise docs describing the commands, prompt files, `.juliet` folder semantics, expected response text, and the exact `swarm` command sequence, explicitly noting that all workflow logic lives in prompts; end constraint: documentation reinforces a minimal Rust CLI wrapper around Codex. [5 pts] (blocked by #1, #3)
