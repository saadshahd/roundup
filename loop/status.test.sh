#!/usr/bin/env bash
# Tests for loop/status.sh with a fake `gh`, `loop/runs.sh` and `loop/retro.sh`, and a real origin. Usage: loop/status.test.sh
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
mkdir -p "$dir/bin" "$dir/loop"
git init -q --bare "$dir/origin.git"
git -C "$dir" init -q
git -C "$dir" remote add origin "$dir/origin.git"
git -C "$dir" -c user.name=t -c user.email=t@t commit -q --allow-empty -m base
git -C "$dir" push -q origin HEAD:refs/heads/main HEAD:refs/heads/build/U5 HEAD:refs/heads/build/U7 HEAD:refs/heads/build/U8
cp "$root/loop/status.sh" "$root/loop/lib.sh" "$dir/loop/"
cat >"$dir/loop/runs.sh" <<'RUNS'
#!/usr/bin/env bash
set -euo pipefail
[ "$1" = builders ] || exit 2
[ ! -e "$FIXTURES/builders-fail" ] || exit 4
cat "$FIXTURES/builders"
RUNS
cat >"$dir/loop/retro.sh" <<'RETRO'
#!/usr/bin/env bash
set -euo pipefail
[ "$1" = cost ] || exit 2
[ ! -e "$FIXTURES/cost-fail" ] || exit 4
echo "$2" >"$FIXTURES/cost-since"
echo 1.9M
RETRO
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
  'issue list --state open --label loop:status --json number') cat "$FIXTURES/issues" ;;
  'pr list --state merged --search merged:>='*' --limit 1000 --json mergedAt') cat "$FIXTURES/merged" ;;
  'pr list --state open --limit 200 --json '*) cat "$FIXTURES/open" ;;
  'api repos/{owner}/{repo}/commits/'*'/status --jq '*) sha=${2#repos/\{owner\}/\{repo\}/commits/}; jq -r "$filter" "$FIXTURES/status-${sha%/status}" ;;
  'run list --workflow check.yml --branch main --event push --status completed --limit 1 '*) cat "$FIXTURES/main" ;;
  'run list --workflow '*'.yml --limit 100 '*) cat "$FIXTURES/runs-${4%.yml}" 2>/dev/null || echo '[]' ;;
  *) echo "unexpected gh call: $*" >&2; exit 1 ;;
esac
GH
chmod +x "$dir/bin/gh" "$dir/loop/runs.sh" "$dir/loop/retro.sh" "$dir/loop/status.sh"
LOOP_NOW=$(jq -n '"2026-10-09T12:00:00Z" | fromdate')
export PATH="$dir/bin:$PATH" FIXTURES="$dir/fixtures" LOOP_NOW
mkdir "$FIXTURES"
cd "$dir"
failures=0

