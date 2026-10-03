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

# L54 carry: a temporary repo with `origin/main`, a PR branch holding an approval commit, and a fake `gh` that
# prints the PR's head. Exit codes: 0 carried, 1 not carried (named), 4 gh failed.
carry_repo() {
  new_repo
  git checkout -q main
  printf '| id | item |\n|---|---|\n| L1 | one |\n| L2 | two |\n' >q.md
  mkdir -p .work scenarios && mv q.md .work/queue.md && echo a >scenarios/loop.md
  git add -A && git commit -qm "queue" -m "Author-Agent: t"
  git update-ref refs/remotes/origin/main HEAD
  git checkout -q -b pr
  echo work >work.txt && git add -A && git commit -qm "work" -m "Author-Agent: t"
  git commit -q --allow-empty -m "review: approve" -m "Reviewed-by-Agent: r"
  approval=$(git rev-parse HEAD)
  mkdir -p fakebin
  printf '#!/bin/sh\n[ -z "${GH_FAIL:-}" ] || exit 1\ncat "$PWD/gh-head"\n' >fakebin/gh && chmod +x fakebin/gh
  export PATH="$PWD/fakebin:$PATH"
}
# main_commit <file> <content>: advance origin/main on its own branch, then come back to pr.
main_commit() {
  git checkout -q main && printf '%s' "$2" >"$1" && git add -A && git commit -qm "main change" -m "Author-Agent: t" && git update-ref refs/remotes/origin/main HEAD && git checkout -q pr
}
# merge_main: merge origin/main into pr; the caller resolves a conflict by writing the file, then calls finish_merge.
merge_main() { git merge -q --no-commit --no-ff origin/main >/dev/null 2>&1 || true; }
finish_merge() { git add -A && git commit -qm "Merge main" -m "${1-Author-Agent: t}"; }
carry() { git rev-parse HEAD >gh-head; loop/rules.sh carry 7; }
carry_code() { local want=$1 name=$2 why=${3:-} got=0 out; out=$(carry 2>&1) || got=$?; if [ "$got" -eq "$want" ] && [[ $out == *"$why"* ]]; then echo "ok:   $name"; else echo "FAIL: $name (wanted exit $want, got $got: $out)"; failures=$((failures + 1)); fi; }
queue_with() { printf '| id | item |\n|---|---|\n%s' "$1"; }

carry_repo; main_commit .work/queue.md "$(queue_with '| L1 | one |
| L2 | two |
| L3 | three |
')"; echo "| L9 | mine |" >>.work/queue.md; git add -A; git commit -qm "my row" -m "Author-Agent: t"; git commit -q --allow-empty -m "review: approve" -m "Reviewed-by-Agent: r"; approval=$(git rev-parse HEAD)
merge_main; queue_with '| L1 | one |
| L2 | two |
| L9 | mine |
| L3 | three |
' >.work/queue.md; finish_merge
carry_code 0 "L54 carry: a merge that conflicts only in queue rows and keeps both carries"
out=$(carry 2>&1 || true); [[ $out == "carried $approval "* ]] && echo "ok:   L54 carry prints carried <A> <H>" || { echo "FAIL: L54 carry prints carried <A> <H> ($out)"; failures=$((failures + 1)); }

carry_repo; echo "| L9 | mine |" >>.work/queue.md; git add -A; git commit -qm "my row" -m "Author-Agent: t"; git commit -q --allow-empty -m "review: approve" -m "Reviewed-by-Agent: r"
main_commit .work/queue.md "$(queue_with '| L1 | one |
| L2 | two |
| L3 | three |
')"; merge_main; queue_with '| L1 | one |
| L2 | two |
| L9 | mine |
' >.work/queue.md; finish_merge
carry_code 1 "L54 carry: a merge that drops a row main added exits 1 (rule b)" "rule b"

carry_repo; main_commit .work/queue.md "$(queue_with '| L1 | one |
')"; echo "| L9 | mine |" >>.work/queue.md; git add -A; git commit -qm "my row" -m "Author-Agent: t"; git commit -q --allow-empty -m "review: approve" -m "Reviewed-by-Agent: r"
merge_main; queue_with '| L1 | one |
| L2 | two |
| L9 | mine |
' >.work/queue.md; finish_merge
carry_code 1 "L54 carry: a merge that keeps a row main removed exits 1 (rule b)" "rule b"

