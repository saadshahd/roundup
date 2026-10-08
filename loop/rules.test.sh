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

rules() { loop/rules.sh "$1"; }

# L82
workflows="$(dirname "$script")/../.github/workflows"
pipefail_everywhere() {
  local w
  for w in "${1:-$workflows}"/*.yml; do
    [ "$(grep -A2 -x 'defaults:' "$w" | tr -d ' \n')" = defaults:run:shell:bash ] || return 1
  done
}
expect pass 'L82 every workflow runs its steps under bash -eo pipefail' pipefail_everywhere

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

gate_error_contains() {
  local wanted=$1 error
  shift
  error=$("$@" 2>&1 >/dev/null) || true
  [[ $error == *"$wanted"* ]]
}

# L46 touches: the one definition of a Code PR and of loop machinery.
touches() { printf "$1" | "$script" touches | tr '\n' ' ' | sed 's/ $//'; }
expect_output code 'L46 touches: apps, crates and contracts are code' touches 'docs/a.md\ncrates/x/y.rs\n'
expect_output code 'L46 touches: contracts are code' touches 'contracts/a.ts\n'
expect_output 'code ui loop' 'L46 touches: code, apps/desktop/src and loop machinery are each named' touches 'apps/desktop/src/a.tsx\nloop/rules.sh\n'
expect_output code 'L46 touches: another app is code but not ui' touches 'apps/web/src/a.tsx\n'
for path in loop/x.sh .github/workflows/a.yml .agents/builder.md .claude/settings.json AGENTS.md docs/AGENTS.md CLAUDE.md; do
  expect_output loop "L46 touches: $path is loop machinery" touches "$path\n"
done
for path in justfile Cargo.toml Cargo.lock package.json pnpm-lock.yaml pnpm-workspace.yaml rust-toolchain.toml tsconfig.json .oxlintrc.json .fallowrc.json .cargo/config.toml tools/oxlint/a.js; do
  expect_output code "L46 touches: the Build file $path is code" touches "$path\n"
done
expect_output 'code loop' 'L46 touches: an AGENTS.md inside a crate is both' touches 'crates/x/AGENTS.md\n'
expect_output '' 'L46 touches: docs and scenarios are neither' touches 'docs/a.md\nscenarios/ui.md\nGLOSSARY.md\nREADME.md\nskills-lock.json\n'
expect_output code 'L46 touches: a last path without a newline still counts' touches 'docs/a.md\ncrates/a.rs'

# L54 clean-merge: a throwaway repo with `origin/main` and a PR branch.
merge_repo() {
  new_repo
  git checkout -q main
  mkdir -p scenarios && echo a >scenarios/loop.md
  commit queue 'Author-Agent: t'
  git update-ref refs/remotes/origin/main HEAD
  git checkout -q -b pr
  echo work >work.txt
  commit work 'Author-Agent: t'
}
# main_commit <file> <content>: advance origin/main on its own branch, then come back to pr.
main_commit() {
  git checkout -q main && printf '%s' "$2" >"$1" && git add -A && git commit -qm "main change" -m "Author-Agent: t" && git update-ref refs/remotes/origin/main HEAD && git checkout -q pr
}
# merge_main: merge origin/main into pr; the caller resolves a conflict by writing the file, then calls finish_merge.
merge_main() { git merge -q --no-commit --no-ff origin/main >/dev/null 2>&1 || true; }
finish_merge() { git add -A && git commit -qm "Merge main" -m "${1-}"; }
clean_code() {
  local want=$1 name=$2 why=${3:-} got=0 out
  out=$(loop/rules.sh clean-merge HEAD 2>&1) || got=$?
  if [ "$got" -eq "$want" ] && [[ $out == *"$why"* ]]; then echo "ok:   $name"; else echo "FAIL: $name (wanted exit $want, got $got: $out)"; failures=$((failures + 1)); fi
}
merge_repo; main_commit other.txt x; merge_main; finish_merge
clean_code 0 "L54 a conflict-free merge of main is clean"
merge_repo; main_commit scenarios/loop.md changed; echo mine >>scenarios/loop.md; commit "edit loop" 'Author-Agent: t'
merge_main; echo resolved >scenarios/loop.md; finish_merge
clean_code 1 "L54 a resolved conflict is not clean (rule a)" "rule a"
merge_repo; main_commit other.txt x; merge_main; echo sneaky >>scenarios/loop.md; finish_merge
clean_code 1 "L54 a merge that also edits another file is not clean (rule c)" "rule c"
merge_repo; git checkout -q -b offmain main; echo x >off.txt; commit off 'Author-Agent: t'; git checkout -q pr
git merge -q --no-ff offmain -m "Merge offmain" >/dev/null
clean_code 1 "L54 a merge whose second parent is off main is not clean" "not a merge of main"
merge_repo; main_commit other.txt x; main_commit other.txt y
git merge -q --no-ff "$(git rev-parse main~1)" -m "Merge main" >/dev/null
clean_code 0 "L54 a merge of main's older ancestor is clean"
merge_repo
clean_code 1 "L54 a commit with one parent is not a merge" "not a merge"
merge_repo; main_commit other.txt x; git checkout -q -b side main; echo s >side.txt; commit side 'Author-Agent: t'; git checkout -q pr; git update-ref refs/remotes/origin/main main
git merge -q --no-ff -m "Octopus" main side >/dev/null 2>&1 || true
clean_code 1 "L54 an octopus merge is not clean" "not a merge of main"
merge_repo; git checkout -q --orphan unrelated; git rm -rqf . >/dev/null; echo r >root.txt; commit root 'Author-Agent: t'
git update-ref refs/remotes/origin/main HEAD; git checkout -q pr
git merge -q --no-ff --allow-unrelated-histories --no-edit origin/main -m "Merge main" >/dev/null 2>&1 || true
clean_code 1 "L54 a merge of unrelated history fails closed in merge-tree" "merge-tree"
expect_exit 2 "L54 clean-merge needs a commit" loop/rules.sh clean-merge

# L2, L33, L37, L46 read only fake GitHub data, with real commit trees. By default the PR is a Code PR whose head
# holds one trusted approve from github-actions[bot].
gate_repo() {
  new_repo
  mkdir -p docs scenarios
  printf '**L46 a gate.** Given a PR, when checked, then it passes.\n' >scenarios/loop.md
  commit fixtures
  git branch -f main HEAD
  echo changed >crates/change.rs
  commit change 'Author-Agent: builder'
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
assert args[0] == 'api' and '--paginate' in args and '--slurp' in args, args
route = args[1]
def git(*args):
    return subprocess.check_output(['git', *args], text=True).strip()
head = d.get('head') or git('rev-parse', 'work')
base = git('rev-parse', 'main')
pr = {'head': {'sha': head}, 'base': {'ref': d.get('base', 'main'), 'sha': base},
      'state': 'open', 'draft': d.get('draft', False), 'body': d.get('body', 'Scenarios: L46\n')}
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
    out = d.get('comments', [[{'id': 1, 'created_at': '2026-10-03T00:00:00Z', 'user': {'login': 'github-actions[bot]'},
                               'author_association': 'NONE', 'body': 'VERDICT: approve\nHead: '+head+'\n\nReviewed-by-Agent: reviewer'}]])
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
# gate_say <approve|reject> <sha, or '' for none> [agent] [login] [association]: append one verdict comment, an hour
# after the last; the first call replaces the default approve.
gate_say() {
  gate_set --arg v "$1" --arg sha "$2" --arg agent "${3:-reviewer}" --arg login "${4:-github-actions[bot]}" --arg assoc "${5:-NONE}" \
    '(.comments // [[]])[0] as $c | .comments = [$c + [{id: (($c | length) + 1), created_at: ("2026-10-03T0\($c | length):00:00Z"),
      user: {login: $login}, author_association: $assoc,
      body: ("VERDICT: \($v)\n" + (if $sha == "" then "" else "Head: \($sha)\n" end) + "\nReviewed-by-Agent: \($agent)")}]]'
}
gate_quiet() { gate_set '.comments=[[]]'; }
ready_at() { expect_output "ready $(git rev-parse HEAD)" "$1" gate merge-ready 12; }

# L2: every authored commit carries Author-Agent; an approval is a VERDICT comment, never a commit.
gate_repo
expect_exit 0 'L2 an authored commit passes' gate ci-trailers 12
echo y >crates/y.rs; commit two 'Author-Agent: a-1
Author-Agent: a-2'
expect_exit 0 'L2 two authors on one commit pass' gate ci-trailers 12
gate_repo; echo x >crates/x.rs; commit bare
expect_exit 1 'L2 a commit with no trailers fails' gate ci-trailers 12
gate_repo; git commit -q --allow-empty -m approval -m 'Reviewed-by-Agent: reviewer'
expect_exit 1 'L2 an approval commit fails' gate ci-trailers 12
expect pass 'L2 an approval commit is named' gate_error_contains 'an approval is a VERDICT comment' gate ci-trailers 12
for merge_trailer in missing authored approval invalid-id; do
  gate_repo
  git checkout -q main; printf 'main update\n' >docs/main.md; commit main-update; git checkout -q work
  want=1
  case $merge_trailer in
    missing) trailer=''; want=0 ;;
    authored) trailer='Author-Agent: builder'; want=0 ;;
    approval) trailer='Reviewed-by-Agent: reviewer' ;;
    invalid-id) trailer='Author-Agent: invalid id' ;;
  esac
  git merge -q --no-ff main -m 'merge main' -m "$trailer"
  expect_exit "$want" "L2 a $merge_trailer merge of main" gate ci-trailers 12
done
gate_repo
git checkout -q main; printf 'main update\n' >docs/main.md; commit main-update; git checkout -q work
git merge -q --no-ff --no-commit main; echo sneaky >>crates/change.rs; git add -A; git commit -qm 'merge main and edit'
expect_exit 1 'L2 an untrailed merge that changes more than main fails' gate ci-trailers 12
gate_repo; gate_set '.base="stack"'
expect_exit 1 'L2 ci-trailers needs base main' gate ci-trailers 12

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
}
copilot_repo; copilot_fix
expect_exit 0 'L37 a GitHub-signed Copilot Autofix commit needs no Author-Agent in CI' gate ci-trailers 12
ready_at 'L37 merge-ready accepts a Copilot Autofix commit'
gate_quiet; gate_say approve "$(git rev-parse HEAD)" copilot
expect_exit 1 'L37 copilot cannot approve a PR it touched' gate merge-ready 12
copilot_repo; copilot_fix ''
expect_exit 1 'L37 an unsigned commit claiming Copilot fails' gate ci-trailers 12
copilot_repo; copilot_fix other@example.com
expect_exit 1 'L37 a Copilot claim signed by another key fails' gate ci-trailers 12
copilot_repo; copilot_fix noreply@github.com noreply@github.com ''
expect_exit 1 'L37 a GitHub-signed commit without the Copilot Autofix trailer fails' gate ci-trailers 12
copilot_repo; copilot_fix noreply@github.com someone@example.com
expect_exit 1 'L37 a Copilot claim GitHub did not commit fails' gate ci-trailers 12
gate_repo; gate_quiet; gate_say approve "$(git rev-parse HEAD)" copilot
expect_exit 1 'L37 copilot cannot approve a PR it did not touch' gate merge-ready 12
copilot_repo
git checkout -q main; echo m >docs/main.md; git add -A; git commit -qm main-update; git checkout -q work
git merge -q --no-ff --no-commit main; echo extra >>crates/change.rs; git add -A
GIT_COMMITTER_NAME=GitHub GIT_COMMITTER_EMAIL=noreply@github.com git -c gpg.format=openpgp -c user.signingkey=noreply@github.com commit -q -S -m 'merge main' -m 'Co-authored-by: Copilot Autofix powered by AI <62310815+github-advanced-security[bot]@users.noreply.github.com>'
expect_exit 1 'L37 a signed Copilot merge that changes more than main still needs Author-Agent' gate ci-trailers 12
unset GNUPGHOME

# L33 base.
gate_repo
expect_exit 0 'L33 base main passes' gate base 12
gate_set '.base="architect/x"'
expect_exit 1 'L33 stacked base fails' gate base 12
expect pass 'L33 names a non-main base on stderr' gate_error_contains 'base is architect/x, not main' gate base 12
gate_set '.failure=true'
expect_exit 4 'L33 gh failure is not success' gate base 12
expect pass 'L33 names gh on failure' gate_error_contains gh gate base 12
expect_exit 2 'L33 missing number fails before gh' gate base
expect_exit 2 'L33 nonnumeric number fails before gh' gate base 12a
gate_set '.failure=false | .hang=true'
expect_exit 4 'L33 gh timeout is bounded' env LOOP_GH_TIMEOUT=0.2 loop/rules.sh base 12
expect pass 'L33 timeout names gh on stderr' gate_error_contains gh env LOOP_GH_TIMEOUT=0.2 loop/rules.sh base 12
gate_set '.hang=false | .base="main"'
expect_exit 0 'L33 a fractional LOOP_GH_TIMEOUT is honoured' env LOOP_GH_TIMEOUT=0.5 loop/rules.sh base 12

# L46 merge-ready: verdicts.
gate_repo
ready_at 'L46 a Code PR approved on its head is ready'
gate_quiet
expect_exit 1 'L46 a Code PR without an approve fails' gate merge-ready 12
expect pass 'L46 a missing approve is named' gate_error_contains 'needs an independent VERDICT: approve' gate merge-ready 12
gate_say approve "$(git rev-parse HEAD)" builder
expect_exit 1 'L46 the author cannot approve' gate merge-ready 12
gate_quiet; gate_say approve "$(git rev-parse HEAD)" reviewer stranger NONE
expect_exit 1 'L46 a verdict from a commenter without write access counts for nothing' gate merge-ready 12
gate_quiet; gate_say approve "$(git rev-parse HEAD)" reviewer saadshahd OWNER
ready_at 'L46 a person with write access can approve'
gate_quiet; gate_say approve '' reviewer
expect_exit 1 'L46 an approve naming no SHA covers nothing' gate merge-ready 12
gate_set --arg head "$(git rev-parse HEAD)" --arg older "$(git rev-parse HEAD^)" '.comments=[[{id:1,created_at:"2026-10-03T00:00:00Z",user:{login:"github-actions[bot]"},author_association:"NONE",body:("VERDICT: approve\nHead: "+$head+"\nCompared with "+$older+"\n\nReviewed-by-Agent: reviewer")}]]'
ready_at 'L46 a Head line wins over other SHAs in the body'
gate_repo; gate_set '.body="No scenario here"'
expect_exit 1 'L46 a Code PR needs a Scenarios line' gate merge-ready 12

gate_repo
approved=$(git rev-parse HEAD)
git checkout -q main; printf 'main update\n' >docs/main.md; commit main-update; git checkout -q work
git merge -q --no-ff main -m 'merge main without a trailer'
gate_quiet; gate_say approve "$approved"
ready_at 'L46 an approve covers later clean merges of main'
expect_output "approve $approved true" 'L46 verdicts says the approve covers the head' bash -c "loop/rules.sh verdicts 12 | jq -r '\"\(.verdict) \(.head) \(.current)\"'"
git reset -q --hard HEAD^; git merge -q --no-ff --no-commit main; echo sneaky >>crates/change.rs; git add -A; git commit -qm 'merge main and edit' -m 'Author-Agent: builder'
expect_exit 1 'L46 an approve does not cover a merge that changes more than main' gate merge-ready 12
git reset -q --hard "$approved"; echo more >>crates/change.rs; commit more 'Author-Agent: builder'
expect_exit 1 'L46 an approve does not cover a later authored commit' gate merge-ready 12

gate_repo
head=$(git rev-parse HEAD)
gate_quiet; gate_say reject "$head"
expect pass 'L46 a reject on the head blocks' gate_error_contains 'on this head is reject' gate merge-ready 12
gate_say approve "$head"
ready_at 'L46 a newer approve of the head clears its reject'
gate_quiet; gate_say approve "$head"; gate_say reject "$head"
expect pass 'L46 a reject after the approve blocks' gate_error_contains 'on this head is reject' gate merge-ready 12
gate_quiet; gate_say reject ''
expect pass 'L46 a reject naming no SHA blocks' gate_error_contains 'on this head is reject' gate merge-ready 12
gate_say approve "$head"
ready_at 'L46 a later approve naming the head clears a reject naming no SHA'
gate_quiet; gate_say reject 0000000000000000000000000000000000000000; gate_say approve "$head"
ready_at 'L46 a reject of a SHA no longer in the PR does not block'
gate_set --arg head "$head" --arg older "$(git rev-parse HEAD^)" '.comments=[[{id:1,created_at:"2026-10-03T00:00:00Z",user:{login:"github-actions[bot]"},author_association:"NONE",body:("VERDICT: reject\nReviewed "+$head+"; compared with "+$older+"\n\nReviewed-by-Agent: reviewer")}]]'
expect pass 'L46 a reject naming two SHAs without a Head line blocks' gate_error_contains 'on this head is reject' gate merge-ready 12
gate_say approve "$head"
ready_at 'L46 a later approve of the head clears a reject naming two SHAs'

gate_repo
rejected=$(git rev-parse HEAD)
echo fix >>crates/change.rs; commit fix 'Author-Agent: builder'
gate_quiet; gate_say reject "$rejected"; gate_say approve "$(git rev-parse HEAD)"
ready_at 'L46 an approve of the fix clears the reject of its parent'
gate_quiet; gate_say reject "$(git rev-parse HEAD)"; gate_say approve "$rejected"
expect pass 'L46 an approve of an ancestor cannot clear a reject on the head' gate_error_contains 'on this head is reject' gate merge-ready 12
gate_quiet; gate_say reject "$rejected"; gate_say reject "$(git rev-parse HEAD)"; gate_say approve "$(git rev-parse HEAD)"
expect_exit 1 'L46 a second reject blocks even after an approve' gate merge-ready 12
expect pass 'L46 a second reject names the user' gate_error_contains 'second reject: the user decides' gate merge-ready 12
gate_quiet; gate_say reject "$rejected" builder; gate_say reject "$rejected" builder; gate_say approve "$(git rev-parse HEAD)"
ready_at 'L46 author verdicts count for nothing'
gate_quiet; gate_say reject "$rejected"; gate_say reject "$rejected" builder; gate_say approve "$(git rev-parse HEAD)"
expect_output "$(printf 'reject %s\napprove %s' "$rejected" "$(git rev-parse HEAD)")" 'L46 verdicts prints each independent verdict in order' \
  bash -c "loop/rules.sh verdicts 12 | jq -r '\"\\(.verdict) \\(.head)\"'"
expect_exit 2 'L46 verdicts needs a number' gate verdicts

# L46 merge-ready: what the PR touches.
gate_repo; git reset -q --hard main; echo prose >docs/notes.md; commit docs 'Author-Agent: builder'; gate_quiet
ready_at 'L46 a docs PR needs no verdict'
echo 'more' >loop/x.sh; commit loop 'Author-Agent: builder'
expect_exit 1 'L46 a loop PR waits for the user' gate merge-ready 12
expect pass 'L46 a loop PR names the user' gate_error_contains 'the user merges' gate merge-ready 12
gate_repo; git reset -q --hard main; mkdir -p docs/sub; echo x >docs/sub/AGENTS.md; commit agents 'Author-Agent: builder'; gate_quiet
expect_exit 1 'L46 a nested AGENTS.md waits for the user' gate merge-ready 12
gate_repo; git reset -q --hard main; echo prose >docs/notes.md; commit docs; gate_quiet
expect_exit 1 'L46 a docs PR still needs Author-Agent' gate merge-ready 12

gate_visible() {
  gate_repo
  git reset -q --hard main
  mkdir -p apps/desktop/src
  echo visible >apps/desktop/src/view.tsx
  commit visible 'Author-Agent: builder'
}
gate_percy() {
  gate_set --arg head "$(git rev-parse HEAD)" --arg conclusion "$1" --arg at "${2:-$(git rev-parse HEAD)}" \
    '.checks=[{check_runs:([("check","rules")|{name:.,head_sha:$head,status:"completed",conclusion:"success",app:{slug:"github-actions"}}] + [{name:"percy",head_sha:$at,status:"completed",conclusion:$conclusion,app:{slug:"github-actions"}}])}]'
}
gate_visible
ready_at 'L46 a visible PR with a green percy check on its head is ready'
gate_percy failure
expect_exit 1 'L46 a failed percy check fails' gate merge-ready 12
gate_percy success 0000000000000000000000000000000000000000
expect_exit 1 'L46 a percy check on another head fails' gate merge-ready 12
gate_set '.checks=[{check_runs:[("check","rules")|{name:.,head_sha:"'"$(git rev-parse HEAD)"'",status:"completed",conclusion:"success",app:{slug:"github-actions"}}]}]'
expect_exit 1 'L46 a visible PR without a percy check fails' gate merge-ready 12
gate_repo; git reset -q --hard main; echo prose >docs/notes.md; commit docs 'Author-Agent: builder'; gate_quiet
gate_set '.checks=[{check_runs:[("check","rules")|{name:.,head_sha:"'"$(git rev-parse HEAD)"'",status:"completed",conclusion:"success",app:{slug:"github-actions"}}]}]'
ready_at 'L46 a PR outside apps/desktop/src needs no percy check'

gate_repo; git reset -q --hard main; mkdir -p crates/desktop; echo shell >crates/desktop/window.rs; commit desktop 'Author-Agent: builder'
expect_exit 1 'L46 a crates/desktop PR without a macOS line fails' gate merge-ready 12
gate_set '.body="Scenarios: L46\nmacOS: the window opens at 1280×800 in just app\n"'
ready_at 'L46 a crates/desktop PR with a macOS line is ready'

# L81: a draft whose body has a Stopped: line waits on the user; the line alone, or a draft alone, does not.
gate_repo; gate_set '.draft=true | .body="Scenarios: L46\nStopped: needs a GLOSSARY term for Pin\n"'
expect pass 'L81 a draft with a Stopped line names why its Builder stopped' gate_error_contains 'the Builder stopped: needs a GLOSSARY term for Pin' gate merge-ready 12
gate_set '.body="Scenarios: L46\r\nStopped: needs a term.\r\n"'
expect pass 'L81 a body edited on GitHub, with CRLF line ends, names why' gate_error_contains 'the Builder stopped: needs a term.' gate merge-ready 12
expect fail 'L81 the reason holds no CR' gate_error_contains $'\r' gate merge-ready 12
gate_set '.body="Scenarios: L46\n"'
expect fail 'L81 a draft without a Stopped line is not stopped' gate_error_contains 'the Builder stopped' gate merge-ready 12
gate_set '.draft=false | .body="Scenarios: L46\nStopped: needs a GLOSSARY term for Pin\n"'
ready_at 'L81 a ready PR with an old Stopped line is ready'

# L46 merge-ready: checks, base and the pinned head.
gate_repo
gate_set '.checks=[{"check_runs":[]}]'
expect_exit 1 'L46 missing checks fail' gate merge-ready 12
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
gate_set '.checks=[{check_runs:([("check","rules")|{name:.,head_sha:"'"$(git rev-parse HEAD)"'",status:"completed",conclusion:"success",app:{slug:"github-actions"}}] + [{name:"check",head_sha:"'"$(git rev-parse HEAD)"'",status:"completed",conclusion:"skipped",app:{slug:"github-actions"}}])}]'
expect_exit 0 'L46 a check skipped on a draft head counts for nothing beside the one run when ready' gate merge-ready 12
gate_set '.checks[0].check_runs |= map(select(.name != "check" or .conclusion == "skipped"))'
expect_exit 1 'L46 a skipped check alone fails' gate merge-ready 12
gate_set 'del(.checks) | .base="stack"'
expect_exit 1 'L46 non-main base fails' gate merge-ready 12
gate_set 'del(.base) | .failure=true'
for command in ci-trailers verdicts merge-ready; do
  expect_exit 4 "L46 $command propagates gh failure" gate "$command" 12
done

gate_repo
gate_set '.change_head=true'
expect_exit 1 'L46 head changing during the gate fails' gate merge-ready 12

gate_repo
ready=$(gate merge-ready 12)
git commit -q --allow-empty -m later -m 'Author-Agent: builder'
expect_exit 1 'L46 match-head-commit refuses a later push' gh pr merge 12 --match-head-commit "${ready#ready }"

gate_repo
gate_set '.body="Scenarios: L46\n$(touch /tmp/roundup-gate-executed)\n"'
expect_exit 0 'L46 PR text is data even with shell syntax' gate merge-ready 12
expect pass 'L46 shell syntax in PR text was not executed' test ! -e /tmp/roundup-gate-executed

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
  printf '#!/bin/sh\n[ -z "${GH_FAIL:-}" ] || exit 1\n[ -z "${GH_HANG:-}" ] || exec sleep 5\ncat "$PWD/.git/gh-prs"\n' >.git/ready-bin/gh
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
echo '[{"number":301,"title":"U2: waits for the U3 row"},{"number":302,"title":"Investigate U3 readiness"}]' >.git/gh-prs
expect_output "done U1 scenarios/ui.md
in-flight U2 scenarios/ui.md #301
ready U3 scenarios/ui.md
unspecified U9 scenarios/ui.md
ready U1, U3 scenarios/ui.md" 'L34 dependency mentions in titles never reserve prerequisite work' ready

ready_repo
echo '[{"number":301,"title":"U2–U3, U9: after U1"}]' >.git/gh-prs
expect_output "done U1 scenarios/ui.md
in-flight U2 scenarios/ui.md #301
in-flight U3 scenarios/ui.md #301
in-flight U9 scenarios/ui.md #301
in-flight U1, U3 scenarios/ui.md #301" 'L34 leading grouped ranges reserve every owned id' ready

ready_repo
expect_exit 4 'L34 gh failure exits 4' env GH_FAIL=1 loop/rules.sh ready
expect pass 'L34 gh failure names gh' gate_error_contains gh env GH_FAIL=1 loop/rules.sh ready
expect pass 'L34 gh slower than LOOP_GH_TIMEOUT names gh' gate_error_contains gh env GH_HANG=1 LOOP_GH_TIMEOUT=0.2 loop/rules.sh ready
expect pass 'L34 a fractional LOOP_GH_TIMEOUT is honoured' env LOOP_GH_TIMEOUT=0.5 loop/rules.sh ready
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
