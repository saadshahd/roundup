#!/usr/bin/env bash
# Each mutants/<law>/model.bend is model.bend with one fault. Check PROOF.bend against it in a scratch copy;
# bend must print SOME PROOFS FAIL and the first failing definition belongs to that law's proof chain.
set -u
here=$(cd "$(dirname "$0")" && pwd)
bend=${BEND:-bend}
status=0
for d in "$here"/m*/; do
  name=$(basename "$d")
  tmp=$(mktemp -d)
  cp "$here"/../LAWS.bend "$here"/../spec.bend "$here"/../PROOF.bend "$tmp"/
  cp "$d"model.bend "$tmp"/model.bend
  out=$(cd "$tmp" && "$bend" PROOF.bend 2>&1)
  rm -rf "${tmp:?}"
  if grep -q "SOME PROOFS FAIL" <<<"$out"; then
    echo "$name: fails at $(grep -m1 '^Location:' <<<"$out")"
  else
    echo "$name: SURVIVED"
    status=1
  fi
done
exit $status
