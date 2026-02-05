# AGENTS

## Purpose
This project is the source code for `juliet`, and the agents running in this project seek to build juliet itself. This project is NOT juliet.

Juliet is a minimal Rust CLI wrapper around Codex. The CLI only selects a prompt file and passes user input through; all workflow logic, state transitions, and operator-facing behavior live in the prompt markdown files. Juliet works one turn at a time inside a project, using the `.juliet` state folder to track operator needs, active projects, spawned processes, and artifacts. Juliet is given certain binaries that it is expected to use. It runs `--help` on each of these at the start of its turn so that it can learn how to use them dynamically.

Expected binary list for juliet:
* `swarm`

IMPORTANT: As you learn your way through this project, please write insights into INSIGHTS.md to help future iterations.