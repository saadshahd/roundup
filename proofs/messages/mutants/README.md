# Mutants

`mutants/<law>/model.bend` is `model.bend` with one fault that breaks that law.
`BEND=/tmp/bend-spike/bend/bin/bend proofs/messages/mutants/run.sh` checks `PROOF.bend` against each in a scratch copy.
Every mutant must print `SOME PROOFS FAIL`; the script names the first failing definition, which sits in that law's proof chain.