carry_repo; main_commit .work/queue.md "$(queue_with '| L1 | one |
| L2 | two |
| L3 | three |
')"; echo "| L9 | mine |" >>.work/queue.md; git add -A; git commit -qm "my row" -m "Author-Agent: t"; git commit -q --allow-empty -m "review: approve" -m "Reviewed-by-Agent: r"
merge_main; queue_with '| L1 | one |
| L2 | two |
| L9 | mine |
| L3 | three |
| L7 | invented |
' >.work/queue.md; finish_merge
carry_code 1 "L54 carry: a merge that invents a row exits 1 (rule b)" "rule b"

carry_repo; main_commit .work/queue.md "$(queue_with '| L1 | one |
| L2 | two |
| L3 | three |
')"; printf '| L3 | three |\n' >>.work/queue.md; git add -A; git commit -qm "same row" -m "Author-Agent: t"; git commit -q --allow-empty -m "review: approve" -m "Reviewed-by-Agent: r"
merge_main; queue_with '| L1 | one |
| L2 | two |
| L3 | three |
' >.work/queue.md; finish_merge
carry_code 0 "L54 carry: a row both sides added appears once and carries"

carry_repo; git checkout -q -b offmain main; echo x >off.txt; git add -A; git commit -qm off -m "Author-Agent: t"; git checkout -q pr
git merge -q --no-ff offmain -m "Merge offmain" -m "Author-Agent: t" >/dev/null
carry_code 1 "L54 carry: a merge whose second parent is off main exits 1" "not a merge of main"

carry_repo; main_commit other.txt x; main_commit other.txt y; git update-ref refs/remotes/origin/main main
git merge -q --no-ff "$(git rev-parse main~1)" -m "Merge main" -m "Author-Agent: t" >/dev/null
carry_code 0 "L54 carry: a merge of main's older ancestor carries"

carry_repo; main_commit scenarios/loop.md changed; echo mine >>scenarios/loop.md; git add -A; git commit -qm "edit loop" -m "Author-Agent: t"; git commit -q --allow-empty -m "review: approve" -m "Reviewed-by-Agent: r"
merge_main; echo resolved >scenarios/loop.md; finish_merge
carry_code 1 "L54 carry: a conflict in scenarios/loop.md exits 1 (rule a)" "rule a"

carry_repo; main_commit other.txt x; merge_main; echo sneaky >>work.txt; finish_merge
carry_code 1 "L54 carry: a merge that also edits another file exits 1 (rule c)" "rule c"

carry_repo; main_commit other.txt x; merge_main; finish_merge "no trailer here"
carry_code 1 "L54 carry: a merge with no Author-Agent exits 1" "Author-Agent"

carry_repo; echo more >more.txt; git add -A; git commit -qm "later work" -m "Author-Agent: t"
carry_code 1 "L54 carry: a non-merge commit after the approval exits 1" "not a merge"

carry_repo; main_commit other.txt x; merge_main; finish_merge; main_commit other2.txt y; merge_main; finish_merge
carry_code 0 "L54 carry: two carried merges in a row carry"

new_repo; git checkout -q main; git update-ref refs/remotes/origin/main HEAD; git checkout -q work; mkdir -p fakebin; printf '#!/bin/sh\ncat "$PWD/gh-head"\n' >fakebin/gh; chmod +x fakebin/gh; export PATH="$PWD/fakebin:$PATH"
carry_code 1 "L54 carry: a head with no approval commit exits 1" "no approval"

carry_repo; git rev-parse HEAD >gh-head; got=0; GH_FAIL=1 loop/rules.sh carry 7 >/dev/null 2>err || got=$?
[ "$got" -eq 4 ] && grep -q gh err && echo "ok:   L54 carry: a gh failure exits 4 naming gh" || { echo "FAIL: L54 carry: a gh failure exits 4 naming gh (got $got)"; failures=$((failures + 1)); }
got=0; loop/rules.sh carry x >/dev/null 2>&1 || got=$?
[ "$got" -eq 2 ] && echo "ok:   L54 carry: a non-numeric <pr> exits 2" || { echo "FAIL: L54 carry: a non-numeric <pr> exits 2 (got $got)"; failures=$((failures + 1)); }

[ "$failures" -eq 0 ] || { echo "$failures failed"; exit 1; }
