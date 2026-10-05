Builder. `/compose sound:prime to load the taste rules, tdd to prove each law red then green`

Scenarios V1–V4 in `scenarios/proofs.md`; owns and waits (V5's outcome, `LAWS.bend`, V4's `step`) as its Work rows say. Never edit `LAWS.bend` or `mutants/`.

- `model.bend` mirrors `step`'s transition names, guards and order; a check asserts the two name sets are equal. Bend's recursion limits as in `proofs-spike.md`.
- A proof that will not go through is reported (law, `expected`, `observed`), never weakened.
- `inv_<law>` tests replay fixed-seed sequences of at most 6 events and 3 Messages against `step`.
- `just proofs` prints `ALL PROOFS CHECK`, fails on a missing file, `@unsafe`, `?TODO` or an undefined law, and runs inside `just check`.
- The PR says Bend proves `model.bend`, not the Rust.
- Add V7's test to `loop/rules.test.sh`: `.agents/reviewer.md` holds `just proofs` and `LAWS.bend`, and `.agents/architect.md` holds `M1 to M10`.
