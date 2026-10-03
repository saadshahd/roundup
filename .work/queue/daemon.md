# Work queue: daemon

The rows this squad holds (`docs/squads.md`, "Moving the queue"). The protocol, the status rules and the `Ids`/`Holder` table stay in `.work/queue.md`; the rule for a row is the same as there.

## In flight

A Builder holds each row and its PR is in review. Nothing below is dispatched again.

| Item | Ids | Owns | Observer | Claimed by |
|---|---|---|---|---|
| reopen, App half | S5 | `crates/desktop/**` | `s5_` tests pass; `s1_` and `s3_` tests stay green | boxd-agents |
| D4 flake: one deadline and a readable failure | D4 | `crates/rup/tests/**` | the Builder reproduces first; `just check` | boxd-agents |
| hold an early Signal until the Agent is registered | A14 | `crates/agents/**` | `a14_` tests fail before the fix and pass after | boxd-agents |

## Ready now

Scenario text is on main and nothing it needs is unmerged. Every row owns a directory no ready or in-flight row owns, so all start at once. Start the first row you have a free Builder for.

| Id | Item | Owns | Keeps green |
|---|---|---|---|
| audit-todos | coverage audit: T1, T2, T3, T5, T7 | `crates/todos/**` | existing tests |
| audit-pads | coverage audit: P2, P3, P7 | `crates/pads/**` | existing tests |

## Waiting

Scenario text is on main. The row starts when what it waits on has merged. A UI row ends its Item with `(Moves: <D ids>)`, the design-system checks it moves (`docs/design-system.md`, baseline protocol); the Builder's PR body repeats the line and corrects it if the diff moves other checks.

| Id | Item | Owns | Keeps green | Waits on |
|---|---|---|---|---|
| A15 | clean environment for every program the Daemon starts | `crates/terminal/**`, `crates/agents/src/claude_code/**`, `crates/rupd/src/lib.rs`, `crates/rup/tests/**`; no `SpawnParams` field; no vendor name in `crates/terminal` (P4) | the `a` and `x` tests | after #114 lands |
| sweep-rupd-harness | one shared way for tests to start a `rupd` and wait for `daemon.ping` | tests under `crates/rup/tests/**` and `crates/rupd/tests/**` | every existing test | after D4 merges |
