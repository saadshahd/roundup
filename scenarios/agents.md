# Agents and the Rail

Read `spikes/hooks-state/REPORT.md` and ADR 0006 first: they define how hook Signals, the terminal title and the exit status become a Status.

**A1 signals to status.** Given an adapter replaying `spikes/hooks-state/log*.jsonl` payloads as Signals, then `UserPromptSubmit` gives `working`; `Stop` gives `idle`; `PermissionRequest` gives `needs-you` whose label quotes the question or tool; `StopFailure` gives `error` whose label is the `error` field; `PostToolUse` after a `needs-you` gives `working`; `SessionEnd` gives `done`. The mapping is a pure function `observe(Observation) -> Option<Status>` with `since` set by an injected clock.

**A2 title closes the silent gaps.** Given the title Observations in `spikes/hooks-state/screen.jsonl`, then a ◐/◑ title gives `working`, and an ✳ title after an Esc interrupt or a denied dialog leaves `working` and `needs-you`. Where the fixtures cannot tell the two apart, the scenario's test documents the rule chosen and why.

**A3 exit.** Given a Terminal behind an Agent, when it exits with code 0 the Status is `done`; with a non-zero code or a signal it is `error` with label `exited <code>`. A Signal arriving after exit is ignored.

**A4 spawn.** Given `agent.spawn {cwd, prompt}` with `ROUNDUP_CLAUDE_BIN` pointing at a fake `claude` script (tests never run the real one), then a Terminal starts running it with a per-Agent `--settings` file whose one command hook runs `rup hook <agent-id>`, the working directory is pre-trusted, the Agent appears in `rail.tree` with `kind: agent`, and the prompt is typed into the Terminal at the first idle. `rup hook <agent-id>` reads one hook payload from stdin, identifies as that Agent, and calls `agent.signal`.

**A5 signal path.** Given an Agent, when `agent.signal {id, payload}` arrives, then the adapter updates the Status and, only if it changed, emits `agent.status {id, status}`. A payload the adapter does not recognise is ignored and logged to stderr, never an error to the caller.

**A6 rail tree.** Given Groups and Agents, then `rail.createGroup`, `rail.rename` and `rail.move {id, parent, index}` keep sibling `order` values contiguous from 0, emit `rail.changed`, and fail with `CONFLICT` when a node would move into its own descendant or under a plain Agent or Terminal (nesting only under Groups and Meta-agents). `rail.tree` returns nodes ordered parent-first, then by `order`.

**A7 promote and stop.** Given a Group with two Agent children, when `rail.promote` is called, then it is a Meta-agent (`meta: true`) with a live Agent and a Status. When `agent.stop` is called on it, then it stays in the Rail with status `done`, its children move up one level (parent becomes the Meta-agent's parent, placed where it was) and keep running.

**A8 persistence.** Given a Rail, when `Agents::open` is called again on the same directory, then Groups, `parent` and `order`, names and `meta` flags are intact. Agents whose Terminal is gone come back with Status `done`; the module never claims a dead process is working.

Deferred past Phase 2: `blocked` for Agents (needs Todo ownership), Meta-agents receiving child events (needs the message bus), the Claude-trust-dialog screen scrape.
