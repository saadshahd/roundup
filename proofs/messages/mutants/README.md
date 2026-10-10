# Mutants

`mutants/<law>/model.bend` is `model.bend` with one fault that breaks that law.
`BEND=/tmp/bend-spike/bend/bin/bend proofs/messages/mutants/run.sh` checks `PROOF.bend` against each in a scratch copy.
Every mutant must print `SOME PROOFS FAIL`. Bend stops at the first failing definition, so the script names the earliest definition that reads the mutated line; that can sit in an earlier law's chain (the M4, M6 and M7 mutants fail in `m10_go`, `invw_step` and `keeps_step`).
