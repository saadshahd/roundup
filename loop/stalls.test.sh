#!/usr/bin/env bash
set -euo pipefail
script=$(cd "$(dirname "$0")" && pwd)/stalls.sh
failures=0
head_sha=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
now=2026-10-03T12:00:00Z

fixture() {
  dir=$(mktemp -d)
  mkdir -p "$dir/loop" "$dir/bin" "$dir/data" "$dir/loop/out/verdicts"
  cp "$script" "$dir/loop/stalls.sh"
  cat >"$dir/bin/gh" <<'GH'
#!/usr/bin/env bash
set -euo pipefail
case "$*" in
  'repo view --json nameWithOwner') file=repo ;;
  *required_status_checks*) file=required ;;
  *commits/main/check-runs*) file=main-checks ;;
  *'pulls?state=open&per_page=100'*) file=pulls ;;
  *'/issues/1/comments'*) file=comments-1 ;;
  *'/pulls/1/commits'*) file=commits-1 ;;
  *'/commits/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/check-runs'*) file=pr-checks-1 ;;
  *) echo "unexpected gh $*" >&2; exit 9 ;;
esac
if [ -f "$DATA_DIR/gh-fail-$file" ]; then exit 9; fi
if [ -f "$DATA_DIR/gh-unprotected-$file" ]; then echo 'gh: Branch not protected (HTTP 404)' >&2; exit 1; fi
if [ -f "$DATA_DIR/gh-hang-$file" ]; then sleep 3; fi
cat "$DATA_DIR/$file.json"
GH
  cat >"$dir/bin/boxd" <<'BOXD'
#!/usr/bin/env bash
set -euo pipefail
[ "$*" = 'machine list --json' ] || exit 9
if [ -f "$DATA_DIR/boxd-fail" ]; then exit 9; fi
if [ -f "$DATA_DIR/boxd-hang" ]; then sleep 3; fi
cat "$DATA_DIR/machines.json"
BOXD
  chmod +x "$dir/bin/gh" "$dir/bin/boxd"
  printf '%s\n' '{"nameWithOwner":"o/r"}' >"$dir/data/repo.json"
  printf '%s\n' '{"contexts":["check"],"checks":[]}' >"$dir/data/required.json"
  printf '%s\n' '[{"check_runs":[{"name":"check","conclusion":"success","completed_at":"2026-10-03T11:59:00Z","id":1}]}]' >"$dir/data/main-checks.json"
  printf '%s\n' '[[{"number":1,"created_at":"2026-10-03T11:50:00Z","head":{"sha":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"user":{"login":"human"}}]]' >"$dir/data/pulls.json"
  printf '%s\n' '[[]]' >"$dir/data/comments-1.json"
  printf '%s\n' '[[{"commit":{"message":"work\n\nAuthor-Agent: builder-1"}}]]' >"$dir/data/commits-1.json"
  printf '%s\n' '[{"check_runs":[{"name":"check","conclusion":"success","completed_at":"2026-10-03T11:59:00Z","id":1}]}]' >"$dir/data/pr-checks-1.json"
  printf '%s\n' '[{"name":"ru-a"},{"name":"ru-b"},{"name":"ru-c"},{"name":"ru-d"},{"name":"ru-e"},{"name":"ru-f"},{"name":"ru-g"},{"name":"ru-h"}]' >"$dir/data/machines.json"
}
run() { (cd "$dir" && PATH="$dir/bin:$PATH" DATA_DIR="$dir/data" L28_NOW="$now" BOXD_GH_TIMEOUT=1 loop/stalls.sh check); }
expect() {
  local name=$1 want=$2 got=0 output
  output=$(run 2>&1) || got=$?
  if [ "$got" = "$want" ]; then echo "ok: l28_$name"; else echo "FAIL: l28_$name expected $want got $got: $output"; failures=$((failures+1)); fi
}
file() { cat "$dir/loop/out/stalls/$1"; }

