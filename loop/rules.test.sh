#!/usr/bin/env bash
# Tests for loop/rules.sh, each in a throwaway git repo. Usage: loop/rules.test.sh
# shellcheck disable=SC2016 # Fixture scripts and Markdown fences must retain literal shell syntax.
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
  printf '_Avoid_: session, bot, process\n_Avoid_: parent agent\n' >GLOSSARY.md
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
new_repo; mkdir -p crates/agents/src/claude_code; echo 'pub fn session_id() {}' >crates/agents/src/claude_code/a.rs; commit x
expect pass "L3 vocab: claude_code adapter is exempt" rules vocab
new_repo; mkdir -p crates/other/claude_code; echo 'pub fn session_id() {}' >crates/other/claude_code/a.rs; commit x
expect fail "L3 vocab: another claude_code dir is not exempt" rules vocab
new_repo; echo 'export type SessionId = string;' >contracts/a.ts; commit x
expect fail "L3 vocab: TS export" rules vocab
new_repo; rm GLOSSARY.md; echo 'pub fn session_id() {}' >crates/b.rs; commit x
expect fail "L3 vocab: missing GLOSSARY.md fails loudly" rules vocab

# delta
expect_exit() {
  local want=$1 name=$2 got=0 out
  shift 2
  out=$("$@" 2>&1) || got=$?
  if [ "$got" != "$want" ]; then echo "FAIL: $name (wanted exit $want, got $got): $out"; failures=$((failures + 1)); else echo "ok:   $name"; fi
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
carry_repo; main_commit other.txt x; merge_main; finish_merge
carry_code 0 "L54 carry: a conflict-free merge of main carries"
out=$(carry 2>&1 || true); if [[ $out == "carried $approval "* ]]; then echo "ok:   L54 carry prints carried <A> <H>"; else echo "FAIL: L54 carry prints carried <A> <H> ($out)"; failures=$((failures + 1)); fi

carry_repo; main_commit .work/queue.md changed; echo mine >>.work/queue.md; git add -A; git commit -qm "edit queue" -m "Author-Agent: t"; git commit -q --allow-empty -m "review: approve" -m "Reviewed-by-Agent: r"
merge_main; echo resolved >.work/queue.md; finish_merge
carry_code 1 "L54 carry: a conflict in any file exits 1 (rule a)" "rule a"

carry_repo; git checkout -q -b offmain main; echo x >off.txt; git add -A; git commit -qm off -m "Author-Agent: t"; git checkout -q pr
git merge -q --no-ff offmain -m "Merge offmain" -m "Author-Agent: t" >/dev/null
carry_code 1 "L54 carry: a merge whose second parent is off main exits 1" "not a merge of main"

carry_repo; main_commit other.txt x; main_commit other.txt y; git update-ref refs/remotes/origin/main main
git merge -q --no-ff "$(git rev-parse main~1)" -m "Merge main" -m "Author-Agent: t" >/dev/null
carry_code 0 "L54 carry: a merge of main's older ancestor carries"

carry_repo; main_commit scenarios/loop.md changed; echo mine >>scenarios/loop.md; git add -A; git commit -qm "edit loop" -m "Author-Agent: t"; git commit -q --allow-empty -m "review: approve" -m "Reviewed-by-Agent: r"
merge_main; echo resolved >scenarios/loop.md; finish_merge
carry_code 1 "L54 carry: a conflict in scenarios/loop.md exits 1 (rule a)" "rule a"

carry_repo; main_commit other.txt x; merge_main; echo sneaky >>scenarios/loop.md; finish_merge
carry_code 1 "L54 carry: a merge that also edits another file exits 1 (rule c)" "rule c"

carry_repo; main_commit other.txt x; merge_main; finish_merge "no trailer here"
carry_code 0 "L54 carry: a clean merge of main needs no Author-Agent"

carry_repo; echo more >more.txt; git add -A; git commit -qm "later work" -m "Author-Agent: t"
carry_code 1 "L54 carry: a non-merge commit after the approval exits 1" "not a merge"

carry_repo; main_commit other.txt x; merge_main; finish_merge; main_commit other2.txt y; merge_main; finish_merge
carry_code 0 "L54 carry: two carried merges in a row carry"

new_repo; git checkout -q main; git update-ref refs/remotes/origin/main HEAD; git checkout -q work; mkdir -p fakebin; printf '#!/bin/sh\ncat "$PWD/gh-head"\n' >fakebin/gh; chmod +x fakebin/gh; export PATH="$PWD/fakebin:$PATH"
carry_code 1 "L54 carry: a head with no approval commit exits 1" "no approval"

carry_repo; git rev-parse HEAD >gh-head; got=0; GH_FAIL=1 loop/rules.sh carry 7 >/dev/null 2>err || got=$?
if [ "$got" -eq 4 ] && grep -q gh err; then echo "ok:   L54 carry: a gh failure exits 4 naming gh"; else echo "FAIL: L54 carry: a gh failure exits 4 naming gh (got $got)"; failures=$((failures + 1)); fi
got=0; loop/rules.sh carry x >/dev/null 2>&1 || got=$?
if [ "$got" -eq 2 ]; then echo "ok:   L54 carry: a non-numeric <pr> exits 2"; else echo "FAIL: L54 carry: a non-numeric <pr> exits 2 (got $got)"; failures=$((failures + 1)); fi

# Further L54 carry cases: fail closed, an empty chain, other shapes of commit.
carry_repo; git checkout -q --orphan unrelated; git rm -rqf . >/dev/null; echo r >root.txt; git add -A; git commit -qm root -m "Author-Agent: t"
git update-ref refs/remotes/origin/main HEAD; git checkout -q pr
git merge -q --no-ff --allow-unrelated-histories --no-edit origin/main -m "Merge main" -m "Author-Agent: t" >/dev/null 2>&1 || true
carry_code 1 "L54 carry: a merge of unrelated history exits 1, merge-tree failing closed" "merge-tree"

carry_repo
carry_code 0 "L54 carry: an empty chain (head is the approval) carries" "carried $approval $approval"


carry_repo; git checkout -q main; printf '| id |\n|---|\n| A1 |\n' >scenarios/README.md; git add -A; git commit -qm readme -m "Author-Agent: t"; git update-ref refs/remotes/origin/main HEAD; git checkout -q pr; git merge -q --no-edit -m "Merge main" -m "Author-Agent: t" origin/main >/dev/null; git commit -q --allow-empty -m "review: approve" -m "Reviewed-by-Agent: r"
echo "| A2 |" >>scenarios/README.md; git add -A; git commit -qm "row" -m "Author-Agent: t"; git commit -q --allow-empty -m "review: approve" -m "Reviewed-by-Agent: r"
main_commit scenarios/README.md "$(printf '| id |\n|---|\n| A1 |\n| A3 |\n')"; merge_main; printf '| id |\n|---|\n| A1 |\n| A2 |\n| A3 |\n' >scenarios/README.md; finish_merge
carry_code 1 "L54 carry: a conflict in scenarios/README.md exits 1 because it has no table" "rule a"

carry_repo; git checkout -q main; printf 'Intro\nShared prose\n' >scenarios/README.md; git add -A; git commit -qm readme -m 'Author-Agent: t'; git update-ref refs/remotes/origin/main HEAD; git checkout -q pr; git merge -q --no-edit -m 'Merge main' -m 'Author-Agent: t' origin/main >/dev/null; git commit -q --allow-empty -m approve -m 'Reviewed-by-Agent: r'
echo 'PR prose' >>scenarios/README.md; git add -A; git commit -qm prose -m 'Author-Agent: t'; git commit -q --allow-empty -m approve -m 'Reviewed-by-Agent: r'
main_commit scenarios/README.md "$(printf 'Intro\nShared prose\nMain prose\n')"; merge_main; printf 'Intro\nShared prose\nPR prose\nMain prose\n' >scenarios/README.md; finish_merge
carry_code 1 "L54 carry: README prose conflict exits 1 (rule a)" "rule a"


carry_repo; main_commit other.txt x; git checkout -q -b side main; echo s >side.txt; git add -A; git commit -qm side -m "Author-Agent: t"; git checkout -q pr; git update-ref refs/remotes/origin/main main
git merge -q --no-ff -m "Octopus" -m "Author-Agent: t" main side >/dev/null 2>&1 || true
carry_code 1 "L54 carry: an octopus merge exits 1" "not a merge of main"

carry_repo; git commit -q --allow-empty -m "review: approve" -m "no trailer"; echo later >later.txt; git add -A; git commit -qm "later" -m "Author-Agent: t"
carry_code 1 "L54 carry: an empty commit without Reviewed-by-Agent is not an approval, the earlier one is" "not a merge of main"

# L33/L44/L45/L46/L53 read only fake GitHub data, with real commit trees.
gate_repo() {
  new_repo
  mkdir -p docs scenarios
  printf 'Three architects: `architect-a`, `architect-b` and `architect-c`.\n' >docs/squads.md
  printf '**L46 a gate.** Given a PR, when checked, then it passes.\n' >scenarios/loop.md
  commit fixtures
  git branch -f main HEAD
  echo changed >crates/change.rs
  commit change 'Author-Agent: builder'
  git commit -q --allow-empty -m approval -m 'Reviewed-by-Agent: reviewer'
  mkdir -p .git/fake-bin
  export GATE_DATA="$PWD/.git/gate.json"
  printf '{}\n' >"$GATE_DATA"
  cat >.git/fake-bin/gh <<'GH'
#!/usr/bin/env python3
import json, os, subprocess, sys, time
from pathlib import Path
args = sys.argv[1:]
d = json.loads(Path(os.environ['GATE_DATA']).read_text())
if d.get('failure'):
    print('gh fixture unavailable', file=sys.stderr)
    sys.exit(1)
if d.get('hang'):
    time.sleep(10)
if args[:2] == ['pr', 'merge']:
    expected = args[args.index('--match-head-commit')+1]
    actual = subprocess.check_output(['git', 'rev-parse', 'work'], text=True).strip()
    sys.exit(0 if expected == actual else 1)
if args[:2] == ['pr', 'view']:
    if d.get('fail_carry'):
        print('gh fixture unavailable during carry', file=sys.stderr)
        sys.exit(1)
    print(d.get('head') or subprocess.check_output(['git', 'rev-parse', 'work'], text=True).strip())
    sys.exit(0)
assert args[0] == 'api' and '--paginate' in args and '--slurp' in args, args
route = args[1]
def git(*args):
    return subprocess.check_output(['git', *args], text=True).strip()
head = d.get('head') or git('rev-parse', 'work')
base = git('rev-parse', 'main')
pr = {'head': {'sha': head}, 'base': {'ref': d.get('base', 'main'), 'sha': base},
      'state': 'open', 'draft': False,
      'body': d.get('body', 'Scenarios: L46\n## Shape\n```\na -> b\n```\n## Proof\nHead: '+head+'\n```\nok L46_gate\n```')}
if '/pulls/' in route and route.endswith('/files?per_page=100'):
    files = []
    for name in git('diff', '--name-only', '--no-renames', base+'...'+head).splitlines():
        files.append({'filename': name, 'patch': 'patch'})
    out = d.get('files', [files])
elif '/pulls/' in route:
    d['reads'] = d.get('reads', 0)+1
    Path(os.environ['GATE_DATA']).write_text(json.dumps(d))
    if d.get('change_head') and d['reads'] > 1:
        pr['head']['sha'] = '0'*40
    out = [pr]
elif '/issues/' in route:
    out = d.get('comments', [[]])
elif '/check-runs' in route:
    out = d.get('checks', [{'check_runs': [{'id': i, 'name': name, 'head_sha': head,
        'status': 'completed', 'conclusion': 'success', 'app': {'slug': 'github-actions'}}
        for i, name in enumerate(['check', 'rules', 'percy'], 1)]}])
else:
    print('unexpected gh route: '+route, file=sys.stderr)
    sys.exit(1)
print(json.dumps(out))
GH
  chmod +x .git/fake-bin/gh
  export PATH="$PWD/.git/fake-bin:$PATH"
}
gate_set() { jq "$@" "$GATE_DATA" >"$GATE_DATA.tmp"; mv "$GATE_DATA.tmp" "$GATE_DATA"; }
gate() { loop/rules.sh "$@"; }

# L37: a GitHub-signed Copilot Autofix commit is authored by `copilot`. A throwaway key stands in for web-flow.
copilot_repo() {
  gate_repo
  git reset -q --hard main
  GNUPGHOME=$(mktemp -d)
  export GNUPGHOME
  gpg --batch -q --passphrase '' --quick-gen-key 'GitHub <noreply@github.com>' ed25519 sign never 2>/dev/null
  gpg --batch -q --passphrase '' --quick-gen-key 'Other <other@example.com>' ed25519 sign never 2>/dev/null
  gpg --armor --export noreply@github.com >loop/web-flow.asc
  commit web-flow
  git branch -f main HEAD
  echo changed >crates/change.rs
  commit change 'Author-Agent: builder'
}
copilot_fix() {
  local key=${1-noreply@github.com} committer=${2:-noreply@github.com} trailer=${3-Co-authored-by: Copilot Autofix powered by AI <62310815+github-advanced-security[bot]@users.noreply.github.com>}
  local sign=--no-gpg-sign
  [ -z "$key" ] || sign=-S
  echo fix >>crates/change.rs
  git add -A
  GIT_COMMITTER_NAME=GitHub GIT_COMMITTER_EMAIL=$committer git -c gpg.format=openpgp -c user.signingkey="$key" commit -q "$sign" \
    -m 'Potential fix for pull request finding' ${trailer:+-m "$trailer"}
  git commit -q --allow-empty -m approval -m "Reviewed-by-Agent: ${4:-reviewer}"
}
copilot_repo; copilot_fix
expect_exit 0 'L37 a GitHub-signed Copilot Autofix commit needs no Author-Agent in CI' gate ci-trailers 12
expect_output "ready $(git rev-parse HEAD)" 'L37 merge-ready accepts a Copilot Autofix commit' gate merge-ready 12
expect_exit 0 'L37 local trailers agree on a review checkout' rules trailers
copilot_repo; copilot_fix ''
expect_exit 1 'L37 an unsigned commit claiming Copilot fails' gate ci-trailers 12
copilot_repo; copilot_fix other@example.com
expect_exit 1 'L37 a Copilot claim signed by another key fails' gate ci-trailers 12
copilot_repo; copilot_fix noreply@github.com noreply@github.com ''
expect_exit 1 'L37 a GitHub-signed commit without the Copilot Autofix trailer fails' gate ci-trailers 12
copilot_repo; copilot_fix noreply@github.com someone@example.com
expect_exit 1 'L37 a Copilot claim GitHub did not commit fails' gate ci-trailers 12
copilot_repo; copilot_fix noreply@github.com noreply@github.com 'Co-authored-by: Copilot Autofix powered by AI <62310815+github-advanced-security[bot]@users.noreply.github.com>' copilot
expect_exit 1 'L37 copilot cannot approve its own fix' gate ci-trailers 12
unset GNUPGHOME

gate_repo
expect_exit 0 'L33 base main passes' gate base 12
gate_set '.base="architect/x"'
expect_exit 1 'L33 stacked base fails' gate base 12
gate_set '.failure=true'
expect_exit 4 'L33 gh failure is not success' gate base 12
expect_exit 2 'L33 missing number fails before gh' gate base
expect_exit 2 'L33 nonnumeric number fails before gh' gate base 12a
gate_set '.failure=false | .hang=true'
expect_exit 4 'L33 gh timeout is bounded' env BOXD_GH_TIMEOUT=1 loop/rules.sh base 12

gate_repo
expect_output block 'L44 source is block' gate class 12
expect_exit 0 'L46 CI trailers accept a block approval' gate ci-trailers 12
expect_exit 0 'L46 empty approval head with exact checks passes' gate merge-ready 12
expect_output "ready $(git rev-parse HEAD)" 'L46 prints the pinned head' gate merge-ready 12
gate_set '.checks=[{"check_runs":[]}]'
expect_exit 1 'L46 missing checks fail' gate merge-ready 12
gate_set 'del(.checks) | .base="stack"'
expect_exit 1 'L46 non-main base fails' gate merge-ready 12
gate_set 'del(.base) | .body="Scenarios: L46"'
expect_exit 1 'L53 missing shape and proof fail' gate proof 12
expect_exit 1 'L46 block requires proof' gate merge-ready 12

for merge_trailer in missing authored self-review approval invalid-id; do
  gate_repo
  git checkout -q main
  printf 'main update\n' >docs/main.md
  commit main-update
  git update-ref refs/remotes/origin/main HEAD
  git checkout -q work
  want=1
  case $merge_trailer in
    missing) trailer=''; want=0 ;;
    authored) trailer='Author-Agent: builder'; want=0 ;;
    self-review) trailer='Author-Agent: reviewer' ;;
    approval) trailer='Reviewed-by-Agent: reviewer' ;;
    invalid-id) trailer='Author-Agent: invalid id' ;;
  esac
  git merge -q --no-ff main -m 'merge main' -m "$trailer"
  git commit -q --allow-empty -m approval -m 'Reviewed-by-Agent: reviewer'
  expect_exit "$want" "L2 local trailers check $merge_trailer merge" rules trailers
  expect_exit "$want" "L2 CI trailers check $merge_trailer merge" gate ci-trailers 12
  if [ "$merge_trailer" = authored ]; then
    gate_set '.failure=true'
    expect_exit 0 'L2 local trailers need no GitHub' rules trailers
  fi
