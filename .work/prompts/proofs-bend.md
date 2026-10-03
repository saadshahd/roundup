You are a Builder (`.agents/builder.md`). Prove the six laws of `proofs/messages/LAWS.bend` for scenarios V2, V3 and V4 in `scenarios/proofs.md`. It starts after the Architect's LAWS.bend PR and the first `crates/messages` PR (the pure `step`, V4) are on main.

Scope: `proofs/messages/model.bend`, `proofs/messages/PROOF.bend`, the `inv_` tests in `crates/messages/tests/**`, the `just proofs` and `just proofs-setup` recipes in `justfile`, and `proofs/BEND_VERSION`. You never edit `LAWS.bend` or `mutants/`.

Build, in order, failing test first:
1. `just proofs-setup` (V1): install the pinned Bend into `proofs/.tools/` with its checksum checked, no `sudo`, `BEND_NO_TELEMETRY=1`, a loud failure naming the tool on a version mismatch. Shell tests with a fake download.
2. `model.bend`: a pure, total mirror of `crates/messages`'s `step`, with the same transition names, guards and order as `docs/messages.md`. Read `bend guide` ("Laws and Proofs", "Recursion and Termination") first. Recursion must shrink its first parameter; no mutual recursion; `match` only on a parameter or a pattern-bound variable.
3. `PROOF.bend`: one `def Laws.<name>` per law, same name. No `@unsafe`, no `?TODO`, no fixed-witness `exs`. If a proof will not go through, do not weaken or change the law: report the law, the checker's `expected` and `observed`, and stop.
4. `inv_<law name>` Rust tests that replay the event sequences each law quantifies over (fixed seed, at most 6 events and 3 Messages) against `step`, and a check that the transition names in `step` and `model.bend` are the same set.
5. `just proofs` runs `bend proofs/messages/PROOF.bend` and the Quint check (when `model.qnt` exists), prints `ALL PROOFS CHECK`, exits 0 only if all pass, and fails on a missing file, `@unsafe`, `?TODO` or a law with no def; `just check` runs it. Each mutant in `mutants/` must make it print `SOME PROOFS FAIL` naming its law.

The PR body says "Serves PRINCIPLES.md P1", pastes the last line of `just proofs`, and says Bend proves `model.bend`, not the Rust. Every commit carries `Author-Agent: <your id>`. Observer: `just check` green with `just proofs` inside it. Report: what changed, any law that would not go through, the PR number.
