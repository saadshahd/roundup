#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
mkdir -p "$dir/bin" "$dir/loop"
# Execute the trusted workflow itself so a regression cannot hide in a copied guard.
sed -n '/^        run: |$/,$ { /^          /s/^          //p; }' \
  "$root/.github/workflows/merge-ready.yml" >"$dir/compute.sh"
test -s "$dir/compute.sh"
cat >"$dir/bin/gh" <<'GH'
#!/usr/bin/env bash
set -euo pipefail
case "$*" in
  'pr view 281 --json headRefOid --jq .headRefOid') echo head ;;
  'pr diff 281 --name-only') echo "$PR_PATH" ;;
  'pr view 281 --json autoMergeRequest --jq .autoMergeRequest != null')
    echo read >>"$TRACE"
    if [ "$CASE" = read_error ]; then echo 'GitHub read failed' >&2; exit 1; fi
    printf '{"autoMergeRequest":%s}\n' "$AUTO_MERGE" | jq -r "$7"
    ;;
  'pr merge 281 --disable-auto')
    echo disarm >>"$TRACE"
    if [ "$AUTO_MERGE" = null ] || [ "$CASE" = disarm_error ]; then exit 1; fi
    ;;
  'api repos/test/repo/statuses/head -f state='*)
    printf '%s\n' "$4" >>"$TRACE"
    ;;
  'api repos/test/repo/issues/281') printf '%s\n' "$ISSUE" ;;
  'api repos/test/repo/issues/281/labels -f labels[]=flag:needs-user') echo label >>"$TRACE" ;;
  'api repos/test/repo/issues/281/assignees -f assignees[]=owner') echo assign >>"$TRACE" ;;
  'api repos/test/repo/issues/281/comments?per_page=100 --paginate') printf '%s\n' "$COMMENTS" ;;
  'api repos/test/repo/issues/281/comments -f body='*) echo comment >>"$TRACE"; printf '%s' "${4#body=}" >"$BODY" ;;
  'api -X PATCH repos/test/repo/issues/comments/9 -f body='*) echo edit >>"$TRACE"; printf '%s' "${6#body=}" >"$BODY" ;;
  'api -X DELETE repos/test/repo/issues/281/labels/flag:needs-user') echo unlabel >>"$TRACE" ;;
  'api -X DELETE repos/test/repo/issues/281/assignees -f assignees[]=owner') echo unassign >>"$TRACE" ;;
  *) echo "unexpected gh call: $*" >&2; exit 1 ;;
esac
GH
cat >"$dir/loop/rules.sh" <<'RULES'
#!/usr/bin/env bash
set -euo pipefail
if [ "$*" = touches ]; then exec bash "$ROOT/loop/rules.sh" touches; fi
[ "$*" = 'merge-ready 281' ]
echo gates >>"$TRACE"
if [ "$CASE" = transient ] && [ "$(grep -c '^gates$' "$TRACE")" -eq 1 ]; then
  echo 'merge-ready: PR changed during merge-ready'
  exit 1
fi
printf '%s\n' "$GATES_OUT"
exit "$GATES_CODE"
RULES
chmod +x "$dir/bin/gh" "$dir/loop/rules.sh"
export PATH="$dir/bin:$PATH" ROOT="$root" REPO=test/repo PR_FROM_PR=281 PR_FROM_COMMENT='' PR_FROM_DISPATCH='' PR_FROM_RUN=''
export TRACE="$dir/trace" BODY="$dir/body" AUTO_MERGE PR_PATH CASE OWNER=owner
export GATES_OUT='ready head' GATES_CODE=0 ISSUE='{"labels":[],"assignees":[]}' COMMENTS='[]'
cd "$dir"
failures=0
expect() {
  local name=$1 want=$2 expected=$3 message=${4:-} code=0
  : >"$TRACE"
  bash -eo pipefail compute.sh >output 2>&1 || code=$?
  if [ "$code" -eq "$want" ] && [ "$(cat "$TRACE")" = "$expected" ] &&
      { [ -z "$message" ] || grep -Fq "$message" output; }; then
    echo "ok: $name"
  else
    echo "FAIL: $name (exit $code, wanted $want)"
    cat "$TRACE" output
    failures=$((failures + 1))
  fi
}
for PR_PATH in .github/workflows/check.yml loop/rules.sh .agents/builder.md AGENTS.md crates/rupd/src/main.rs; do
  CASE=enabled AUTO_MERGE='{"enabledAt":"2026-10-04T00:00:00Z"}'
  expect "l78_approved_changes_keep_auto_merge_${PR_PATH//[^A-Za-z]/_}" 0 $'gates\nstate=success'
