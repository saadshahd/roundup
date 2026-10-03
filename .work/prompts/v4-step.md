# V4: the pure `step` of `crates/messages`

Scenario V4 (`scenarios/proofs.md`). Make `crates/messages/src/tests/inv_tests.rs` pass and never edit it.

1. Add `crates/messages/src/step.rs` and the lines `mod step;` and `mod inv_tests;` (inside `mod tests`) to `lib.rs`. The seam is pinned in the test file's header. `.work/prompts/v4-step-reference.rs` is a reference `step` that passes the `inv_` tests: copy it, then check it against `proofs/messages/model.bend`; a guard or order the reference lacks is yours to fix, and a transition the model lacks is yours to report.
2. Run every transition of `crates/messages` through `step`: `send`, `takeover.begin`, `takeover.end`, an `idle` or `done` or `error` status event, `message.deliver`, `message.drop`. The Daemon builds the `Event`, keeps the `State` per receiver, applies the `Effect::Type(id)` as one `Deliver` call, and stores each status change. Slice 2's behaviour does not change: typing at the moment of a send is slice 3, so the Daemon keeps `idle` false until an `agent.status` event says `idle`.
3. Every `b` test in `crates/messages` and `crates/rupd` still passes.
4. Never edit `proofs/messages/LAWS.bend`, `model.bend` or `spec.bend`.
