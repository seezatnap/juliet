# Juliet

Juliet is a simple bootup script for Claude Code or Codex. By default, it's built to be an expert at using Swarm Hug, another library I wrote which manages swarms of agents in a sprint-like configuration. But you can tweak the prompt to make it an expert at anything else.

## Initializing

You fire up Juliet by creating a new role. This will use the default prompt, which is a team orchestrator / director:

`juliet init --role eng-lead`

Boot Juliet up:

`juliet --role eng-lead claude`

Juliet will scan your project for existing configurations in the `.juliet` folder for rehydration, but this is a new project so it will simply ask:

`hi, i'm juliet. what do you want to work on today?`

##  Creating a project.

At this point you can create a new project by talking to Juliet, providing a PRD, etc.

Juliet will create the project, saving the necessary information to its memory so that it can rehydrate from a cold reboot later (you can also switch between codex and claude code easily)

Juliet will have you review the plan, and then ask how many attempts you'd like to make. These will be stored in separate worktrees. It will also ask if you'd like to run them with claude, codex, or a mix.

Now you wait. Sprints can take a long time, up to an hour if the tasks are heavy. So you can periodically just ask for status. Juliet's state contains what it needs from *you*, so you could build a tool to scan for those if you need more active visibility.

By default Juliet will run one sprint at a time, then ask for your feedback. You can tell it not to do this, to just run them all. Or you can review the first one, then tell it to run two sprints before asking for your feedback.

You can also tell Juliet to code-review and compare the branches to pick winners, ask it to inject tasks based on what it's seeing, or kill worktrees that are going off the rails. This keeps you as human-in-the-loop but manages the actual interaction with the code for you. You can of course dive into the worktree, make any changes you like, then continue. Because Juliet stops to ask for feedback you control the cadence of this.

Once you're done, just ask Juliet to merge for you, and ask her to clean up the worktrees. 

## Installation

Juliet is built to be unobstructive, thus needs to be run without confirmations (i.e., it is inherently dangerous to run outside of a sandbox). You should run it in a docker container, or a cloud machine. I use Digital Ocean droplets myself.

```
brew install swarm // fix command
brew install juliet
```

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
```