done

gate_repo
git checkout -q main
printf 'main update\n' >docs/main.md
commit main-update
git update-ref refs/remotes/origin/main HEAD
git checkout -q work
git merge -q --no-ff main -m 'merge main' -m 'Author-Agent: builder'
expect_output "ready $(git rev-parse HEAD)" 'L46 L54-carried approval passes' gate merge-ready 12
expect_exit 1 'L2 local approval requires newest commit even when L54 carries remotely' rules trailers

gate_repo
git checkout -q main
printf 'main update\n' >docs/main.md
commit main-update
git checkout -q work
git merge -q --no-ff main -m 'merge main without a trailer'
expect_exit 0 'L46 block allows an untrailed clean merge of main in CI' gate ci-trailers 12
expect_output "ready $(git rev-parse HEAD)" 'L46 block carries an approval across an untrailed clean merge' gate merge-ready 12
git reset -q --hard HEAD^
git merge -q --no-ff --no-commit main
echo sneaky >>crates/change.rs
git add -A
git commit -qm 'merge main and edit'
expect_exit 1 'L46 block still needs Author-Agent on a merge that changes more than main' gate ci-trailers 12

gate_repo
git reset -q --hard HEAD^
expect_exit 1 'L46 block without approval fails' gate merge-ready 12

gate_repo
git reset -q --hard main
printf 'Notes\n' >docs/notes.md
commit docs 'Author-Agent: builder'
expect_output post 'L44 regular docs are post' gate class 12
expect_exit 0 'L46 CI trailers accept authored post prose without approval' gate ci-trailers 12
expect_exit 0 'L46 post needs no approval' gate merge-ready 12
git commit -q --allow-empty -m approval -m 'Reviewed-by-Agent: reviewer'
expect_exit 1 'L46 post rejects an approval-only commit in CI' gate ci-trailers 12
expect_exit 1 'L46 post rejects an approval-only commit at merge' gate merge-ready 12

