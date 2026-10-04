You are a Builder (`.agents/builder.md`). Build the first slice of the daemon Messages: the store, the Routes and the user's calls, for scenarios B1, B3, B4, B5, B11 and the storage half of B8 in `scenarios/messages.md`. Read `AGENTS.md`, `GLOSSARY.md`, `PRINCIPLES.md`, `scenarios/messages.md`, `docs/messages.md` and `.claude/sound/` first. The scenarios are the spec; where a clause is unclear, stop and ask the Architect, do not guess.

Scope, and nothing else: a new crate `crates/messages`, registered with the Daemon in `crates/rupd/src/lib.rs`, with dependency wiring in `crates/rupd/Cargo.toml` and the resulting `Cargo.lock` updates; the contract types and methods it needs in `crates/contracts` (regenerate `contracts/generated/`, never edit it by hand); `crates/provenance` only if a Touch cannot be logged through its existing `item` string. Contract change (`AGENTS.md` rule 4): the PR needs an architect's approval, so keep it to the methods below.

Build, in order, writing each failing test first:
1. Types and storage: `Message` with `id`, `from`, `to`, `kind` (`note` or `question`), `body`, `replyTo`, `status` (`pending`, `held`, `delivered`, `dropped`), `reason`, `passedFrom` is NOT in this slice, and `at`; `Route`. Tables in `.roundup/roundup.db` (ADR 0004, WAL), next id continuing after a restart.
2. `message.send` (B1: every rejection in the text, and a rejected call changes nothing), `message.get`, `message.list`. `from` is the calling Actor, never a parameter.
3. Routes (B5): default `auto`. `route.set` is user only (`FORBIDDEN` otherwise) and emits `route.changed`. `route.list` returns every stored Route; the user-only restriction applies only to `route.set`.
4. A Message on an `ask-first` Route is `held` with reason `ask-first`; `message.deliver` and `message.drop` are the user's calls (B3); a Route of `drop` makes it `dropped` (B4). The `auto` Route leaves the Message `pending`: nothing types it in this slice.
5. Touches (B11): `wrote` on send, deliver and drop, `read` on `message.get`, reading by Actor.
6. Restart (B8, storage half): Routes, Messages and statuses survive `Messages::open` on the same directory; the id sequence continues.

Not in this slice: typing into a Terminal, Takeover, Held reasons other than `ask-first`, escalation, the MCP tools, digests. Do not add a trait, field or method that a later slice would use; each slice ships only what it calls.

You may edit only the files above. Name tests after their scenario: `b1_...`, `b3_...`, `b4_...`, `b5_...`, `b8_...`, `b11_...`. Every existing test stays green; run `just check`. If #180 (A16, `rail.remove`) is not on `main` yet, expect a one-line conflict in `crates/contracts/src/methods.rs` and merge `origin/main` into your branch. Every commit carries `Author-Agent: <your id>`, and the PR body says "Serves PRINCIPLES.md P1, P3, P4" and answers each of their gates.

Observer: `just check` green and the `b` tests above pass. Report: the methods and types added, anything in the scenarios that could not be tested as written, and the PR number.

Pure step (V4, `scenarios/proofs.md`): put every transition you build in the one pure, total function `step(&State, Event) -> (State, Vec<Effect>)` with no I/O, clock or random source, and name each transition as `proofs/messages/model.bend` does. Add an `inv_` replay test for each law of `proofs/messages/LAWS.bend` your slice touches. Never edit `proofs/messages/LAWS.bend`, `model.bend` or `spec.bend`; if the Rust needs a transition the model lacks, stop and report it.
