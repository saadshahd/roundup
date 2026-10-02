#!/usr/bin/env bash
# Tests for loop/rules.sh, each in a throwaway git repo. Usage: loop/rules.test.sh
set -euo pipefail

script="$(cd "$(dirname "$0")" && pwd)/rules.sh"
failures=0

new_repo() {
  dir=$(mktemp -d)
  cd "$dir"
  git init -q -b main
  git config user.email t@t
  git config user.name t
  mkdir loop crates contracts
  cp "$script" loop/rules.sh
  printf '_Avoid:_ session, bot, process.\n_Avoid:_ parent agent.\n' >CONTEXT.md
  echo 'pub fn ok() {}' >crates/a.rs
  git add -A
  git commit -qm base
  git checkout -q -b work
}

commit() { git add -A && git commit -qm "$1" -m "${2:-}"; }

expect() {
  local want=$1 name=$2
  shift 2
  if "$@" >/dev/null 2>&1; then got=pass; else got=fail; fi
  if [ "$got" != "$want" ]; then echo "FAIL: $name (wanted $want, got $got)"; failures=$((failures + 1)); else echo "ok:   $name"; fi
}

expect_stderr() {
  local want=$1 name=$2 err
  shift 2
  err=$("$@" 2>&1 >/dev/null || true)
  if { [ "$want" = advisory ] && [[ $err == *"size guide"* ]]; } || { [ "$want" = quiet ] && [ -z "$err" ]; }; then echo "ok:   $name"; else echo "FAIL: $name (stderr: $err)"; failures=$((failures + 1)); fi
}

rules() { loop/rules.sh "$1" main; }

# size
new_repo; seq 1 2001 >crates/big.txt; commit x
expect pass "L1 size: above the guide still passes" rules size
expect_stderr advisory "L1 size: above the guide prints an advisory" rules size
new_repo; seq 1 2000 >crates/big.txt; commit x
expect pass "L1 size: at the guide passes" rules size
expect_stderr quiet "L1 size: at the guide prints nothing" rules size
new_repo; mkdir -p a/b; seq 1 2100 >a/b/pnpm-lock.yaml; echo x >crates/x.txt; commit x
expect pass "L1 size: nested lockfile is excluded" rules size
expect_stderr quiet "L1 size: nested lockfile is excluded and silent" rules size
new_repo; echo x >crates/x.txt; echo y >contracts/y.txt; commit x
expect pass "L1 size: two module directories" rules size
expect_stderr advisory "L1 size: two module directories prints an advisory" rules size
new_repo; mkdir -p crates/b; printf '\x00\x01' >crates/b/bin.dat; echo x >contracts/y.txt; commit x
expect pass "L1 size: binary in a second module" rules size
expect_stderr advisory "L1 size: binary in a second module prints an advisory" rules size
new_repo; mkdir -p docs; echo hello >docs/x.md; git add -A; git commit -qm docs; git checkout -q main; git merge -q work 2>/dev/null || true; git checkout -q -b work2; git mv docs/x.md crates/x.md; commit mv
expect pass "L1 size: rename across modules counts both" rules size
expect_stderr advisory "L1 size: rename across modules counts both prints an advisory" rules size
new_repo; mkdir -p crates/m/tests; seq 1 1100 >crates/m/tests/a.rs; git add -A; git commit -qm base; git checkout -q main; git merge -q work 2>/dev/null || true; git checkout -q -b work3; mkdir -p crates/m/tests/scenarios; git mv crates/m/tests/a.rs crates/m/tests/scenarios/a.rs; seq 1 40 >crates/m/new.rs; commit mv
expect pass "L1 size: a move inside one module counts only its new lines" rules size
expect_stderr quiet "L1 size: a move inside one module is silent" rules size
new_repo
expect fail "L1 size: unknown base ref fails loudly" loop/rules.sh size no-such-ref

# trailers
new_repo; echo x >crates/x.txt; commit work 'Author-Agent: builder-1'
expect fail "L2 trailers: no reviewer" rules trailers
git commit -q --allow-empty -m approve -m 'Reviewed-by-Agent: reviewer-1'
expect pass "L2 trailers: approval commit with only a reviewer trailer" rules trailers
new_repo; echo x >crates/x.txt; commit work 'Author-Agent: builder-1'
echo y >crates/y.txt; commit second 'Author-Agent: reviewer-1'
git commit -q --allow-empty -m approve -m 'Reviewed-by-Agent: reviewer-1'
expect fail "L2 trailers: reviewer is also an author" rules trailers
new_repo; echo x >crates/x.txt; commit self 'Reviewed-by-Agent: self-1'
expect fail "L2 trailers: a code commit cannot approve itself" rules trailers
new_repo; echo x >crates/x.txt; commit work 'Author-Agent: builder-1'
git commit -q --allow-empty -m approve -m 'Reviewed-by-Agent: reviewer-1'
echo y >crates/y.txt; commit late 'Author-Agent: builder-1'
expect fail "L2 trailers: a commit after the approval needs a new approval" rules trailers
new_repo; echo x >crates/x.txt; commit work 'Author-Agent: a-1
Author-Agent: a-2'
git commit -q --allow-empty -m approve -m 'Reviewed-by-Agent: r-1'
expect pass "L2 trailers: two authors on one commit" rules trailers
new_repo; echo x >crates/x.txt; commit work
git commit -q --allow-empty -m approve -m 'Reviewed-by-Agent: r-1'
expect fail "L2 trailers: commit with no trailers" rules trailers