gate_repo
git reset -q --hard main
printf 'Notes\n' >docs/notes.md
commit docs 'Author-Agent: builder'
git checkout -q main
printf 'Other note\n' >docs/other.md
commit other-note
git checkout -q work
git merge -q --no-ff main -m 'merge main without a trailer'
expect_exit 0 'L46 post allows an untrailed merge in CI' gate ci-trailers 12
expect_exit 0 'L46 post allows an untrailed merge at merge' gate merge-ready 12

gate_repo
expect_exit 0 'L53 nonvisible test proof passes' gate proof 12
gate_set --arg head "$(git rev-parse HEAD)" '.body="Scenarios: L46\n## Shape\n```\na\n```\n## Proof\nHead: "+$head+"\n```\nok L46_gate\n```\nContext mentions U99 without claiming it.\n"'
expect_exit 0 'L53 context ids do not create extra proof obligations' gate proof 12
gate_set '.body="Scenarios: L46\n## Shape\n```\na\n```\n## Proof\nNo output"'
expect_exit 1 'L53 absent test output fails' gate proof 12

gate_repo
printf '**L46 screenshots are proof of a loop rule.** Given a PR, when checked, then it passes.\n' >scenarios/loop.md
commit scenario 'Author-Agent: builder'
git commit -q --allow-empty -m approval -m 'Reviewed-by-Agent: reviewer'
expect_exit 0 'L53 a loop rule mentioning screenshots is not a visible App change' gate proof 12


