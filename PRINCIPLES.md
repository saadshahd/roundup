# Principles

Every design and UX choice serves these. A PR body names the ids it serves; the Reviewer answers each gate from the diff, and every answer must be "no" (a goal's gate is advisory).

## P1 Single front door

The user speaks to one Thread, which breaks objectives down, routes work, absorbs Agent chatter and gives one summary when the work settles. It is the default, never the only way in: the user can always type into any Agent, and during a Takeover the Thread sends that Agent nothing (its Messages wait Held).

- Gate: does a path make the user address an Agent instead of the Thread to start, route or read work; make an Agent reachable only through the Thread; or let the Thread send to an Agent under Takeover?
- Observer: work started, routed and finished through the Thread alone; a Takeover whose Messages stay Held until it ends.

## P2 Worktree isolation (goal)

Concurrent Agents never share a working directory: each commits in its own worktree and lands only when done. The Project default is still off (`docs/wireframes.md`, decision 12).

- Gate: can two `working` Agents share one cwd?
- Observer: spawn two Agents, compare cwds.

## P3 Cards over chatter

Agents run in the background until they cannot continue; the only interruption is a Decision, and answering it resumes the Agent. A Decision is valid when its Agent is `needs-you`, it holds one question and the answers that unblock it, and it is that Agent's only open one (a repeat updates it). Every `needs-you` Agent has exactly one.

- Gate: does anything but a valid Decision interrupt the user (a bell, an OS alert, Ink on a routine Kind), or does routine progress yield `needs-you`?
- Observer: replay `spikes/hooks-state/*.jsonl`: routine transitions open no Decision, each `needs-you` opens one valid Decision.

## P4 Black-box harness

A vendor tool is an opaque PTY process, run on the user's subscription, never an API key. An Adapter has three seams: spawn in a worktree and PTY; fold Observations into a Kind; inject input and reap the exit.

- Gate: does code outside `crates/agents/<vendor>/` read a vendor string, environment variable or API, or does `AgentAdapter` gain a fourth seam?
- Observer: the trait diff; a `loop/rules.sh` check beside `vocab`.

## P5 Explicit landing

An Agent never pushes the shared branch: its changes rebase, pass `check` and fast-forward. A dropped or rejected Agent leaves no worktree and no branch.

- Gate: does a path write `main` other than rebase, green `check`, fast-forward; or does a rejected Agent leave an entry in `git worktree list` or `git branch`?
- Observer: both lists compared before spawn and after reject.

## P6 A topic holds everything

A Group or Meta-agent is a topic holding every item about it. Each item has one Home the user can change at any time; a Meta-agent's Agent places the children it makes, and the user's move always wins.

- Gate: does a new kind of item lack a Home the user can change, or does an Agent's placement override the user's?
- Observer: one item of each kind made in Home A, moved to B, found only in B, the move in Provenance.
