#!/usr/bin/env bash
# Tests for loop/retro.sh with a fake `gh`, `curl` and `loop/rules.sh` (its `touches` is the real one). Usage: loop/retro.test.sh
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
mkdir -p "$dir/bin" "$dir/loop"
cp "$root/loop/retro.sh" "$root/loop/runs.sh" "$root/loop/lib.sh" "$root/loop/outcome.py" "$root/loop/dispatch.py" "$dir/loop/"
# The Trail reader has its own tests (loop/trail.test.py); here it answers from a fixture.
printf 'import os, sys\nif __name__ == "__main__":\n    sys.exit(4 if os.path.exists(os.environ["FIXTURES"] + "/trail-fail") else print(open(os.environ["FIXTURES"] + "/repeats").read()))\n' >"$dir/loop/trail.py"
git -C "$dir" init -q
git -C "$dir" add loop
GIT_COMMITTER_DATE='2026-10-08T09:00:00+02:00' git -C "$dir" -c user.name=t -c user.email=t@t commit -q -m retro
cat >"$dir/loop/rules.sh" <<'RULES'
#!/usr/bin/env bash
set -euo pipefail
case $1 in
  touches) exec bash "$ROOT/loop/rules.sh" touches ;;
  verdicts) [ ! -e "$FIXTURES/verdicts-$2.code" ] || { cat "$FIXTURES/verdicts-$2" >&2; exit "$(cat "$FIXTURES/verdicts-$2.code")"; }
    cat "$FIXTURES/verdicts-$2" ;;
  *) exit 2 ;;
esac
RULES
# Answers by the arguments it is called with, applying --jq as gh does.
cat >"$dir/bin/gh" <<'GH'
#!/usr/bin/env bash
set -euo pipefail
echo "gh $*" >>"$FIXTURES/trace"
[ ! -e "$FIXTURES/gh-fail" ] || exit 1
filter=.
args=("$@")
for i in "${!args[@]}"; do [ "${args[$i]}" != --jq ] || filter=${args[$((i + 1))]}; done
case "$*" in
  'pr list --state merged --limit 1000 --json headRefName,mergedAt '*) jq -r "$filter" "$FIXTURES/merged" ;;
  'pr list --state merged --search merged:>='*) cat "$FIXTURES/merged" ;;
  'pr list --state open --json headRefName '*) jq -r "$filter" "$FIXTURES/open" ;;
  'api repos/{owner}/{repo}/actions/artifacts?per_page=100 --paginate '*) jq -r "$filter" "$FIXTURES/artifacts" ;;
  'run download '*) mkdir -p "$7"; cp "$FIXTURES/$5" "$7/ledger.json" ;;
  *) echo "unexpected gh call: $*" >&2; exit 1 ;;