gate_rejects() {
  local count=$1 agent=${2:-reviewer}
  gate_set ".comments=[[range(0;$count) | {id: ., created_at: \"2026-10-03T00:00:00Z\", body: \"VERDICT: reject\\n$(git rev-parse HEAD)\\nReviewed-by-Agent: $agent\"}]]"
}
gate_pick() {
  local pick=$1 agent=$2
  gate_set ".comments += [[{id: 100, created_at: \"2026-10-03T01:00:00Z\", body: \"ARCHITECT: $pick\\nArchitect: $agent\"}]]"
}
gate_repo
gate_rejects 2
expect_output 2 'L45 two independent rejects print two' gate rounds 12
expect_exit 1 'L46 latest independent reject blocks' gate merge-ready 12
gate_rejects 3
expect_exit 1 'L45 third reject blocks' gate rounds 12
gate_pick split architect-a
expect_output 0 'L45 independent architect split resets count' gate rounds 12
gate_set '.comments += [[range(101;104) | {id: ., created_at: "2026-10-03T02:00:00Z", body: "VERDICT: reject\n'"$(git rev-parse HEAD)"'\nReviewed-by-Agent: reviewer"}]]'
expect_exit 1 'L45 three rejects after split block' gate rounds 12
gate_rejects 3
gate_pick amend stranger
expect_exit 1 'L45 unknown architect cannot reset count' gate rounds 12
gate_pick amend builder
expect_exit 1 'L45 author cannot reset count' gate rounds 12
gate_rejects 1
gate_pick retire architect-a
expect_exit 1 'L45 retire blocks below the cap' gate rounds 12
gate_pick split architect-b
expect_exit 1 'L45 split cannot undo retirement' gate rounds 12
gate_rejects 4 builder
expect_output 0 'L45 author verdicts do not count' gate rounds 12
gate_rejects 4
gate_set '.comments[][] .body |= sub("[0-9a-f]{40}"; "0000000000000000000000000000000000000000")'
expect_exit 1 'L45 independent verdicts on rewritten history still count' gate rounds 12

gate_repo
reviewed=$(git rev-parse HEAD)
earlier=$(git rev-parse HEAD^)
gate_set --arg reviewed "$reviewed" --arg earlier "$earlier" '.comments=[[range(1;4) | {id: .,created_at:"2026-10-03T00:00:00Z",body:("VERDICT: reject\nHead: "+$reviewed+"\nEarlier: "+$earlier+"\nReviewed-by-Agent: reviewer")}]]'
expect_exit 1 'L45 explicit Head counts despite explanatory earlier SHA' gate rounds 12

gate_repo
reviewed=$(git rev-parse HEAD)
earlier=$(git rev-parse HEAD^)
gate_set --arg reviewed "$reviewed" --arg earlier "$earlier" '.comments=[[range(1;4) | {id: .,created_at:"2026-10-03T00:00:00Z",body:("VERDICT: reject\nReviewed "+$reviewed+"; compared with "+$earlier+"\nReviewed-by-Agent: reviewer")}]]'
expect_exit 1 'L45 unlabelled multi-SHA rejects count toward the round cap' gate rounds 12
gate_set --arg reviewed "$reviewed" --arg earlier "$earlier" '.comments=[[{id:1,created_at:"2026-10-03T00:00:00Z",body:("VERDICT: reject\nReviewed "+$reviewed+"; compared with "+$earlier+"\nReviewed-by-Agent: reviewer")}]]'
expect_exit 1 'L46 unlabelled multi-SHA reject naming the current head blocks' gate merge-ready 12

gate_repo
reviewed_content=$(git rev-parse HEAD^)
gate_set --arg sha "$reviewed_content" '.comments=[[{id:1,created_at:"2026-10-03T00:00:00Z",body:("VERDICT: reject\n"+$sha+"\nReviewed-by-Agent: reviewer")}]]'
expect_exit 1 'L46 a reject on the content below the approval blocks' gate merge-ready 12
printf 'another change\n' >>crates/change.rs
commit followup 'Author-Agent: builder'
git commit -q --allow-empty -m approval -m 'Reviewed-by-Agent: reviewer'
gate_set --arg sha "$reviewed_content" '.comments=[[range(0;3) | {id: .,created_at:"2026-10-03T00:00:00Z",body:("VERDICT: reject\n"+$sha+"\nReviewed-by-Agent: reviewer")}]]'
expect_exit 1 'L45 earlier content rejects remain in the round cap' gate rounds 12

gate_repo
old_approval=$(git rev-parse HEAD)
git checkout -q main
printf 'main update\n' >docs/main.md
commit main-update
git update-ref refs/remotes/origin/main HEAD
git checkout -q work
git merge -q --no-ff main -m 'merge main' -m 'Author-Agent: builder'
gate_set --arg sha "$old_approval" '.comments=[[{id:1,created_at:"2026-10-03T00:00:00Z",body:("VERDICT: reject\nReviewed-head: "+$sha+"\nReviewed-by-Agent: reviewer")}]]'
expect_exit 1 'L46 reject on carried approval still blocks' gate merge-ready 12