fixture
expect no_condition 0
[ ! -d "$dir/loop/out/stalls" ] || [ -z "$(ls -A "$dir/loop/out/stalls")" ] || { echo 'FAIL: l28_no_condition wrote a file'; failures=$((failures+1)); }
fixture
touch "$dir/data/gh-unprotected-required"
printf '%s\n' '[{"check_runs":[{"name":"check","conclusion":"failure","completed_at":"2026-10-03T11:59:00Z","id":2}]}]' >"$dir/data/main-checks.json"
expect unprotected_main_still_requires_check 1
fixture
printf '%s\n' '[{"check_runs":[{"name":"check","conclusion":"failure","completed_at":"2026-10-03T11:59:00Z","id":2}]}]' >"$dir/data/main-checks.json"
expect main_required_check_failed 1
[ "$(file a-main | sed -n 's/^owner=//p')" = Triage ] || { echo 'FAIL: l28_main owner'; failures=$((failures+1)); }
printf '%s\n' '[{"check_runs":[{"name":"check","conclusion":"failure","completed_at":"2026-10-03T11:58:00Z","id":1},{"name":"check","conclusion":"success","completed_at":"2026-10-03T11:59:00Z","id":2}]}]' >"$dir/data/main-checks.json"
expect newest_main_run_recovers 0
[ ! -f "$dir/loop/out/stalls/a-main" ] || { echo 'FAIL: l28_newest main run'; failures=$((failures+1)); }
fixture
printf '%s\n' '[{"check_runs":[{"name":"check","conclusion":"failure","completed_at":"2026-10-03T11:59:00Z","id":2}]}]' >"$dir/data/pr-checks-1.json"
expect required_check_red_on_all_prs 1
[ -f "$dir/loop/out/stalls/a-all-prs" ] || { echo 'FAIL: l28_all-prs file'; failures=$((failures+1)); }
fixture
printf '%s\n' '[[{"body":"VERDICT: approve\naaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nReviewed-by-Agent: reviewer-1","created_at":"2026-10-03T11:45:00Z"}]]' >"$dir/data/comments-1.json"
expect approved_pr_open_20_minutes 0
python3 - "$dir/data/pulls.json" <<'PY'
import pathlib,sys
p=pathlib.Path(sys.argv[1]); p.write_text(p.read_text().replace('11:50:00','11:39:00'))
PY
expect approved_pr_stalled 1
[ "$(file b-1 | sed -n 's/^owner=//p')" = Merger ] || { echo 'FAIL: l28_approved owner'; failures=$((failures+1)); }
fixture
python3 - "$dir/data/pulls.json" <<'PY'
import pathlib,sys
p=pathlib.Path(sys.argv[1]); p.write_text(p.read_text().replace('11:50:00','11:39:00'))
PY
printf '%s\n' '[[{"body":"VERDICT: approve\naaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nReviewed-by-Agent: reviewer-1"},{"body":"VERDICT: reject\naaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nReviewed-by-Agent: reviewer-2"}]]' >"$dir/data/comments-1.json"
expect newer_reject_revokes_approval 0
fixture
printf '%s\n' '[[{"body":"VERDICT: reject\naaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nReviewed-by-Agent: r-1"},{"body":"VERDICT: reject\naaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nReviewed-by-Agent: r-2"},{"body":"VERDICT: reject\naaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nReviewed-by-Agent: r-3"}]]' >"$dir/data/comments-1.json"
expect third_reject 1
[ "$(file c-1 | sed -n 's/^except=//p')" = builder-1 ] || { echo 'FAIL: l28_third_reject except'; failures=$((failures+1)); }
fixture
printf '%s\n' '[[{"body":"VERDICT: reject\naaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nReviewed-by-Agent: builder-1"},{"body":"VERDICT: reject\naaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nReviewed-by-Agent: r-2"},{"body":"VERDICT: reject\naaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nReviewed-by-Agent: r-3"}]]' >"$dir/data/comments-1.json"
expect self_reject_does_not_count 0
fixture
other_sha=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
printf '%s\n' "[[{\"body\":\"VERDICT: reject\\n$other_sha\\nReviewed-by-Agent: r-1\"},{\"body\":\"VERDICT: reject\\n$other_sha\\nReviewed-by-Agent: r-2\"},{\"body\":\"VERDICT: reject\\n$other_sha\\nReviewed-by-Agent: r-3\"}]]" >"$dir/data/comments-1.json"
expect unrelated_sha_rejects_do_not_trip_round_cap 0
printf '%s\n' "[[{\"sha\":\"$other_sha\",\"commit\":{\"message\":\"work\\n\\nAuthor-Agent: builder-1\"}}]]" >"$dir/data/commits-1.json"
expect prior_pr_head_rejects_still_count 1
[ -f "$dir/loop/out/stalls/c-1" ] || { echo 'FAIL: l28_prior head rejects not counted'; failures=$((failures+1)); }
fixture
printf '%s\n' "[[{\"body\":\"VERDICT: reject\\nHead: $head_sha\\nDiff-base: $other_sha\\nReviewed-by-Agent: r-1\"},{\"body\":\"VERDICT: reject\\nHead: $head_sha\\nDiff-base: $other_sha\\nReviewed-by-Agent: r-2\"},{\"body\":\"VERDICT: reject\\nHead: $head_sha\\nDiff-base: $other_sha\\nReviewed-by-Agent: r-3\"}]]" >"$dir/data/comments-1.json"
expect rejects_with_diff_base_still_count 1
[ -f "$dir/loop/out/stalls/c-1" ] || { echo 'FAIL: l28_diff-base rejected verdicts lost'; failures=$((failures+1)); }
fixture
printf '%s\n' '[]' >"$dir/data/machines.json"
python3 - "$dir/data/pulls.json" <<'PY'
import pathlib,sys
p=pathlib.Path(sys.argv[1]); p.write_text(p.read_text().replace('11:50:00','11:39:00'))
PY
printf '%s\n' "[[{\"sha\":\"$other_sha\",\"commit\":{\"message\":\"work\\n\\nAuthor-Agent: builder-1\"}},{\"sha\":\"$head_sha\",\"parents\":[{\"sha\":\"$other_sha\"}],\"commit\":{\"message\":\"approve\\n\\nReviewed-by-Agent: reviewer-1\"}}]]" >"$dir/data/commits-1.json"
printf '%s\n' "[[{\"body\":\"VERDICT: approve\\nHead reviewed: $other_sha\\nReviewed-by-Agent: reviewer-1\\nDiff-base: cccccccccccccccccccccccccccccccccccccccc\"}]]" >"$dir/data/comments-1.json"
expect approval_commit_parent_is_reviewed_head 1
[ -f "$dir/loop/out/stalls/b-1" ] && [ ! -f "$dir/loop/out/stalls/e-1" ] || { echo 'FAIL: l28_approval commit reported wrong stall'; failures=$((failures+1)); }
printf '%s\n' '[{"name":"ru-a"},{"name":"ru-b"},{"name":"ru-c"},{"name":"ru-d"},{"name":"ru-e"},{"name":"ru-f"},{"name":"ru-g"},{"name":"ru-h"}]' >"$dir/data/machines.json"
printf '%s\n' "[[{\"body\":\"VERDICT: approve\\nHead reviewed: cccccccccccccccccccccccccccccccccccccccc\\nComparison: $head_sha\\nReviewed-by-Agent: reviewer-1\"}]]" >"$dir/data/comments-1.json"
expect reviewed_head_marker_ignores_comparison 0
[ ! -f "$dir/loop/out/stalls/b-1" ] || { echo 'FAIL: l28_comparison SHA counted as reviewed'; failures=$((failures+1)); }
fixture
printf '%s\n' '[]' >"$dir/data/machines.json"
printf '%s\n' '[[{"body":"VERDICT: approve\naaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nReviewed-by-Agent: builder-1"}]]' >"$dir/data/comments-1.json"
expect self_verdict_does_not_hide_vm_stall 1
[ -f "$dir/loop/out/stalls/e-1" ] || { echo 'FAIL: l28_self verdict hid e'; failures=$((failures+1)); }
fixture
printf '%s\n' '[[{"commit":{"message":"work\n\nAuthor-Agent: Build.Agent_1"}}]]' >"$dir/data/commits-1.json"
printf '%s\n' '[[{"body":"VERDICT: reject\naaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nReviewed-by-Agent: Review.One_1"},{"body":"VERDICT: reject\naaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nReviewed-by-Agent: Review.Two_2"},{"body":"VERDICT: reject\naaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nReviewed-by-Agent: Review.Three_3"}]]' >"$dir/data/comments-1.json"
expect dotted_underscored_uppercase_ids_count 1
[ "$(file c-1 | sed -n 's/^except=//p')" = Build.Agent_1 ] || { echo 'FAIL: l28_valid author id not parsed'; failures=$((failures+1)); }
fixture
printf '%s\n' "$head_sha" >"$dir/loop/out/verdicts/review.md"
python3 - "$dir/loop/out/verdicts/review.md" <<'PY'
import os,sys
os.utime(sys.argv[1], (1791028200,1791028200))
PY
expect verdict_file_without_comment 1
[ "$(file d-1 | sed -n 's/^owner=//p')" = Driver ] || { echo 'FAIL: l28_verdict owner'; failures=$((failures+1)); }
printf '%s\n' '[[{"body":"VERDICT: approve\naaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nReviewed-by-Agent: reviewer-1"}]]' >"$dir/data/comments-1.json"
expect verdict_comment_clears_stall 0
[ ! -f "$dir/loop/out/stalls/d-1" ] || { echo 'FAIL: l28_verdict comment did not clear'; failures=$((failures+1)); }
fixture
printf '%s\n' '[]' >"$dir/data/machines.json"
expect too_few_vms_for_unreviewed_pr 1
[ -f "$dir/loop/out/stalls/e-1" ] || { echo 'FAIL: l28_vm file'; failures=$((failures+1)); }
fixture
printf '%s\n' '[]' >"$dir/data/machines.json"
expect first_run_sets_since 1
first=$(file e-1 | sed -n 's/^since=//p')
now=2026-10-03T12:10:00Z
expect recheck_preserves_since 1
[ "$(file e-1 | sed -n 's/^since=//p')" = "$first" ] || { echo 'FAIL: l28_recheck since'; failures=$((failures+1)); }
printf '%s\n' '[{"name":"ru-a"},{"name":"ru-b"},{"name":"ru-c"},{"name":"ru-d"},{"name":"ru-e"},{"name":"ru-f"},{"name":"ru-g"},{"name":"ru-h"}]' >"$dir/data/machines.json"
expect recovery_deletes_file 0
[ ! -f "$dir/loop/out/stalls/e-1" ] || { echo 'FAIL: l28_recovery'; failures=$((failures+1)); }
fixture
printf '%s\n' '[]' >"$dir/data/machines.json"
expect initial_stall_for_tool_error 1
touch "$dir/data/gh-fail-required"
expect gh_failure_exit_4 4
[ -f "$dir/loop/out/stalls/e-1" ] || { echo 'FAIL: l28_gh failure mutated files'; failures=$((failures+1)); }
rm "$dir/data/gh-fail-required"
touch "$dir/data/boxd-hang"
expect boxd_timeout_exit_4 4
[ -f "$dir/loop/out/stalls/e-1" ] || { echo 'FAIL: l28_boxd timeout mutated files'; failures=$((failures+1)); }
rm "$dir/data/boxd-hang"
touch "$dir/data/gh-hang-pulls"
expect gh_timeout_exit_4 4
[ -f "$dir/loop/out/stalls/e-1" ] || { echo 'FAIL: l28_gh timeout mutated files'; failures=$((failures+1)); }

[ "$failures" -eq 0 ] || exit 1
