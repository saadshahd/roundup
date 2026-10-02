You are a Builder (`.agents/builder.md`). Build the second slice of the daemon Messages: Takeover, the Held reasons, delivery order and Messages to a receiver that is gone, for scenarios B6, B7, B9 and B10 in `scenarios/messages.md`. It starts only after slice 1 (`b-store`: B1, B3, B4, B5, B11, the storage half of B8) is on `main`; read its code first. Read `AGENTS.md`, `CONTEXT.md`, `PRINCIPLES.md`, `scenarios/messages.md`, `docs/messages.md` and `.claude/sound/`. The scenarios are the spec; where a clause is unclear, stop and ask the Architect.

Scope: `crates/messages` and the contract additions it implements in `crates/contracts` (regenerate `contracts/generated/`, never edit it by hand): the methods `takeover.begin` and `takeover.end` (the user's calls, B6) and the event `takeover.changed`. A contract change (`AGENTS.md` rule 4): its PR needs an architect's approval.

Build, in order, writing each failing test first:
1. Takeovers, held in memory (B6): `begin` and `end` as the text says, including `FORBIDDEN`, `NOT_FOUND`, `CONFLICT` for an ended Agent, a repeated call changing nothing, and `takeover.changed`.
2. Held reasons (B6): during a Takeover a Message from any Actor but the user is `held` with the reason `takeover` on an `auto` Route, keeps `ask-first` on that Route, and is `dropped` on a `drop` Route; at the end the `takeover` ones become `pending` in id order; `message.deliver` on one releases it at once.
3. Order (B7): pending Messages go in the order in which they became deliverable, ties by id; a held Message never blocks a later one.
4. Receiver gone (B9) and the user as receiver (B10, Rust-observable clauses only): a Kind of `done` or `error` drops the `pending` Messages with `receiver gone`, in id order, with one `message.dropped` each; Messages held for `takeover` first become `pending` and are dropped with the rest; `ask-first` ones stay held; a later `message.send` is `CONFLICT`, and `message.deliver` on a held Message to such an Agent is `CONFLICT` and leaves it held. A Message to the user is `delivered` at once with no `agent.status` event.

Typing a Message into a Terminal is slice 3, not this one. Give `crates/messages` a `Deliver` function value it is built with (a function of the Agent id and the text), so that tests pass a fake that records calls and slice 3 passes `Agents::prompt`. No other new seam, trait, field or method.

You may edit only the files above. Name tests after their scenario: `b6_...`, `b7_...`, `b9_...`, `b10_...`. Every existing test stays green; run `just check`. Every commit carries `Author-Agent: <your id>`; the PR body says "Serves PRINCIPLES.md P1, P3, P4" and answers each of their gates.

Observer: `just check` green and the `b` tests above pass. Report: what changed, any clause that could not be tested as written, and the PR number.