esac
GH
cat >"$dir/bin/curl" <<'CURL'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$@" >"$FIXTURES/curl-args"
[ ! -e "$FIXTURES/curl-fail" ] || { echo '{"error":"rate"}'; exit 22; }
while [ $# -gt 0 ]; do [ "$1" != -d ] || printf '%s\n' "$2" >>"$FIXTURES/jev-requests"; shift; done
cat "$FIXTURES/jev"
CURL
chmod +x "$dir/bin/gh" "$dir/bin/curl" "$dir/loop/rules.sh" "$dir/loop/retro.sh"
export PATH="$dir/bin:$PATH" ROOT="$root" FIXTURES="$dir/fixtures" TYPESAFE_API_KEY=k GITHUB_RUN_ID=77
mkdir "$FIXTURES"
cd "$dir"
failures=0

# shellcheck source=loop/test-lib.sh
. "$root/loop/test-lib.sh"
fresh() {
  rm -rf "${FIXTURES:?}"/*
  : >"$FIXTURES/trace"
  printf '[]\n' >"$FIXTURES/open"
  printf '{"answers":{"kind":{"choice":"real-defect","confidence":0.9},"class":{"choice":"behaviour","confidence":0.8}}}\n' >"$FIXTURES/jev"
}
# merged <n> [retro-at]: n merged PRs after 2026-10-08T10:00:00Z, and a merged Retro PR at retro-at.
merged() {
  jq -n --argjson n "$1" --arg retro "${2:-}" '
    [range($n) | {number: (. + 1), title: "PR \(. + 1)", headRefName: "build/x\(.)", mergedAt: "2026-10-09T00:00:\(10 + .)Z", files: [{path: "docs/a.md"}]}]
    + if $retro == "" then [] else [{number: 900, title: "Retro", headRefName: "retro/2026-10-08", mergedAt: $retro, files: [{path: "loop/a.sh"}]}] end' >"$FIXTURES/merged"
}

# L29 ledger
fresh
printf '[{"type":"system"},{"type":"assistant","message":{"id":"m1","usage":{"input_tokens":5}}},{"type":"result","subtype":"success","num_turns":3,"modelUsage":{"claude-sonnet-5-5":{},"claude-haiku-4-5":{}},"usage":{"input_tokens":10,"output_tokens":20,"cache_creation_input_tokens":30,"cache_read_input_tokens":40}}]\n' >"$FIXTURES/run.json"
check 'L29 a finished run is one row' 0 loop/retro.sh ledger builder 'U105–U107' "$FIXTURES/run.json" success
holds 'L29 turns and tokens come from the result entry' test "$(cat "$FIXTURES/out")" = '{"role":"builder","subject":"U105–U107","run":"77","model":"claude-haiku-4-5,claude-sonnet-5-5","turns":3,"exit":"success","input":10,"output":20,"cache_write":30,"cache_read":40,"usage":"recorded"}'
printf '[{"type":"assistant","message":{"id":"m1","usage":{"input_tokens":5,"output_tokens":1,"cache_read_input_tokens":100}}},{"type":"assistant","message":{"id":"m1","usage":{"input_tokens":5,"output_tokens":1,"cache_read_input_tokens":100}}},{"type":"user"},{"type":"assistant","message":{"id":"m2","model":"claude-opus-5-5","usage":{"input_tokens":7,"output_tokens":2,"cache_creation_input_tokens":9}}}]\n' >"$FIXTURES/cut.json"
check 'L29 a run cut off before its result is one row' 0 loop/retro.sh ledger reviewer 12 "$FIXTURES/cut.json" failure
holds 'L29 each assistant message counts once' test "$(cat "$FIXTURES/out")" = '{"role":"reviewer","subject":"12","run":"77","model":"claude-opus-5-5","turns":2,"exit":"failure","input":12,"output":3,"cache_write":9,"cache_read":100,"usage":"partial"}'
check 'L29 missing execution output is a row' 0 loop/retro.sh ledger builder U3 '' failure
holds 'L29 it reports missing output and unavailable usage' test "$(cat "$FIXTURES/out")" = '{"role":"builder","subject":"U3","run":"77","model":"","turns":0,"exit":"missing-output","input":0,"output":0,"cache_write":0,"cache_read":0,"usage":"unavailable"}'
printf '[{"type":"result","usage":{"input_tokens":0,"output_tokens":0,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}]\n' >"$FIXTURES/zero.json"
check 'L29 actual zero usage remains recorded' 0 loop/retro.sh ledger builder U3 "$FIXTURES/zero.json" success
holds 'L29 zero is different from missing usage' jq -e '.usage == "recorded" and .input == 0 and .output == 0' "$FIXTURES/out"
printf '[{"type":"result","usage":{"input_tokens":15}}]\n' >"$FIXTURES/partial.json"
check 'L29 partial result usage remains partial' 0 loop/retro.sh ledger builder U3 "$FIXTURES/partial.json" success
holds 'L29 missing fields are not called complete usage' jq -e '.usage == "partial" and .input == 15' "$FIXTURES/out"
check 'L90 a feedback observer can write a Ledger row' 0 loop/retro.sh ledger feedback 355 "$FIXTURES/run.json" success
holds 'L90 the Ledger preserves the feedback role' jq -e '.role == "feedback"' "$FIXTURES/out"
check 'L29 an unknown role fails before reading' 2 loop/retro.sh ledger driver U3 '' failure

# Exercise the real spend command; the feedback tests' API fake cannot establish this integration.
now=$(date -u +%Y-%m-%dT%H:%M:%SZ)
jq -n --arg now "$now" '{artifacts:[{name:"ledger-77-1-feedback-355", expired:false, created_at:$now, workflow_run:{id:77}}]}' >"$FIXTURES/artifacts"
loop/retro.sh ledger feedback 355 "$FIXTURES/run.json" success >"$FIXTURES/ledger-77-1-feedback-355"
check 'L90 feedback can read its actual spend command' 0 loop/retro.sh spent
holds 'L83 spend is weighted and rounded upward' test "$(cat "$FIXTURES/out")" = 152
: >"$FIXTURES/trace"
check 'L83 cached spend reads the Ledger' 0 env LOOP_LEDGER_CACHE="$dir/cache" loop/retro.sh spent
check 'L83 a second scan reuses immutable downloads' 0 env LOOP_LEDGER_CACHE="$dir/cache" loop/retro.sh spent
holds 'L83 repeat scans download each artifact once' test "$(grep -c 'gh run download' "$FIXTURES/trace")" = 1
printf '{"usage":"unavailable"}\n' >"$FIXTURES/ledger-77-1-feedback-355"
check 'L83 missing usage fails instead of inventing zero spend' 4 loop/retro.sh spent
printf '{"usage":"partial","input":5,"output":0,"cache_write":0,"cache_read":0}\n' >"$FIXTURES/ledger-77-1-feedback-355"
check 'L83 partial usage preserves its known floor' 0 loop/retro.sh spent
holds 'L83 the partial floor remains visible' test "$(cat "$FIXTURES/out")" = 5
holds 'L83 partial fields are never called full coverage' grep -q 'coverage is partial' "$FIXTURES/err"

# Execute the action's actual extraction step against a main ref, with no checkout files in its output directory.
git update-ref refs/remotes/origin/main HEAD
mkdir "$dir/runner"
sed -n '/^      run: |$/,/^    - uses:/p' "$root/.github/actions/ledger/action.yml" |
  sed '1d;$d;s/^        //' >"$dir/ledger-step.sh"
check 'L29 the action extracts every dependency needed for a Ledger row' 0 env \
  GITHUB_ACTION_PATH="$dir" RUNNER_TEMP="$dir/runner" GITHUB_OUTPUT="$dir/outputs" ROLE=builder SUBJECT=A23 \
  EXECUTION_FILE="$FIXTURES/run.json" CONCLUSION=success bash -eo pipefail "$dir/ledger-step.sh"
holds 'L29 the extracted action writes its row and artifact slug' bash -c \
  'jq -e '\''.subject == "A23" and .exit == "success" and .turns == 3'\'' "$1/runner/ledger/ledger.json" && grep -qx "slug=A23" "$1/outputs"' _ "$dir"

holds 'L92 the action outputs an explicit explanation without raw events and keeps no outcome.json' bash -c \
  'sed -n "s/^explanation=//p" "$1/outputs" | jq -e '\''.kind == "model-claim" and (has("events") | not)'\'' && test ! -e "$1/runner/ledger/outcome.json"' _ "$dir"

# An old or modified PR checkout cannot select the extraction scripts.
mkdir -p "$dir/pr/loop"
git -C "$dir/pr" init -q
printf 'exit 99\n' >"$dir/pr/loop/retro.sh"
git -C "$dir/pr" add loop
git -C "$dir/pr" -c user.name=t -c user.email=t@t commit -q -m untrusted
pushd "$dir/pr" >/dev/null
check 'L29 the action reads its trusted checkout instead of PR HEAD' 0 env \
  GITHUB_ACTION_PATH="$dir" RUNNER_TEMP="$dir/runner" GITHUB_OUTPUT="$dir/outputs" ROLE=builder SUBJECT=trusted \
  EXECUTION_FILE="$FIXTURES/run.json" CONCLUSION=success bash -eo pipefail "$dir/ledger-step.sh"
popd >/dev/null
holds 'L29 the trusted checkout produced the artifact' jq -e '.subject == "trusted" and .exit == "success"' "$dir/runner/ledger/ledger.json"

# L27 due
fresh; merged 20 2026-10-08T23:00:00Z
check 'L27 twenty PRs since the last Retro are due' 0 loop/retro.sh due
holds 'L27 due names the count and the start' grep -qx '20 PRs merged since 2026-10-08T23:00:00Z' "$FIXTURES/out"
fresh; merged 20 2026-10-09T00:00:15Z
check 'L27 PRs merged before the last Retro do not count' 1 loop/retro.sh due
holds 'L27 not due names the count' grep -qx '14 of 20 PRs merged since 2026-10-09T00:00:15Z' "$FIXTURES/out"
fresh; merged 20
check 'L27 with no Retro merged, the count starts when retro.sh was added' 0 loop/retro.sh due
holds 'L27 the start is the commit time in UTC' grep -qx '20 PRs merged since 2026-10-08T07:00:00Z' "$FIXTURES/out"
fresh; merged 25 2026-10-08T23:00:00Z; printf '[{"headRefName":"retro/2026-10-09"}]\n' >"$FIXTURES/open"
check 'L27 an open Retro PR is not due' 1 loop/retro.sh due
holds 'L27 the open Retro PR is named' grep -qx 'a Retro PR is open' "$FIXTURES/out"
fresh; merged 20; touch "$FIXTURES/gh-fail"
check 'L27 a gh failure exits 4' 4 loop/retro.sh due

# L27 classify
fresh
check 'L27 classify asks Jev two Choices' 0 bash -c 'echo "{\"id\":\"#1 t\",\"text\":\"rule 2: dead code\"}" | loop/retro.sh classify'
holds 'L27 classify prints each choice with its confidence' test "$(cat "$FIXTURES/out")" = '{"id":"#1 t","kind":"real-defect","kind_confidence":0.9,"class":"behaviour","class_confidence":0.8}'
holds 'L27 the request pins jev-1.13.0 and sends the finding as state' test "$(jq -r '"\(.model) \(.state)"' "$FIXTURES/jev-requests")" = 'jev-1.13.0 rule 2: dead code'
holds 'L27 the request offers every kind and class' test "$(jq -c '[.questions.kind.criteria, .questions.class.criteria | keys]' "$FIXTURES/jev-requests")" = '[["process-rule","real-defect","wording"],["behaviour","contract","process","scope","security","slop","test","visual","vocabulary"]]'
holds 'L27 the key goes in the Authorization header' grep -qx 'Authorization: Bearer k' "$FIXTURES/curl-args"
fresh; touch "$FIXTURES/curl-fail"
check 'L27 a Jev failure exits 4' 4 bash -c 'echo "{\"id\":\"#1 t\",\"text\":\"x\"}" | loop/retro.sh classify'
fresh; printf '{}\n' >"$FIXTURES/jev"
check 'L27 a Jev answer without choices exits 4' 4 bash -c 'echo "{\"id\":\"#1 t\",\"text\":\"x\"}" | loop/retro.sh classify'
fresh
check 'L27 classify without a key fails before calling' 2 env -u TYPESAFE_API_KEY bash -c 'echo "{}" | loop/retro.sh classify'

# L27 report
fresh
jq -n '[{number: 1, title: "U3 thing", headRefName: "build/U3", mergedAt: "2026-10-09T01:00:00Z", files: [{path: "crates/x/a.rs"}]},
  {number: 2, title: "loop", headRefName: "retro/x", mergedAt: "2026-10-08T23:00:00Z", files: [{path: "loop/a.sh"}]},
  {number: 3, title: "spec", headRefName: "s", mergedAt: "2026-10-09T02:00:00Z", files: [{path: "scenarios/a.md"}]},
  {number: 4, title: "U4", headRefName: "build/U4", mergedAt: "2026-10-09T03:00:00Z", files: [{path: "apps/desktop/src/a.tsx"}]},
  {number: 5, title: "loop 2", headRefName: "l", mergedAt: "2026-10-09T04:00:00Z", files: [{path: ".agents/builder.md"}]}]' >"$FIXTURES/merged"
printf '{"artifacts":[{"name":"ledger-50-builder-U3","expired":false,"created_at":"2026-10-09T00:30:00Z","workflow_run":{"id":50}},{"name":"ledger-51-reviewer-1","expired":false,"created_at":"2026-10-09T00:40:00Z","workflow_run":{"id":51}},{"name":"ledger-40-builder-U1","expired":false,"created_at":"2026-10-08T20:00:00Z","workflow_run":{"id":40}},{"name":"percy","expired":false,"created_at":"2026-10-09T00:40:00Z","workflow_run":{"id":52}}]}\n' >"$FIXTURES/artifacts"
printf '{"role":"builder","subject":"U3","run":"50","model":"claude-sonnet-5-5","turns":40,"exit":"success","input":1000000,"output":200000,"cache_write":400000,"cache_read":10000000}\n' >"$FIXTURES/ledger-50-builder-U3"
printf '{"role":"reviewer","subject":"1","run":"51","model":"claude-opus-5-5","turns":9,"exit":"error_max_turns","input":100000,"output":20000,"cache_write":0,"cache_read":1000000}\n' >"$FIXTURES/ledger-51-reviewer-1"
printf '{"verdict":"reject","at":"2026-10-09T00:45:00Z","body":"VERDICT: reject\\nrule 2: dead code"}\n{"verdict":"approve","at":"2026-10-09T00:55:00Z","body":"VERDICT: approve"}\n' >"$FIXTURES/verdicts-1"
printf 'merge-ready: no Author-Agent trailers\n' >"$FIXTURES/verdicts-4"; echo 1 >"$FIXTURES/verdicts-4.code"
echo '[{"issue":372,"title":"B1 to B18","count":2,"url":"https://example/c1"}]' >"$FIXTURES/repeats"
check 'L27 report reads the Ledger, PRs and rejects' 0 loop/retro.sh report
holds 'L92 report lists each Trail that ended the same way twice' grep -qx -- '- #372 B1 to B18: 2 runs ended the same way with no new head, \[last entry\](https://example/c1)' "$FIXTURES/out"
holds 'L27 report counts PRs merged since the last Retro' grep -qx '# Retro: 4 PRs merged since 2026-10-08T23:00:00Z' "$FIXTURES/out"
holds 'L27 report counts each kind' bash -c 'grep -qx "| product | 2 |" "$FIXTURES/out" && grep -qx "| loop | 1 |" "$FIXTURES/out" && grep -qx "| spec | 1 |" "$FIXTURES/out"'
holds 'L27 report sums the Ledger by role and model, weighted' bash -c 'grep -qx "| builder | claude-sonnet-5-5 | 1 | 0 | 40 | 3.5M |" "$FIXTURES/out" && grep -qx "| reviewer | claude-opus-5-5 | 1 | 1 | 9 | 300K |" "$FIXTURES/out"'
holds 'L27 report leaves out runs before the last Retro' bash -c '! grep -q "U1" "$FIXTURES/out"'
holds 'L27 report divides by merged product PRs' grep -qx 'Weighted tokens per merged product PR: ≥1.9M recorded/product PR; 2 product PRs, 2/2 usage records; partial coverage' "$FIXTURES/out"
holds 'L27 report classifies each reject, not each approve' bash -c 'grep -qx "| real-defect | behaviour | 90%, 80% | #1 2026-10-09T00:45:00Z |" "$FIXTURES/out" && test "$(wc -l <"$FIXTURES/jev-requests")" -eq 1'
holds 'L27 report names a PR whose verdicts it could not read' grep -qx 'Verdicts unread on PR #4: merge-ready: no Author-Agent trailers' "$FIXTURES/out"
holds 'L27 report lists the costliest run first' bash -c 'grep -A4 "^## Costliest runs" "$FIXTURES/out" | tail -n1 | grep -qx "| 50 | builder | U3 | 3.5M | success |"'

touch "$FIXTURES/trail-fail"
check 'L92 an unreadable Trail makes the report exit 4' 4 loop/retro.sh report
rm "$FIXTURES/trail-fail"

# L80 cost, on the report's PRs and Ledger
check 'L80 cost reads merged PRs and the Ledger after since' 0 loop/retro.sh cost 2026-10-08T23:00:00Z
holds 'L80 cost is the weighted tokens per merged product PR' test "$(cat "$FIXTURES/out")" = '≥1.9M recorded/product PR; 2 product PRs, 2/2 usage records; partial coverage'
check 'L80 cost after the last merge' 0 loop/retro.sh cost 2026-10-09T05:00:00Z
holds 'L80 cost with no product PR merged says so' test "$(cat "$FIXTURES/out")" = 'unavailable: no recorded usage (0 product PRs)'
check 'L80 cost without a UTC time fails before reading' 2 loop/retro.sh cost yesterday
touch "$FIXTURES/gh-fail"
check 'L80 a cost gh failure exits 4' 4 loop/retro.sh cost 2026-10-08T23:00:00Z
rm "$FIXTURES/gh-fail"

# L80 missing and small usage must not look free.
cp "$FIXTURES/artifacts" "$FIXTURES/artifacts.saved"
printf '{"artifacts":[]}\n' >"$FIXTURES/artifacts"
check 'L80 missing Ledger is unavailable even with delivered product PRs' 0 loop/retro.sh cost 2026-10-08T23:00:00Z
holds 'L80 absent evidence is never zero' grep -qx 'unavailable: no recorded usage (2 product PRs)' "$FIXTURES/out"
cp "$FIXTURES/artifacts.saved" "$FIXTURES/artifacts"
printf '{"role":"builder","subject":"U3","run":"50","model":"m","turns":1,"exit":"success","input":10,"output":10,"cache_write":0,"cache_read":0,"usage":"recorded"}\n' >"$FIXTURES/ledger-50-builder-U3"
printf '{"role":"reviewer","subject":"1","run":"51","model":"","turns":0,"exit":"missing-output","input":0,"output":0,"cache_write":0,"cache_read":0,"usage":"unavailable"}\n' >"$FIXTURES/ledger-51-reviewer-1"
check 'L80 small nonzero usage and missing records are visible' 0 loop/retro.sh cost 2026-10-08T23:00:00Z
holds 'L80 reports known usage and its coverage' grep -qx '≥30 recorded/product PR; 2 product PRs, 1/2 usage records; partial coverage' "$FIXTURES/out"

echo 4 >"$FIXTURES/verdicts-4.code"
check 'L27 a verdicts gh failure exits 4' 4 loop/retro.sh report
echo 1 >"$FIXTURES/verdicts-4.code"; touch "$FIXTURES/curl-fail"
check 'L27 a Jev failure fails the report' 4 loop/retro.sh report

[ "$failures" -eq 0 ] || { echo "$failures failed"; exit 1; }
