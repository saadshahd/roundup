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

# L84
actions="$(dirname "$script")/../.github/actions"
job_runner() { awk -v job="$2" '$0 ~ "^  "job":$" {f=1; next} f && /^  [a-z-]+:$/ {f=0} f && /runs-on:/ {print $2; exit}' "$1"; }
runs_on() { [ "$(job_runner "$workflows/$1.yml" "$2")" = "$3" ]; }
expect pass 'L84 build runs on ubuntu-latest' runs_on build-issue build ubuntu-latest
expect pass 'L84 fix runs on ubuntu-latest' runs_on fix fix-run ubuntu-latest
expect pass 'L84 check on a PR stays on macos-latest' runs_on check rust macos-latest
expect pass 'L84 each push to main runs just check on ubuntu-latest' bash -c "runs_on() { $(declare -f job_runner); job_runner \"\$@\"; }; [ \"\$(runs_on '$workflows/check.yml' linux)\" = ubuntu-latest ] && grep -q 'github.event_name == .push.' '$workflows/check.yml' && grep -q 'run: just check\$' '$workflows/check.yml'"
expect pass 'L84 no Linux green merges alone' bash -c "! sed -n '/^  check:/,/^  summary:/p' '$workflows/check.yml' | grep -E 'needs: \\[.*linux'"
expect pass 'L84 setup installs Tauri packages on Linux and coreutils on macOS' bash -c "for p in libwebkit2gtk-4.1-dev build-essential libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev; do grep -q \"\$p\" '$actions/setup/action.yml' || exit 1; done; grep -q 'brew install coreutils' '$actions/setup/action.yml'"
expect pass 'L84 the Rust cache is keyed by the runner OS' grep -q 'shared-key: check-${{ runner.os }}' "$actions/setup/action.yml"

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

# L2, L33, L37 read only fake GitHub data, with real commit trees.
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
assert args[0] == 'api' and '--paginate' in args and '--slurp' in args, args
route = args[1]
def git(*args):
    return subprocess.check_output(['git', *args], text=True).strip()
head = d.get('head') or git('rev-parse', 'work')
base = git('rev-parse', 'main')
pr = {'head': {'sha': head}, 'base': {'ref': d.get('base', 'main'), 'sha': base}}
if '/pulls/' in route:
    out = [pr]
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
copilot_repo; copilot_fix ''
expect_exit 1 'L37 an unsigned commit claiming Copilot fails' gate ci-trailers 12
copilot_repo; copilot_fix other@example.com
expect_exit 1 'L37 a Copilot claim signed by another key fails' gate ci-trailers 12
copilot_repo; copilot_fix noreply@github.com noreply@github.com ''
expect_exit 1 'L37 a GitHub-signed commit without the Copilot Autofix trailer fails' gate ci-trailers 12
copilot_repo; copilot_fix noreply@github.com someone@example.com
expect_exit 1 'L37 a Copilot claim GitHub did not commit fails' gate ci-trailers 12
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

# L2: ci-trailers reads the PR's commits as data, from a trusted checkout of main.
gate_repo
gate_set '.failure=true'
expect_exit 4 'L2 ci-trailers propagates gh failure' gate ci-trailers 12
gate_repo
pr_head=$(git rev-parse HEAD)
gate_set '.head="'"$pr_head"'"'
trusted=$(mktemp -d)
git clone -q --no-local --single-branch --branch main "$PWD" "$trusted"
cd "$trusted"
expect fail 'L2 trusted checkout initially lacks PR objects' git cat-file -e "$pr_head^{commit}"
expect_exit 0 'L2 ci-trailers fetches missing PR objects as data' gate ci-trailers 12
expect_output main 'L2 ci-trailers leaves the trusted branch checked out' git branch --show-current

# L41
src_repo() { new_repo; mkdir -p apps/desktop/src/rail apps/desktop/src/terminal apps/desktop/src/testing; }
tokens_with() { src_repo; printf '%b\n' "$2" >"apps/desktop/src/$1"; commit x; }
tokens_out() { loop/rules.sh tokens 2>&1 || true; }
tokens_says() { [ "$(tokens_out)" = "$1" ]; }
tokens_pass() { [ -z "$(tokens_out)" ] && loop/rules.sh tokens; }
tokens_fail() { ! loop/rules.sh tokens >/dev/null 2>&1; }
tokens_fail_all() { local f=$1 d; shift; for d in "$@"; do tokens_with "$f" "$d"; tokens_fail || return 1; done; }
tokens_pass_all() { local f=$1 d; shift; for d in "$@"; do tokens_with "$f" "$d"; tokens_pass || return 1; done; }

