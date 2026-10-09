Builder. `tdd to prove each law red then green`

Spike V5 in `scenarios/proofs.md`; owns as its Work row says. Timebox: one day; a fail is a result.

- Bend 2.0.34 only in `/tmp` (`BEND_HOME=/tmp/bend-spike`, `BEND_NO_TELEMETRY=1`, no `sudo`); never edit `LAWS.bend`, `model.bend`, `spec.bend` or `crates/**`.
- Order: M1, M2, M3, M10, then the rest. M10 takes a `Nat` fuel, fairness stated in the law.
- Recursion shrinks its first parameter; no mutual recursion; `match` only on a parameter or pattern-bound variable; no `@unsafe` or `?TODO`. One mutant per proved law must print `SOME PROOFS FAIL` naming it.
- Report per law: PASS (with the mutant failing and `just proofs` wall time) or FAIL with Bend's `expected`, `observed`, where it stalled, and what would unblock it.
