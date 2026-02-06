# Juliet

Juliet is a simple bootup script for Claude Code or Codex. By default, it's built to be an expert at using Swarm Hug, another library I wrote which manages swarms of agents in a sprint-like configuration. But you can tweak the prompt to make it an expert at anything else.

## Initializing

You fire up Juliet by creating a new role. This will use the default prompt, which is a team orchestrator / director:

`juliet init --role eng-lead`

Boot Juliet up:

`juliet --role eng-lead claude`

Juliet will scan your project for existing configurations in the `.juliet` folder for rehydration, but this is a new project so it will simply ask:

`hi, i'm juliet. what do you want to work on today?`

##  Working with Juliet

At this point you can create a new project by talking to Juliet, providing a PRD, etc.

<img width="886" height="198" alt="Screenshot 2026-02-06 at 9 58 48 AM" src="https://github.com/user-attachments/assets/eddc04f0-0da5-48ca-9a6b-24ac78be8fae" />

Juliet will create the project, saving the necessary information to its memory so that it can rehydrate from a cold reboot later (you can also switch between codex and claude code easily)

<img width="879" height="351" alt="Screenshot 2026-02-06 at 9 59 11 AM" src="https://github.com/user-attachments/assets/d949fc78-4a21-49ee-b7c6-05cb60e13ee7" />
<img width="885" height="326" alt="Screenshot 2026-02-06 at 9 59 53 AM" src="https://github.com/user-attachments/assets/635dd05a-2718-46a7-b90c-c0d935952752" />

Juliet will have you review the plan, and then ask how many attempts you'd like to make. These will be stored in separate worktrees. It will also ask if you'd like to run them with claude, codex, or a mix.

<img width="892" height="307" alt="Screenshot 2026-02-06 at 10 01 03 AM" src="https://github.com/user-attachments/assets/e926758d-3967-4bb7-bafb-e0572e78a1e8" />

Now you wait. Sprints can take a long time, up to an hour if the tasks are heavy. So you can periodically just ask for status. Juliet's state contains what it needs from *you*, so you could build a tool to scan for those if you need more active visibility.

<img width="880" height="314" alt="Screenshot 2026-02-06 at 10 02 14 AM" src="https://github.com/user-attachments/assets/a4927cbf-f009-4796-9420-5bb2e922b6a9" />

By default Juliet will run one sprint at a time, then ask for your feedback. You can tell it not to do this, to just run them all. Or you can review the first one, then tell it to run two sprints before asking for your feedback.

You can also tell Juliet to code-review and compare the branches to pick winners, ask it to inject tasks based on what it's seeing, or kill worktrees that are going off the rails. This keeps you as human-in-the-loop but manages the actual interaction with the code for you. You can of course dive into the worktree, make any changes you like, then continue. Because Juliet stops to ask for feedback you control the cadence of this.

<img width="1726" height="966" alt="image" src="https://github.com/user-attachments/assets/1b13bce6-9475-462e-8be1-8ce734691d4a" />

Once you're done, just ask Juliet to merge for you, and ask her to clean up the worktrees. 

## Installation

Juliet is built to be unobstructive, thus needs to be run without confirmations (i.e., it is inherently dangerous to run outside of a sandbox). You should run it in a docker container, or a cloud machine. I use Digital Ocean droplets myself.

```
brew tap seezatnap/seezatnap
brew install swarm
brew install juliet
```

`swarm` is installed automatically as a dependency of `juliet`, so `brew install juliet` is all you need. The tap step is only required the first time.

## Commands

The CLI is minimalistic and supports interactive and non-interactive modes (so you can use it directly in chat, or programatically via a heartbeat)

```
Usage: juliet <command> [options]

Commands:
  Initialize a new role:
    juliet init --role <name>

  Launch a specific role:
    juliet --role <name> <claude|codex>

  Launch (auto-selects role when only one exists):
    juliet <claude|codex>

  Reset a role's prompt to default:
    juliet reset-prompt --role <name>

  Clear a role's history:
    juliet clear-history --role <name>

  Execute a single non-interactive turn:
    juliet exec --role <name> <claude|codex> <message...>
    juliet exec <claude|codex> <message...>
```