tokens_with rail/a.css '.a {\n  margin: 0;\n  color: #fff;\n}'
expect pass 'l41_a_hex_colour_fails_naming_file_line_and_property' tokens_says 'apps/desktop/src/rail/a.css:3 color'
expect pass 'l41_rgba_hsl_and_a_bare_colour_name_fail' tokens_fail_all rail/a.css '.a { background: rgba(0,0,0,.5); }' '.a { fill: hsl(0 0% 0%); }' '.a { border: 1px solid red; }' '.a { outline-color: oklch(0.5 0 0); }'
expect pass 'l41_a_unit_font_size_and_radius_fail' tokens_fail_all rail/a.css '.a { font-size: 12px; }' '.a { border-radius: 50%; }'
expect pass 'l41_a_box_shadow_and_a_duration_fail' tokens_fail_all rail/a.css '.a { box-shadow: 0 1px 2px #000; }' '.a { box-shadow: 0 1px 2px; }' '.a { transition: color 120ms; }' '.a { animation-duration: .2s; }'
expect pass 'l41_var_reads_pass' tokens_pass_all rail/a.css '.a { color: var(--text); border: 1px solid var(--hairline); font-size: var(--size); box-shadow: var(--shadow); transition: color var(--dur); }'
expect pass 'l41_transparent_inherit_currentcolor_none_and_0s_pass' tokens_pass_all rail/a.css '.a { background: transparent; color: inherit; fill: currentColor; border: none; box-shadow: none; transition: color 0s; }'
tokens_with tokens.css ':root { --text: #1d1d1f; font-size: 12px; box-shadow: 0 0 1px red; transition: 1s; }'
expect pass 'l41_tokens_css_is_not_read' tokens_pass
src_repo; echo 'const o = { fontSize: 12 };' >apps/desktop/src/terminal/emulator.ts; echo '.a { color: #fff; }' >apps/desktop/src/testing/a.css; echo 'x = <b style={{ color: "red" }} />;' >apps/desktop/src/rail/a.test.tsx; commit x
expect pass 'l41_emulator_ts_and_test_files_are_not_read' tokens_pass
tokens_with rail/a.tsx 'const a = <b style={{ color: "var(--text)", fontSize: "var(--s)" }} />;'
tokens_pass_ok=$(tokens_pass && echo y || echo n)
tokens_with rail/a.tsx 'const a = (\n  <b style={{ color: "var(--text)", borderRadius: "4px" }} />\n);'
expect pass 'l41_an_inline_style_literal_fails_and_a_var_passes' bash -c "[ '$tokens_pass_ok' = y ] && [ \"\$(loop/rules.sh tokens 2>&1)\" = 'apps/desktop/src/rail/a.tsx:2 borderRadius' ]"
tokens_with rail/a.css '/* color: #fff; */\n.a { color: var(--x); /* font-size: 12px */ }'
comment_css=$(tokens_pass && echo y || echo n)
tokens_with rail/a.tsx '// <b style={{ color: "red" }} />\nconst a = 1;'
expect pass 'l41_a_literal_in_a_comment_passes' bash -c "[ '$comment_css' = y ] && loop/rules.sh tokens"
expect_exit 2 'l41_an_argument_exits_2' loop/rules.sh tokens x

# Y3 the laws sheet
laws_repo() { new_repo; mkdir -p scenarios tests; printf '**A1 one.** text\n%s\n' "$1" >scenarios/a.md; echo 'it("a1_works", () => {});' >tests/a.test.ts; commit x; }
laws_repo 'Law: Things hold. Check: a1_'
expect pass 'y3_a_law_with_a_matching_test_prints_one_line' bash -c "[ \"\$(loop/rules.sh laws)\" = 'A1 | Things hold | a1_' ]"
laws_repo 'Law: Things hold.'
expect fail 'y3_a_law_with_no_check_exits_1' loop/rules.sh laws
expect pass 'y3_a_law_with_no_check_prints_unchecked' bash -c "loop/rules.sh laws 2>/dev/null | grep -q 'A1 | Things hold | unchecked'"
laws_repo 'Law: Things hold. Check: z9_'
expect fail 'y3_a_check_naming_a_prefix_no_test_has_exits_1' loop/rules.sh laws
expect pass 'y3_that_exit_names_the_id' bash -c "loop/rules.sh laws 2>&1 | grep -q 'A1 names'"
new_repo
expect_exit 4 'y3_a_missing_scenarios_directory_exits_4' loop/rules.sh laws

# L34 Issue queue behavior is exercised by loop/orders.test.py.

[ "$failures" -eq 0 ] || { echo "$failures failed"; exit 1; }