gate_repo
older_approval=$(git rev-parse HEAD)
git checkout -q main
printf 'main update\n' >docs/main.md
commit main-update
git update-ref refs/remotes/origin/main HEAD
git checkout -q work
git merge -q --no-ff main -m 'merge main' -m 'Author-Agent: builder'
carried_head=$(git rev-parse HEAD)
gate_set --arg head "$carried_head" --arg older "$older_approval" '.comments=[[{id:1,created_at:"2026-10-03T00:00:00Z",body:("VERDICT: reject\nReviewed-head: "+$head+"\nReviewed-by-Agent: reviewer")},{id:2,created_at:"2026-10-03T00:01:00Z",body:("VERDICT: approve\nReviewed-head: "+$older+"\nReviewed-by-Agent: reviewer-two")}]]'
expect_exit 1 'L46 approval of ancestor cannot clear reject on carried head' gate merge-ready 12

gate_repo
git checkout -q main
printf 'main update\n' >docs/main.md
commit main-update
git checkout -q work
git merge -q --no-ff main -m 'merge main' -m 'Author-Agent: builder'
expect_exit 0 'L54 pinned API base accepts carry when origin/main is stale' gate merge-ready 12
gate_set '.fail_carry=true'
expect_exit 4 'L46 carry gh failure is an infrastructure error in CI' gate ci-trailers 12
expect_exit 4 'L46 carry gh failure is an infrastructure error at merge' gate merge-ready 12

gate_repo
for state in queued in_progress completed; do
  gate_set '.checks=[{check_runs:[{name:"check", head_sha:"'"$(git rev-parse HEAD)"'",status:"'"$state"'",conclusion:"failure", app:{slug:"github-actions"}}]}]'
  expect_exit 1 "L46 $state unsuccessful check fails" gate merge-ready 12
done
gate_set '.checks=[{check_runs:[{name:"check",head_sha:"'"$(git rev-parse HEAD)"'",status:"completed",conclusion:"success",app:{slug:"github-actions"}}]}]'
expect_exit 1 'L46 passing check without rules fails' gate merge-ready 12
gate_set '.checks += [{check_runs:[{name:"rules",head_sha:"'"$(git rev-parse HEAD)"'",status:"completed",conclusion:"success",app:{slug:"github-actions"}}]}]'
expect_exit 0 'L46 checks on a second page are read' gate merge-ready 12
gate_set '.checks[1].check_runs[0].head_sha="0000000000000000000000000000000000000000"'
expect_exit 1 'L46 rules on an older head cannot pass' gate merge-ready 12
gate_set 'del(.checks) | .failure=true'
expect_exit 4 'L46 gh errors are visible failures' gate merge-ready 12

gate_repo
git reset -q --hard HEAD^
git commit -q --allow-empty -m approve -m 'Reviewed-by-Agent: builder'
expect_exit 1 'L46 self approval fails' gate merge-ready 12
git reset -q --hard HEAD^
echo change >crates/approval.rs
commit approval 'Reviewed-by-Agent: reviewer'
expect_exit 1 'L46 approval with changed content fails' gate merge-ready 12

gate_repo
git reset -q --hard main
for path in docs/notes.md scenarios/notes.md; do
  git reset -q --hard main
  mkdir -p "$(dirname "$path")"
  echo 'plain prose' >"$path"
  commit prose 'Author-Agent: builder'
  expect_output post "L44 allowed $path is post" gate class 12
done
for path in docs/CLAUDE.md scenarios/CLAUDE.md docs/AGENTS.md docs/.hidden.md docs/.hidden/a.md docs/adr/new.md docs/boxd.md docs/design-system.md docs/development-loop.md scenarios/app.md scenarios/mcp.md scenarios/daemon.md scenarios/loop-rules.md .work/queue.md .work/queue/notes.md .work/queue/.hidden.md; do
  git reset -q --hard main
  mkdir -p "$(dirname "$path")"
  echo prose >"$path"
  commit prose 'Author-Agent: builder'
  expect_output block "L44 excluded $path is block" gate class 12
done
for word in secret token credential password oauth identity impersonat daemon rupd 'api key' auth author-agent reviewed-by verdict architect approve merger rpc seam card loop key ＴＯＫＥＮ '++ secret' '-- secret'; do
  git reset -q --hard main
  echo "$word" >scenarios/notes.md
  commit prose 'Author-Agent: builder'
  expect_output block "L44 screen blocks $word" gate class 12
done

gate_repo
git reset -q --hard main
echo secret >scenarios/notes.md
commit setup
git branch -f main HEAD
echo prose >scenarios/notes.md
commit removal 'Author-Agent: builder'
expect_output block 'L44 removed screen word blocks' gate class 12

gate_repo
git reset -q --hard main
ln -s /tmp/ignored docs/link.md
commit link 'Author-Agent: builder'
expect_output block 'L44 symlink blocks' gate class 12

gate_repo
git reset -q --hard main
echo prose >docs/notes.md
commit setup
git branch -f main HEAD
chmod +x docs/notes.md
commit mode 'Author-Agent: builder'
expect_output block 'L44 mode change blocks' gate class 12

gate_repo
git reset -q --hard main
printf '\0binary' >docs/data.md
commit binary 'Author-Agent: builder'
expect_output block 'L44 binary blocks' gate class 12

gate_repo
git reset -q --hard main
echo prose >docs/notes.md
commit setup
git branch -f main HEAD
git mv docs/notes.md crates/notes.md
commit rename 'Author-Agent: builder'
expect_output block 'L44 rename out of docs blocks' gate class 12

gate_repo
git reset -q --hard main
echo prose >docs/notes.md
commit prose 'Author-Agent: builder'
gate_set '.files=[[{filename:"docs/notes.md"}]]'
expect_output block 'L44 omitted patch blocks' gate class 12
gate_set 'del(.files)'
echo changed >crates/more.rs
commit source 'Author-Agent: builder'
gate_set '.files=[[{filename:"docs/notes.md",patch:"p"}],[{filename:"crates/more.rs",patch:"p"}]]'
expect_output block 'L44 block path on page two blocks' gate class 12