# vocab
new_repo
expect pass "L3 vocab: clean" rules vocab
for decl in 'pub fn session_id() {}' 'pub const fn session_id() {}' 'pub struct Sessions;' 'pub struct S {\n    pub session: u8,\n}' 'pub(crate) fn bot() {}' 'pub fn parent_agent() {}' 'pub fn list_processes() {}'; do
  new_repo; printf '%b\n' "$decl" >crates/b.rs; commit x
  expect fail "L3 vocab: $decl" rules vocab
done
new_repo; mkdir -p crates/agents/claude_code; echo 'pub fn session_id() {}' >crates/agents/claude_code/a.rs; commit x
expect pass "L3 vocab: claude_code adapter is exempt" rules vocab
new_repo; mkdir -p crates/other/claude_code; echo 'pub fn session_id() {}' >crates/other/claude_code/a.rs; commit x
expect fail "L3 vocab: another claude_code dir is not exempt" rules vocab
new_repo; echo 'export type SessionId = string;' >contracts/a.ts; commit x
expect fail "L3 vocab: TS export" rules vocab
new_repo; rm CONTEXT.md; echo 'pub fn session_id() {}' >crates/b.rs; commit x
expect fail "L3 vocab: missing CONTEXT.md fails loudly" rules vocab

# delta
expect_exit() {
  local want=$1 name=$2 got=0
  shift 2
  "$@" >/dev/null 2>&1 || got=$?
  if [ "$got" != "$want" ]; then echo "FAIL: $name (wanted exit $want, got $got)"; failures=$((failures + 1)); else echo "ok:   $name"; fi
}

expect_output() {
  local want=$1 name=$2 got
  shift 2
  got=$("$@" 2>/dev/null || true)
  if [ "$got" != "$want" ]; then echo "FAIL: $name (got: $got)"; failures=$((failures + 1)); else echo "ok:   $name"; fi
}

# A checks file with every id D1 to D10 passing, except the ids given as Dn=status.
write_checks() {
  local file=$1
  shift
  mkdir -p "$(dirname "$file")"
  jq -n '[range(1;11) | "D\(.)"] as $ids
    | ($ARGS.positional | map(split("=") | {(.[0]): .[1]}) | add // {}) as $set
    | $ids | map({(.): {status: ($set[.] // "pass"), value: 1, selector: "x"}}) | add' --args "$@" >"$file"
}

delta() { "$script" delta "$@"; }
new_dirs() { dir=$(mktemp -d); cd "$dir"; mkdir base head; }

new_dirs; write_checks base/s.checks.json; write_checks head/s.checks.json
expect_exit 0 "L42 delta: nothing changed" delta base head
expect_output "$(for n in 1 2 3 4 5 6 7 8 9 10; do echo "s D$n still-passing"; done)" "L42 delta: one line per check, ordered by id" delta base head
new_dirs; write_checks base/s.checks.json D3=fail; write_checks head/s.checks.json
expect_output "s D3 fixed" "L42 delta: fail then pass is fixed" bash -c "'$script' delta base head | grep D3"
expect_exit 0 "L42 delta: a fixed check passes" delta base head
new_dirs; write_checks base/s.checks.json; write_checks head/s.checks.json D6=fail
expect_output "s D6 regressed" "L42 delta: pass then fail is regressed" bash -c "'$script' delta base head | grep D6"
expect_exit 1 "L42 delta: a regressed check exits 1" delta base head
new_dirs; write_checks base/s.checks.json D2=fail; write_checks head/s.checks.json D2=fail
expect_output "s D2 still-failing" "L42 delta: fail then fail is still-failing" bash -c "'$script' delta base head | grep D2"
expect_exit 0 "L42 delta: a still-failing check passes" delta base head
new_dirs; write_checks base/L1/a.checks.json; write_checks head/L1/a.checks.json; write_checks base/L1/b.checks.json; write_checks head/L1/b.checks.json
expect_output "L1/a L1/b" "L42 delta: steps are paths without the suffix, ordered" bash -c "'$script' delta base head | awk '{print \$1}' | uniq | tr '\n' ' ' | sed 's/ \$//'"
new_dirs; write_checks base/s.checks.json; write_checks head/s.checks.json; write_checks base/only.checks.json
expect_exit 2 "L42 delta: a step on one side only exits 2" delta base head
new_dirs; write_checks base/s.checks.json; write_checks head/s.checks.json; jq 'del(.D4)' head/s.checks.json >head/t && mv head/t head/s.checks.json
expect_exit 2 "L42 delta: a missing check id exits 2" delta base head
new_dirs; write_checks base/s.checks.json; write_checks head/s.checks.json D5=maybe
expect_exit 2 "L42 delta: a status that is neither pass nor fail exits 2" delta base head
new_dirs; write_checks base/s.checks.json; echo 'not json' >head/s.checks.json
expect_exit 2 "L42 delta: a file that is not JSON exits 2" delta base head
new_dirs
expect_exit 2 "L42 delta: no checks files exits 2" delta base head
expect_exit 2 "L42 delta: a directory that does not exist exits 2" delta base nope

[ "$failures" -eq 0 ] || { echo "$failures failed"; exit 1; }
