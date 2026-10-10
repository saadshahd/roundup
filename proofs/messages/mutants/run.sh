#!/usr/bin/env bash
# Mutation check for proofs/messages/PROOF.bend (V5).
# For each law id (m2 m3 ...): keep PROOF.bend's `lib` section plus that law's section, check it
# against the unmutated model (it must raise no error), then against model.bend with
# mutants/<id>.patch applied (it must print SOME PROOFS FAIL and a Location inside that section).
# Needs Bend 2.0.34: BEND=/path/to/bend, or `bend` on PATH. Run from anywhere.
set -u
here=$(cd "$(dirname "$0")" && pwd)
dir=$(dirname "$here")
BEND=${BEND:-bend}
export BEND_NO_TELEMETRY=1
laws=("$@")
[ ${#laws[@]} -eq 0 ] && laws=($(cd "$here" && ls *.patch | sed 's/\.patch$//'))
status=0

section() { # section <id>: PROOF.bend without the other laws' sections
  awk -v want="$(echo "$1" | tr a-z A-Z)" '
    /^# ---- / { tag = $3 }
    tag == "" || tag == "lib" || tag == want { print }' "$dir/PROOF.bend"
}

check() { # check <id> <yes|no mutated>: sets out and secs
  local id=$1 mutated=$2 w
  w=$(mktemp -d)
  cp "$dir/LAWS.bend" "$dir/spec.bend" "$dir/model.bend" "$w/"
  section "$id" > "$w/PROOF.bend"
  if [ "$mutated" = yes ]; then
    patch --quiet "$w/model.bend" "$here/$id.patch" || { echo "cannot apply $id.patch"; exit 2; }
  fi
  local t0
  t0=$(date +%s%N)
  out=$(cd "$w" && "$BEND" PROOF.bend 2>&1)
  secs=$(( ($(date +%s%N) - t0) / 1000000 ))
  rm -rf "$w"
}

for id in "${laws[@]}"; do
  check "$id" no
  if echo "$out" | grep -q '^Location:'; then
    echo "FAIL $id: the unmutated proof raises an error"; echo "$out" | head -12; status=1; continue
  fi
  check "$id" yes
  loc=$(echo "$out" | sed -n 's/^Location: //p' | head -1)
  if echo "$out" | grep -q '^SOME PROOFS FAIL' && [ -n "$loc" ]; then
    echo "PASS $id: the mutant fails in $loc (${secs} ms)"
  else
    echo "FAIL $id: the mutant is not rejected"; status=1
  fi
done
exit $status