# L76: a PR touching apps/desktop/src proves its look with the percy check on its head and a Percy build link.
gate_visible() {
  gate_repo
  git reset -q --hard main
  printf '**U1 a visible change.** Given a 1280 by 800 window, when opened, then a screenshot shows it.\n' >scenarios/ui.md
  commit scenario
  git branch -f main HEAD
  mkdir -p apps/desktop/src
  echo visible >apps/desktop/src/view.tsx
  commit visible 'Author-Agent: builder'
  git commit -q --allow-empty -m approve -m 'Reviewed-by-Agent: reviewer'
  gate_set '.body="Scenarios: U1\n## Shape\n```\na -> b\n```\n## Proof\nHead: '"$(git rev-parse HEAD)"'\n```\nok U1_visible\n```\nPercy: https://percy.io/abc/web/roundup/builds/42"'
}
gate_percy() {
  gate_set --arg head "$(git rev-parse HEAD)" --arg conclusion "$1" --arg at "${2:-$(git rev-parse HEAD)}" \
    '.checks=[{check_runs:([("check","rules")|{name:.,head_sha:$head,status:"completed",conclusion:"success",app:{slug:"github-actions"}}] + [{name:"percy",head_sha:$at,status:"completed",conclusion:$conclusion,app:{slug:"github-actions"}}])}]'
}
export PERCY_PROJECT=abc/web/roundup
gate_visible
expect_exit 0 'L76 a green percy check on the head and a Percy link pass' gate proof 12
expect_exit 1 'L76 an unset PERCY_PROJECT fails closed' env -u PERCY_PROJECT loop/rules.sh proof 12
gate_set '.body |= sub("percy.io/abc/web/roundup"; "percy.io/other/web/roundup")'
expect_exit 1 'L76 a link to another Percy project fails' gate proof 12
gate_visible
expect_exit 0 'L76 a green Percy build passes merge-ready' gate merge-ready 12
gate_set '.body |= sub("Percy: [^\n]*"; "")'
expect_exit 1 'L76 a visible PR without a Percy link fails' gate proof 12
gate_visible
gate_set '.body |= sub("https://percy.io/abc/web/roundup/builds/42"; "https://example.com/builds/42")'
expect_exit 1 'L76 a link outside percy.io fails' gate proof 12
gate_visible
gate_percy failure
expect_exit 1 'L76 a failed percy check fails' gate proof 12
gate_percy success 0000000000000000000000000000000000000000
expect_exit 1 'L76 a percy check on another head fails' gate proof 12
gate_set '.checks=[{check_runs:[("check","rules")|{name:.,head_sha:"'"$(git rev-parse HEAD)"'",status:"completed",conclusion:"success",app:{slug:"github-actions"}}]}]'
expect_exit 1 'L76 no percy check fails' gate proof 12
gate_visible
gate_set '.body |= sub("Percy: [^\n]*"; "U1 before [image](https://github.com/example/roundup/blob/proof/pr-12/proof/before.png) 1280×800\nShows: layout")'
expect_exit 1 'L76 images on a proof branch are not visual proof' gate proof 12
gate_set --arg head "$(git rev-parse HEAD^)" '.comments=[[{id:1,created_at:"2026-10-03T00:00:00Z",body:("VERDICT: approve\nReviewed-head: "+$head+"\nVisual: unchanged\nReviewed-by-Agent: reviewer")}]]'
expect_exit 1 'L76 Visual: unchanged no longer replaces a Percy build' gate proof 12

gate_desktop() {
  gate_repo
  git reset -q --hard main
  mkdir -p crates/desktop
  echo shell >crates/desktop/window.rs
  commit desktop 'Author-Agent: builder'
  git commit -q --allow-empty -m approve -m 'Reviewed-by-Agent: reviewer'
}
gate_desktop
expect_exit 1 'L76 a crates/desktop change without a macOS line fails' gate proof 12
gate_set --arg head "$(git rev-parse HEAD)" '.body="Scenarios: L46\n## Shape\n```\na\n```\n## Proof\nHead: "+$head+"\n```\nok L46_gate\n```\nmacOS: the window opens at 1280×800 in just app"'
expect_exit 0 'L76 a crates/desktop change needs only the macOS laptop check' gate proof 12

gate_repo
git reset -q --hard main
echo prose >docs/design-system.md
commit docs 'Author-Agent: builder'
git commit -q --allow-empty -m approve -m 'Reviewed-by-Agent: reviewer'
expect_exit 0 'L76 a docs-only change needs no Percy build' gate proof 12

gate_repo
printf '**U1 a visible change.** Given a 1280 by 800 window, when opened, then a screenshot shows it.\n' >scenarios/ui.md
commit scenario 'Author-Agent: builder'
git commit -q --allow-empty -m approve -m 'Reviewed-by-Agent: reviewer'
gate_set --arg head "$(git rev-parse HEAD)" '.body="Scenarios: U1\n## Shape\n```\na\n```\n## Proof\nHead: "+$head+"\n```\nok U1_visible\n```"'
expect_exit 0 'L76 naming a visual scenario outside apps/desktop/src needs no Percy build' gate proof 12

gate_specification_head() {
  gate_set --arg head "$(git rev-parse HEAD)" '.body |= sub("Head: [a-f0-9]+"; "Head: "+$head) | .comments=[[{id:1,created_at:"2026-10-04T00:00:00Z",body:("VERDICT: approve\nReviewed-head: "+$head+"\nReviewed-by-Agent: reviewer")}]]'
}

gate_specification() {
  gate_repo
  git reset -q --hard main
  printf '**A7 a Room.** Given a Room, when opened, then its screenshot shows the Door at 1280×800.\n' >scenarios/agents.md
  commit specification 'Author-Agent: builder'
  git commit -q --allow-empty -m approval -m 'Reviewed-by-Agent: reviewer'
  gate_set --arg head "$(git rev-parse HEAD)" '.body="Scenarios: A7\nProof scope: specification\n## Shape\n```\nRoom -> Door\n```\n## Proof\nHead: "+$head+"\nSpecification consistency: lifecycle and failure clauses reviewed.\nPending A7: run the real Daemon lifecycle test and capture the stated viewport.\n```text\njust check: exit 0\nSummary: existing tests passed\n```"'
  gate_specification_head
}

gate_specification
expect_exit 0 'L53 specification baseline and future observer pass without invented scenario execution' gate proof 12
expect_exit 0 'L53 specification still passes the ordinary merge gates' gate merge-ready 12
gate_set '.body |= sub("just check: exit 0"; "checks planned")'
expect_exit 1 'L53 specification missing baseline fails' gate proof 12
gate_specification
gate_set '.body |= sub("Pending A7:[^\n]*"; "")'
expect_exit 1 'L53 specification missing future observer fails' gate proof 12
gate_specification
gate_set '.body |= sub("Specification consistency:[^\n]*"; "")'
expect_exit 1 'L53 specification missing consistency review fails' gate proof 12
gate_specification
gate_set '.body |= sub("Proof scope: specification"; "Proof scope: implementation")'
expect_exit 1 'L53 unknown explicit proof scope fails' gate proof 12
for path in crates/change.rs AGENTS.md GLOSSARY.md PRINCIPLES.md scenarios/loop.md scenarios/rpc.md docs/development-loop.md; do
  gate_specification
  printf 'changed\n' >>"$path"
  commit mixed 'Author-Agent: builder'
  git commit -q --allow-empty -m approval -m 'Reviewed-by-Agent: reviewer'
  gate_specification_head
  expect_exit 1 "L53 specification cannot waive $path proof" gate proof 12
