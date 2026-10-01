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

rules() { loop/rules.sh "$1" main; }

# size
new_repo; seq 1 401 >crates/big.txt; commit x
expect fail "L1 size: 401 lines" rules size
new_repo; seq 1 400 >crates/big.txt; commit x
expect pass "L1 size: 400 lines" rules size
new_repo; mkdir -p a/b; seq 1 900 >a/b/pnpm-lock.yaml; echo x >crates/x.txt; commit x
expect pass "L1 size: nested lockfile is excluded" rules size
new_repo; echo x >crates/x.txt; echo y >contracts/y.txt; commit x
expect fail "L1 size: two module directories" rules size
new_repo; mkdir -p crates/b; printf '\x00\x01' >crates/b/bin.dat; echo x >contracts/y.txt; commit x
expect fail "L1 size: binary in a second module" rules size
new_repo; mkdir -p docs; echo hello >docs/x.md; git add -A; git commit -qm docs; git checkout -q main; git merge -q work 2>/dev/null || true; git checkout -q -b work2; git mv docs/x.md crates/x.md; commit mv
expect fail "L1 size: rename across modules counts both" rules size
new_repo; mkdir -p crates/m/tests; seq 1 300 >crates/m/tests/a.rs; git add -A; git commit -qm base; git checkout -q main; git merge -q work 2>/dev/null || true; git checkout -q -b work3; mkdir -p crates/m/tests/scenarios; git mv crates/m/tests/a.rs crates/m/tests/scenarios/a.rs; seq 1 40 >crates/m/new.rs; commit mv
expect pass "L1 size: a move inside one module counts only its new lines" rules size
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

[ "$failures" -eq 0 ] || { echo "$failures failed"; exit 1; }
