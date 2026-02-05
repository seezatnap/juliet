"i'd like to start a new project from ~/prds/foo.md"

juliet responds with "Got it, i'll get going on that now."

juliet thinks: okay, let's turn that into tasks.

runs `swarm project init foo --with-prd ~/prds/foo.md`

once done, juliet says "look at these tasks: <pathtofiles>. if they're good, i'll get going. how many varations  would you like to try?"

the user edits the tasks, or not. and says "okay, go for it. just one variation please."

juliet thinks: okay, let's run the sprint.

runs `tmux new-session -d -s swarm-foo-feature-foo "swarm run --project foo --max-sprints 1 --target-branch feature/foo --no-tui"`

once done, juliet checks the tasks file. if tasks remain she says "here's the results: <pathtofiles>. if you're happy with them, i'll move on to the next sprint. if you're not, i'll help you edit the tasks." if all tasks are done she says "here's the results: <pathtofiles>. looks like everything's done — let me know if you'd like any changes."

the user responds: "ok, add a test"

juliet creates a new project called "sprint-1-followups" with new tasks based on the user's prompt. in practice she'll write a small PRD for the asks, then run:

`swarm project init sprint-1-followups --with-prd .juliet/artifacts/sprint-1-followups.md`

`tmux new-session -d -s swarm-sprint-1-followups-feature-foo "swarm run --project sprint-1-followups --max-sprints 1 --target-branch feature/foo --no-tui"`

once done, she'll request another review, or ask if the user is ready to go on to the next sprint.





the script itself is very thin. there should be:

prompts/(several prompt files in markdown format)
juliet.rs

the prompt contains EVERYTHING that juliet needs to know about using swarm and the process we expect her to follow. she runs one turn at a time, taking the next logical action based on the state of the .juliet folder in the project she's operating within

.juliet/needs-from-operator.md -- what juliet needs from the operator. when the operator runs `juliet next` this will be what she refers to. The user must address the item to remove it from the list.
.juliet/projects.md -- high level projects that juliet is operating at the moment.
.juliet/processes.md -- processes she spawned previously. check these, clean up the file if they're gone and see what the result of the work was. should be annotateed to explain what command it was running, its purpose, and if it's gone she should check to see what came of the work / what feedback she needs from the operator.
.juliet/artifacts -- a place where juliet can store files to provide to `swarm` or for herself to learn more about the project between turns.


juliet.rs simply exposes

`juliet ask` -- ask juliet to do something. juliet makes a PRD (possibly from your prd) to start a project. she responds with a confirmation.
`juliet next` -- juliet asks you something from her needs list. exits immediately. if she has no current needs she tells you what she's working on (what processes and projects are running) and asks you to check back in a bit.
`juliet feedback "<msg>"` -- you give juliet feedback. she decides what to do about it, then responds with a confirmation.

internally all of these commands go straight to prompts, which themselves go to `codex` (to start). codex runs in dangerous mode with no confirmations for permission checks (we'll only run juliet in a sandbox). juliet always starts by running `swarm --help` to understand what she has available to her.