# shellcheck source=loop/test-lib.sh
. "$root/loop/test-lib.sh"
row() { grep "^| $1 |" "$FIXTURES/out" | sed "s/^| $1 | //; s/ |\$//"; }
fresh() {
  rm -rf "${FIXTURES:?}"/*
  : >"$FIXTURES/trace"
  : >"$FIXTURES/builders"
  echo '[]' >"$FIXTURES/merged"
  echo '[]' >"$FIXTURES/open"
  echo '[{"conclusion":"success","headSha":"0123456789abcdef"}]' >"$FIXTURES/main"
}
# pr <number> <title> <updatedAt> [draft] [labels] [merge-ready state] [head]: one open PR, appended to the fixture.
pr() {
  jq -c --argjson n "$1" --arg title "$2" --arg at "$3" --argjson draft "${4:-false}" --arg labels "${5:-}" --arg state "${6:-FAILURE}" --arg head "${7:-build/x$1}" \
    '. + [{number: $n, title: $title, isDraft: $draft, updatedAt: $at, headRefName: $head, headRefOid: "sha\($n)", comments: [],
      labels: ($labels | split(" ") | map(select(. != "") | {name: .})),
      statusCheckRollup: ([{__typename: "CheckRun", name: "check"}] + if $state == "" then [] else [{__typename: "StatusContext", context: "merge-ready", state: $state}] end)}]' \
    "$FIXTURES/open" >"$FIXTURES/open.new"
  mv "$FIXTURES/open.new" "$FIXTURES/open"
}
comment() {
  jq -c --argjson n "$1" --arg login "$2" --arg body "$3" 'map(if .number == $n then .comments += [{author: {login: $login}, body: $body}] else . end)' \
    "$FIXTURES/open" >"$FIXTURES/open.new"
  mv "$FIXTURES/open.new" "$FIXTURES/open"
}

# L80 issue
fresh; echo '[{"number":42}]' >"$FIXTURES/issues"
check 'L80 issue finds the one issue labelled loop:status' 0 loop/status.sh issue
holds 'L80 issue prints its number' test "$(cat "$FIXTURES/out")" = 42
echo '[]' >"$FIXTURES/issues"
check 'L80 no Status issue exits 1' 1 loop/status.sh issue
holds 'L80 issue names the count' grep -q '0 open issues are labelled loop:status' "$FIXTURES/err"
echo '[{"number":42},{"number":43}]' >"$FIXTURES/issues"
check 'L80 two Status issues exit 1' 1 loop/status.sh issue
touch "$FIXTURES/gh-fail"
check 'L80 an issue gh failure exits 4' 4 loop/status.sh issue

# L80 page: an empty loop, every Claim held by a Builder
fresh; printf 'U5\nU7\nU8\n' >"$FIXTURES/builders"
check 'L80 page with nothing to report' 0 bash -c 'loop/status.sh page </dev/null'
holds 'L80 the body starts with its marker and the time' bash -c 'head -n2 "$FIXTURES/out" | tr "\n" " " | grep -q "^<!-- loop-status --> Loop status at 2026-10-09 12:00 UTC\."'
holds 'L80 an empty row reads nothing' bash -c 'for r in "Needs you" Blocked Watch; do grep -qx "| $r | nothing |" "$FIXTURES/out" || exit 1; done'
holds 'L80 with no earlier count the Tokens row says when it comes' grep -qx '| Tokens per merged product PR | counted at 08:03 UTC |' "$FIXTURES/out"
holds 'L80 without --cost the Ledger is not read' test ! -e "$FIXTURES/cost-since"

# L80 page: merged PRs and the token count
fresh
echo '[{"mergedAt":"2026-10-09T00:00:00Z"},{"mergedAt":"2026-10-09T11:00:00Z"},{"mergedAt":"2026-10-08T23:59:59Z"},{"mergedAt":"2026-10-02T12:00:00Z"},{"mergedAt":"2026-10-02T11:59:59Z"}]' >"$FIXTURES/merged"
check 'L80 page counts merged PRs' 0 bash -c 'printf "old\n| Tokens per merged product PR | 2.0M over 7 days, counted 2026-10-09 08:00 UTC |\n" | loop/status.sh page'
holds 'L80 merged counts since 00:00 UTC and over 7 days' test "$(row Merged)" = '2 today, 4 in 7 days'
holds 'L80 without --cost the page keeps the earlier Tokens row' test "$(row 'Tokens per merged product PR')" = '2.0M over 7 days, counted 2026-10-09 08:00 UTC'
check 'L80 page --cost counts the tokens' 0 loop/status.sh page --cost
holds 'L80 --cost asks retro.sh cost for the last 7 days' test "$(cat "$FIXTURES/cost-since")" = 2026-10-02T12:00:00Z
holds 'L80 --cost writes the count and when it was counted' test "$(row 'Tokens per merged product PR')" = '1.9M over 7 days, counted 2026-10-09 12:00 UTC'
touch "$FIXTURES/cost-fail"
check 'L80 a failed count exits 4' 4 loop/status.sh page --cost
check 'L80 page takes only --cost' 2 loop/status.sh page --all

# L80 page: PRs waiting on the user and PRs stuck
fresh
asks=$'<!-- needs-user -->\nmerge-ready waits on you:\n\n- It touches loop machinery: will you read it and merge it?\n- Two rejects stand: will you fix it, merge it or close it?'
pr 10 'L46 build files' 2026-10-09T11:00:00Z false flag:needs-user
comment 10 someone $'<!-- needs-user -->\n- forged'
comment 10 github-actions "$asks"
pr 11 'U9 labelled by hand' 2026-10-09T11:00:00Z false 'flag:needs-user flag:qa-finding'
pr 20 'U3 a | b' 2026-10-09T05:59:00Z
pr 21 'U4 fresh' 2026-10-09T06:01:00Z
pr 22 'U5 draft' 2026-10-08T00:00:00Z true
pr 23 'U6 waits on the user' 2026-10-08T00:00:00Z false flag:needs-user
pr 24 'U7 green' 2026-10-08T00:00:00Z false '' SUCCESS
pr 25 'U8 never gated' 2026-10-08T00:00:00Z false '' ''
echo '{"statuses":[{"context":"check","description":"x"},{"context":"merge-ready","description":"a Code PR needs an independent VERDICT: approve naming its head"}]}' >"$FIXTURES/status-sha20"
echo '{"statuses":[]}' >"$FIXTURES/status-sha25"
check 'L80 page reads open PRs' 0 bash -c 'loop/status.sh page </dev/null'
holds 'L80 needs you holds each flag:needs-user PR with the questions merge-ready asked' test "$(row 'Needs you')" = \
  '#10 L46 build files: It touches loop machinery: will you read it and merge it? Two rejects stand: will you fix it, merge it or close it?<br>#11 U9 labelled by hand: labelled flag:needs-user<br>#23 U6 waits on the user: labelled flag:needs-user'
holds 'L80 needs you ignores a needs-user comment from anyone but merge-ready' bash -c '! grep -q forged "$FIXTURES/out"'
holds 'L80 blocked holds each ready PR merge-ready fails that nothing changed for 6 hours' test "$(row Blocked)" = \
  '#20 U3 a \| b: a Code PR needs an independent VERDICT: approve naming its head, unchanged since 2026-10-09 05:59 UTC<br>#25 U8 never gated: no merge-ready status, unchanged since 2026-10-08 00:00 UTC'

# L80 page: what to watch
fresh
pr 30 'U5 building' 2026-10-09T11:00:00Z true '' FAILURE build/U5
echo U7 >"$FIXTURES/builders"
echo '[{"conclusion":"failure","headSha":"abcdef0123456789"}]' >"$FIXTURES/main"
echo '[{"workflowName":"build","createdAt":"2026-10-09T10:00:00Z","url":"u/1","conclusion":"failure"},{"workflowName":"build","createdAt":"2026-10-09T11:00:00Z","url":"u/2","conclusion":"timed_out"},{"workflowName":"build","createdAt":"2026-10-09T11:30:00Z","url":"u/3","conclusion":"cancelled"},{"workflowName":"build","createdAt":"2026-10-08T11:00:00Z","url":"u/4","conclusion":"failure"},{"workflowName":"build","createdAt":"2026-10-09T11:40:00Z","url":"u/5","conclusion":"success"}]' >"$FIXTURES/runs-build"
echo '[{"workflowName":"review","createdAt":"2026-10-09T09:00:00Z","url":"r/1","conclusion":"startup_failure"}]' >"$FIXTURES/runs-review"
check 'L80 page reads main, Claims and failed runs' 0 bash -c 'loop/status.sh page </dev/null'
holds 'L80 watch names a red main, a Claim with no PR or Builder, and failed agent runs of 24 hours' test "$(row Watch)" = \
  'main is red: check failure on abcdef0<br>build/U8: no open PR and no Builder; delete it to build its row again<br>build: 2 failed in 24 h, latest u/2<br>review: 1 failed in 24 h, latest r/1'
touch "$FIXTURES/builders-fail"
check 'L80 a failed Builder list exits 4, never a Claim with no Builder' 4 bash -c 'loop/status.sh page </dev/null'
rm "$FIXTURES/builders-fail"
git remote set-url origin "$dir/missing.git"
check 'L80 a failed ls-remote exits 4' 4 bash -c 'loop/status.sh page </dev/null'
git remote set-url origin "$dir/origin.git"
touch "$FIXTURES/gh-fail"
check 'L80 a page gh failure exits 4' 4 bash -c 'loop/status.sh page </dev/null'

[ "$failures" -eq 0 ] || { echo "$failures failed"; exit 1; }
