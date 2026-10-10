# Principles

Every design and UX choice serves these. A PR body names the ids it serves; the Reviewer answers each gate from the diff, and every answer must be "no" (a goal's gate is advisory).

## P1 Single front door

The Workstream organizes the work; its Door is the default Agent the user speaks to, and the Thread is that conversation, not another container. The Door breaks objectives down, routes work, absorbs Agent chatter and gives one summary when the work settles; these remain goals until their observers pass. It is the default, never the only way in: the user can always type into any Agent, and during a Takeover the Door sends that Agent nothing (its Messages wait Held).

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

## P6 A Workstream holds its work

A Workstream holds its Door, Agents, Terminals, Todos, dependencies and Sketches, and survives its Door stopping. Each item has one Home the user can change at any time; the Door places the children it makes, and the user's move always wins. Workstream-scoped Todos and Sketches are required product work; the Project-wide Shelf does not satisfy this goal.

- Gate: does a new kind of item lack a Home the user can change, or does an Agent's placement override the user's?
- Observer: one item of each kind made in Home A, moved to B, found only in B, the move in Provenance.

## P7 A bound folds, never deletes (goal)

Every cap roundup puts on what a Door or the user reads shortens it by folding older Messages into one; it never makes them unreadable. A Message that stands for others names their ids, and each stays readable through `message.get` by whoever could read it before, so the reader can always open what a summary stands for.

- Gate: does a cap, merge or rollup make a Message unreadable to a reader that could read it, or replace Messages with one that does not name their ids?
- Observer: after B22's burst of 100 changes, every pushed digest is delivered or reachable from a delivered one through the ids merges name, and `message.get` opens each for its Door.