done

CASE=transient PR_PATH=crates/rupd/src/main.rs
expect l46_transient_gate_mutation_retries 0 $'gates\ngates\nstate=success'
CASE=outside GATES_OUT='merge-ready: PR changed during merge-ready' GATES_CODE=1
expect l46_repeated_mutation_is_bounded 0 $'gates\ngates\ngates\nstate=failure'
GATES_OUT='ready head' GATES_CODE=0

# L79: only a decision requiring the user raises a question.
mac='a crates/desktop PR needs a macOS: line'
asked_mac=$'<!-- needs-user -->\nmerge-ready waits on you:\n\n- It changes crates/desktop: did `just app` work on your Mac? If so, add a `macOS:` line to the body.'
body_is() {
  if [ "$(cat "$BODY")" = "$1" ]; then echo "ok: $2"; else echo "FAIL: $2"; cat "$BODY"; failures=$((failures + 1)); fi
}
CASE=outside PR_PATH=crates/rupd/src/main.rs GATES_CODE=1
GATES_OUT="merge-ready: check: missing or not successful on head head"$'\n'"$mac"
expect l79_a_reason_on_a_later_line_labels_assigns_and_asks 0 $'gates\nstate=failure\nlabel\nassign\ncomment'
body_is "$asked_mac" l79_the_comment_asks_the_reason_after_a_marker
ISSUE='{"labels":[{"name":"flag:needs-user"}],"assignees":[{"login":"owner"}]}'
COMMENTS=$(jq -nc --arg b "$asked_mac" '[{id: 9, user: {login: "github-actions[bot]"}, body: $b}]')
expect l79_a_repeat_changes_nothing 0 $'gates\nstate=failure'
GATES_OUT='ready head' GATES_CODE=0
expect l79_no_reason_left_takes_the_label_and_the_assignee_off 0 $'gates\nstate=success\nunlabel\nunassign'
GATES_OUT='merge-ready: check: missing or not successful on head head' GATES_CODE=1
ISSUE='{"labels":[{"name":"flag:qa-finding"}],"assignees":[{"login":"someone"}]}'
expect l79_another_reason_touches_no_label_or_assignee 0 $'gates\nstate=failure'

# L81: a draft or closed PR asks only why its Builder stopped.
draft='merge-ready: PR is closed or draft'
COMMENTS='[]' ISSUE='{"labels":[{"name":"flag:needs-user"}],"assignees":[{"login":"owner"}]}'
GATES_OUT="$draft"$'\n'"historical loop path prohibition" GATES_CODE=1
expect l81_a_draft_touching_loop_machinery_asks_nothing_and_clears_the_label 0 $'gates\nstate=failure\nunlabel\nunassign'
ISSUE='{"labels":[],"assignees":[]}'
GATES_OUT="$draft"$'\n'"historical second reject prohibition"
expect l81_a_closed_pr_asks_nothing 0 $'gates\nstate=failure'
GATES_OUT="$draft"$'\n'"the Builder stopped: needs a GLOSSARY term for Pin."$'\n'"historical loop path prohibition"
expect l81_a_stopped_draft_labels_assigns_and_asks 0 $'gates\nstate=failure\nlabel\nassign\ncomment'
body_is $'<!-- needs-user -->\nmerge-ready waits on you:\n\n- The Builder stopped (needs a GLOSSARY term for Pin): will you finish it, or close it and delete its branch to build it again?' l81_the_comment_asks_only_why_it_stopped
GATES_OUT="$draft"$'\n'"the Builder stopped: the run ended without marking it ready, https://example.test/run."
expect l88_interrupted_work_is_repaired_without_asking_the_user 0 $'gates\nstate=failure'
[ "$failures" -eq 0 ]