done
gate_specification
git checkout -q main
printf 'policy\n' >AGENTS.md
commit policy
git checkout -q work
git merge -q --no-ff main -m base -m 'Author-Agent: builder'
git mv AGENTS.md scenarios/awareness.md
commit rename 'Author-Agent: builder'
git commit -q --allow-empty -m approval -m 'Reviewed-by-Agent: reviewer'
gate_specification_head
expect_exit 1 'L53 specification rename cannot hide a policy source path' gate proof 12
gate_specification
ln -s ../crates/a.rs scenarios/todos.md
commit symlink 'Author-Agent: builder'
git commit -q --allow-empty -m approval -m 'Reviewed-by-Agent: reviewer'
gate_specification_head
expect_exit 1 'L53 specification refuses a symlink' gate proof 12

gate_specification
gate_set '.comments=[[]]'
expect_exit 1 'L53 author consistency claim without independent review fails' gate proof 12
gate_specification
gate_set '.comments[0][0].body |= sub("reviewer"; "builder")'
expect_exit 1 'L53 specification author cannot supply its independent review' gate proof 12
gate_specification
printf '**T1 a Todo.** Given a title, when created, then a Todo exists.\n' >scenarios/todos.md
commit product-spec 'Author-Agent: builder'
git commit -q --allow-empty -m approval -m 'Reviewed-by-Agent: reviewer'
gate_set '.body |= sub("Scenarios: A7"; "Scenarios: A7 T1") | .body += "\nPending T1: run the Todo creation observer after implementation.\n"'
gate_specification_head
expect_exit 0 'L53 later Todo product specifications use the same proof scope' gate proof 12

for path in docs/wireframes.md .work/prompts/product.md; do
  gate_specification
  mkdir -p "$(dirname "$path")"
  printf 'Product observer specification\n' >"$path"
  commit product-plan 'Author-Agent: builder'
  git commit -q --allow-empty -m approval -m 'Reviewed-by-Agent: reviewer'
  gate_specification_head
  expect_exit 0 "L53 specification accepts $path" gate proof 12
done
gate_specification
printf '**T1 a Todo.** Given a title, when created, then a Todo exists.\n' >scenarios/todos.md
ln -s ../crates/a.rs 'scenarios/[t]odos.md'
commit bracket-link 'Author-Agent: builder'
git commit -q --allow-empty -m approval -m 'Reviewed-by-Agent: reviewer'
gate_specification_head
expect_exit 1 'L53 bracket-named symlink cannot borrow the regular Todo file mode' gate proof 12

gate_visible
printf '**A7 backend lifecycle.** Given an Agent, when stopped, then its record remains.\n' >scenarios/agents.md
commit backend 'Author-Agent: builder'
gate_set --arg head "$(git rev-parse HEAD)" '.body |= sub("Head: [a-f0-9]+"; "Head: "+$head) | .body |= sub("Scenarios: U1"; "Scenarios: U1 A7") | .body |= sub("ok U1_visible"; "ok U1_visible\nok A7_lifecycle")'
expect_exit 0 'L53 mixed proof needs backend test output beside the Percy build' gate proof 12
gate_set '.body |= sub("ok A7_lifecycle"; "")'
expect_exit 1 'L53 mixed proof still requires backend test output' gate proof 12

gate_visible
gate_set '.body |= sub("## Shape"; "## Other")'
expect_exit 1 'L53 missing shape fails' gate proof 12
gate_visible
gate_set '.body |= sub("a -> b"; ([range(41)|"line"]|join("\n")))'
expect_exit 1 'L53 forty-one shape lines fail' gate proof 12


gate_repo
gate_set '.change_head=true'
expect_exit 1 'L46 head changing during the gate fails' gate merge-ready 12

gate_repo
ready=$(gate merge-ready 12)
git commit -q --allow-empty -m later -m 'Author-Agent: builder'
expect_exit 1 'L46 match-head-commit refuses a later push' gh pr merge 12 --match-head-commit "${ready#ready }"

gate_repo
git reset -q --hard main
expect_output block 'L44 empty diff is block' gate class 12
echo prose >docs/note.md
commit prose
expect_exit 1 'L46 post missing author fails' gate merge-ready 12

gate_repo
gate_rejects 1
gate_set '.comments += [[{id: 100, created_at: "2026-10-03T03:00:00Z", body: "VERDICT: approve\n'"$(git rev-parse HEAD)"'\nReviewed-by-Agent: reviewer"}]]'
expect_exit 0 'L46 newer independent approve clears head reject' gate merge-ready 12

gate_repo
gate_set '.body="Scenarios: L46\n## Shape\n```\n$(touch /tmp/roundup-gate-executed)\n```\n## Proof\nHead: '"$(git rev-parse HEAD)"'\n```\nok L46_gate\n```"'
expect_exit 0 'L46 PR text is data even with shell syntax' gate merge-ready 12
expect pass 'L46 shell syntax in PR text was not executed' test ! -e /tmp/roundup-gate-executed


gate_error_contains() {
  local wanted=$1 error
  shift
  error=$("$@" 2>&1 >/dev/null) || true
  [[ $error == *"$wanted"* ]]
}
gate_repo
gate_set '.base="architect/x"'
expect pass 'L33 names a non-main base on stderr' gate_error_contains 'base is architect/x, not main' gate base 12
gate_set '.failure=true'
expect pass 'L33 names gh on failure' gate_error_contains gh gate base 12
for command in class rounds proof merge-ready; do
  expect_exit 4 "L33 $command propagates gh failure" gate "$command" 12
done
gate_set '.failure=false | .hang=true'
expect pass 'L33 timeout names gh on stderr' gate_error_contains gh env BOXD_GH_TIMEOUT=1 loop/rules.sh base 12

gate_repo
git reset -q --hard main
echo content >crates/change.rs
commit author 'Author-Agent: architect-a'
git commit -q --allow-empty -m approve -m 'Reviewed-by-Agent: reviewer'
gate_rejects 3
gate_pick split architect-a
expect_exit 1 'L45 a committee architect who authored the PR cannot pick' gate rounds 12

