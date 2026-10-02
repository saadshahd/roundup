# Principles

What roundup is for, beyond a lean, extensible, fast workbench. Every design and UX choice serves these. Terms are in `CONTEXT.md`; **Thread** and **Card** are named here and defined there after the naming pass (`/shape`).

A PR body lists the principle ids it serves and answers each gate below. A gate is a yes/no question. The answer must be "no" to merge, unless the principle says it is a goal. The Reviewer checks the answers against the diff (`AGENTS.md` rule 1).

## P1 Single front door

The user speaks to one Thread, not to several Agents. The Thread breaks down objectives, routes work, absorbs Agent chatter, and gives one summary when its work settles.

The door is the default, never the only way in. The user can always open any Agent and type into its terminal. During a Takeover of an Agent, the Thread sends it nothing: its Messages wait as Held and deliver when the Takeover ends.

Gate: does a new path make the user address an Agent, not the Thread, to start work, route it, or read its result? Does a new path make an Agent reachable only through the Thread, or let the Thread send to an Agent under Takeover?
Observer: a scenario that starts, routes and finishes work through the Thread alone; a scenario where the user takes over an Agent and its Messages stay Held until the Takeover ends.

## P2 Worktree isolation (goal)

Concurrent Agents never share a working directory. Each gets its own git worktree, commits there, and lands only when done. Today the Project default is off (`docs/wireframes.md`, decision 12); this principle is the target, not yet a rule.

Gate: does a new path let two Agents with Kind `working` share one cwd?
Observer: a test that spawns two Agents and compares their cwds.

## P3 Cards over chatter

Agents run in the background until they cannot continue. The only interruption is a Card: one blocking question in the Inbox. Answering clears it and the Agent resumes. Every Agent with Kind `needs-you` has exactly one Card, and a Card exists only for that Kind.

A Card is valid when all of these hold:

1. its Agent's Kind is `needs-you`;
2. it holds one question and the answers that unblock the Agent;
3. no other Card from the same Agent is open (a repeat updates it).

Gate: does anything interrupt the user except a valid Card (a bell, an OS alert, ink on a routine Kind), or does an Observation of routine progress yield `needs-you`?
Observer: replay `spikes/hooks-state/*.jsonl`; routine transitions produce no Card, each `needs-you` produces one valid Card.

## P4 Black-box harness

A vendor tool is an opaque PTY process. roundup never asks for an API key when the user has a subscription. An Adapter has three seams: spawn in a worktree and PTY; fold Observations into a Kind; inject input and reap the exit code.

Gate: does code outside `crates/agents/<vendor>/` read a vendor string, vendor environment variable or vendor API, or does `AgentAdapter` gain a fourth seam?
Observer: a `loop/rules.sh` check beside `vocab`; the trait diff.

## P5 Explicit landing

An Agent never pushes the shared branch. Its changes rebase, pass `check`, and fast-forward into trunk or a PR. A dropped or rejected Agent leaves no worktree and no branch.

Gate: does a path write `main` other than rebase, `check` green, then fast-forward? Does a rejected Agent leave an entry in `git worktree list` or `git branch`?
Observer: a test comparing both lists before spawn and after reject.