gate_repo
git reset -q --hard main
echo prose >docs/old.md
commit setup
git branch -f main HEAD
git mv docs/old.md docs/new.md
commit rename 'Author-Agent: builder'
gate_set '.files=[[{filename:"docs/new.md",previous_filename:"docs/old.md",patch:"rename"}]]'
expect_output post 'L44 rename inside docs reads both allowed paths' gate class 12

gate_repo
git reset -q --hard main
git update-index --add --cacheinfo "160000,$(git rev-parse HEAD),docs/submodule"
git commit -qm submodule -m 'Author-Agent: builder'
expect_output block 'L44 submodule blocks' gate class 12

gate_repo
gate_set '.body="Scenarios: L46\n## Shape\n```\na\n```\n## Proof\nHead: '"$(git rev-parse HEAD)"'\n```\nok L45_wrong\n```"'
expect_exit 1 'L53 test output for another scenario fails' gate proof 12

gate_repo
gate_set '.body="Scenarios: L46\n## Shape\n```\n" + ([range(40)|"line"]|join("\n")) + "\n```\n## Proof\nHead: '"$(git rev-parse HEAD)"'\n```\nok L46_gate\n```"'
expect_exit 0 'L53 forty shape lines pass' gate proof 12


gate_repo
pr_head=$(git rev-parse HEAD)
gate_set '.head="'"$pr_head"'"'
trusted=$(mktemp -d)
git clone -q --no-local --single-branch --branch main "$PWD" "$trusted"
cd "$trusted"
expect fail 'L46 trusted checkout initially lacks PR objects' git cat-file -e "$pr_head^{commit}"
expect_output "ready $pr_head" 'L46 fetches missing PR objects as data' gate merge-ready 12
expect_output main 'L46 leaves the trusted branch checked out' git branch --show-current

ready_repo() {
  new_repo
  mkdir -p scenarios apps
  printf '%s\n' '**U1 one.** a' '**U2 two.** b' '**U3 three.** c' '' '## Work' '' \
    '| Ids | Item | Owns | Keeps green | After |' '|---|---|---|---|---|' \
    '| U1 | first | `a` | — | — |' '| U2 | second | `b` | — | after U3 |' '| U3 | third | `c` | — | — |' \
    '| U9 | unwritten | — | — | — |' '| U1, U3 | both | — | — | — |' >scenarios/ui.md
  echo 'test("u1_works", () => {});' >apps/u1.test.ts
  commit x
  # A fake `gh pr list` printing gh-prs, failing on GH_FAIL and hanging on GH_HANG.
  mkdir -p .git/ready-bin
  printf '#!/bin/sh\n[ -z "${GH_FAIL:-}" ] || exit 1\n[ -z "${GH_HANG:-}" ] || sleep 5\ncat "$PWD/.git/gh-prs"\n' >.git/ready-bin/gh
  chmod +x .git/ready-bin/gh
  echo '[]' >.git/gh-prs
  export PATH="$PWD/.git/ready-bin:$PATH"
}
ready() { loop/rules.sh ready "$@"; }

ready_repo
expect_output "done U1 scenarios/ui.md
waiting U2 scenarios/ui.md
ready U3 scenarios/ui.md
unspecified U9 scenarios/ui.md
ready U1, U3 scenarios/ui.md" 'L34 ready prints each Work row with its state' ready

ready_repo
echo 'test("u3_works", () => {});' >apps/u3.test.ts; commit x
expect_output "done U1 scenarios/ui.md
ready U2 scenarios/ui.md
done U3 scenarios/ui.md
unspecified U9 scenarios/ui.md
done U1, U3 scenarios/ui.md" 'L34 a done After frees the row' ready

ready_repo
printf '%s\n' '**L7 x.** a' '## Work' '| Ids | Item | Owns | Keeps green | After |' '|---|---|---|---|---|' '| L7 | x | — | — | — |' '| U4–U5 | y | — | — | — |' '**U4 a.** x' '**U5 b.** y' >scenarios/loop.md
printf 'echo L7\n' >loop/x.test.sh; echo 'fn u4_a() {} fn u5_b() {}' >crates/u.rs; commit x
expect_output "done L7 scenarios/loop.md
done U4–U5 scenarios/loop.md" 'L34 L ids read loop tests; a range needs every id' bash -c 'loop/rules.sh ready | grep loop.md'

ready_repo
printf '%s\n' '## Work' '| Ids | Item |' '|---|---|' '| U1 | x |' >scenarios/bad.md; commit x
expect_exit 2 'L34 a Work table missing a column exits 2' ready

ready_repo
echo '[{"number":301,"title":"U3 and U9: third"},{"number":302,"title":"U31 elsewhere"},{"number":303,"title":"U1 again"}]' >.git/gh-prs
expect_output "done U1 scenarios/ui.md
waiting U2 scenarios/ui.md
in-flight U3 scenarios/ui.md #301
in-flight U9 scenarios/ui.md #301
in-flight U1, U3 scenarios/ui.md #301 #303" 'L34 an open PR naming a row id makes it in-flight after done; every naming PR is listed' ready

ready_repo
expect_output "done U1 scenarios/ui.md
waiting U2 scenarios/ui.md
ready U3 scenarios/ui.md
unspecified U9 scenarios/ui.md
ready U1, U3 scenarios/ui.md" 'L34 no open PR leaves every state' ready

ready_repo
expect_exit 4 'L34 gh failure exits 4' env GH_FAIL=1 loop/rules.sh ready
expect pass 'L34 gh failure names gh' gate_error_contains gh env GH_FAIL=1 loop/rules.sh ready
expect pass 'L34 gh slower than BOXD_GH_TIMEOUT names gh' gate_error_contains gh env GH_HANG=1 BOXD_GH_TIMEOUT=1 loop/rules.sh ready
mkdir .git/no-gh-bin
for tool in bash dirname python3 git; do ln -s "$(command -v "$tool")" .git/no-gh-bin/; done
expect_exit 4 'L34 gh missing exits 4' env PATH="$PWD/.git/no-gh-bin" loop/rules.sh ready

ready_repo
echo '[{"number":301,"title":"U3"}]' >.git/gh-prs
expect_output "done U1 scenarios/ui.md
waiting U2 scenarios/ui.md
ready U3 scenarios/ui.md
unspecified U9 scenarios/ui.md
ready U1, U3 scenarios/ui.md" 'L34 --offline skips gh and keeps the four states' env GH_FAIL=1 loop/rules.sh ready --offline

[ "$failures" -eq 0 ] || { echo "$failures failed"; exit 1; }
