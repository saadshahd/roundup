#!/usr/bin/env bash
# The Status issue: one page of what merged, what it cost, what waits on the user and what is stuck (L80).
# Usage: loop/status.sh issue | page [--cost]
# Exit 4 is a gh or git failure, never read as "nothing to report".
set -euo pipefail
cd "$(dirname "$0")/.."

# shellcheck source=loop/lib.sh
. loop/lib.sh
now=${LOOP_NOW:-$(date -u +%s)}
at() { jq -nr --argjson t "$1" "\$t | strftime(\"$2\")"; }
stuck_hours=6

# L80: the Status issue's number: the one open issue labelled loop:status.
issue() {
  local found
  found=$(gh_or_4 issue list --state open --label loop:status --json number)
  [ "$(jq length <<<"$found")" -eq 1 ] || { echo "L80: $(jq length <<<"$found") open issues are labelled loop:status, not one" >&2; exit 1; }
  jq -r '.[0].number' <<<"$found"
}

# L80: the Status issue's body. Without --cost it keeps the Tokens row of the previous body (stdin), since counting
# downloads every Ledger artifact of the week.
page() {
  local day week stale cost tokens merged completed open needs blocked pr sha desc main claims running failed watch
  day=$(at "$now" '%Y-%m-%dT00:00:00Z')
  week=$(at $((now - 7 * 86400)) '%Y-%m-%dT%H:%M:%SZ')
  stale=$(at $((now - stuck_hours * 3600)) '%Y-%m-%dT%H:%M:%SZ')
  if [ "${1:-}" = --cost ]; then
    cost=$(loop/retro.sh cost "$week")
    tokens="| Tokens per merged product PR | $cost over 7 days, counted $(at "$now" '%Y-%m-%d %H:%M') UTC |"
  else
    tokens=$(grep -m1 '^| Tokens per merged product PR |' || echo '| Tokens per merged product PR | counted at 08:03 UTC |')
  fi
  merged=$(gh_or_4 pr list --state merged --search "merged:>=$week" --limit 1000 --json mergedAt |
    jq -r --arg day "$day" --arg week "$week" 'map(.mergedAt) | "\(map(select(. >= $day)) | length) today, \(map(select(. >= $week)) | length) in 7 days"')
  open=$(gh_or_4 pr list --state open --limit 200 --json number,title,isDraft,labels,headRefName,headRefOid,updatedAt,statusCheckRollup,comments)

  # Each PR labelled flag:needs-user, with the questions of merge-ready's needs-user comment (L79).
  needs=$(jq -c '[.[] | select(any(.labels[]; .name == "flag:needs-user"))
    | ([.comments[] | select(.author.login == "github-actions" and (.body | startswith("<!-- needs-user -->")))] | last | .body // ""
       | split("\n") | map(select(startswith("- ")) | ltrimstr("- "))) as $asks
    | "#\(.number) \(.title): " + (if $asks == [] then "labelled flag:needs-user" else $asks | join(" ") end)]' <<<"$open")

  # Each ready PR merge-ready fails that nothing changed for stuck_hours, not waiting on the user.
  blocked=$(jq -r --arg stale "$stale" '.[] | select((.isDraft | not) and (any(.labels[]; .name == "flag:needs-user") | not) and .updatedAt < $stale
    and ([.statusCheckRollup[] | select(.context == "merge-ready")] | first | .state) != "SUCCESS") | "\(.number) \(.headRefOid)"' <<<"$open" |
    while read -r pr sha; do
      desc=$(gh_or_4 api "repos/{owner}/{repo}/commits/$sha/status" --jq '[.statuses[] | select(.context == "merge-ready")] | first | .description // "no merge-ready status"')
      jq -nc --argjson pr "$pr" --arg desc "$desc" --argjson open "$open" '$open[] | select(.number == $pr)
        | "#\(.number) \(.title): \($desc), unchanged since \(.updatedAt[:16] | sub("T"; " ")) UTC"'
    done | jq -s -c .)

  main=$(gh_or_4 run list --workflow check.yml --branch main --event push --status completed --limit 1 --json conclusion,headSha)
  # An unfinished Claim with no open PR and no Builder blocks its row (L23).
  claims=$(git ls-remote --heads origin 'build/*') || { echo "git ls-remote origin failed" >&2; exit 4; }
  completed=$(merged_branches)
  running=$(loop/runs.sh builders)
  # L87: the local service publishes a bounded Claim record, never an immortal running marker.
  running+=$'\n'$(jq -nr --argjson now "$now" --arg status "${CODEX_STATUS:-}" '
    ($status | fromjson? // {}) | select(.state == "running" and .deadline > $now) | .slug // empty')
  claims=$(jq -nc --arg claims "$claims" --arg running "$running" --arg completed "$completed" --argjson open "$open" '
    (($open | map(.headRefName)) + ($completed | split("\n"))) as $heads | ($running | split("\n")) as $running
    | [$claims | scan("refs/heads/(build/\\S+)")[0] | select(. as $b | $heads | index($b) | not) | select(ltrimstr("build/") as $s | $running | index($s) | not)
       | "\(.): no open PR and no Builder; delete it to build its row again"]')
  failed=$(for workflow in build review retro; do
    gh_or_4 run list --workflow "$workflow.yml" --limit 100 --json workflowName,createdAt,url,conclusion
  done | jq -s -c --arg since "$(at $((now - 86400)) '%Y-%m-%dT%H:%M:%SZ')" '
    add | map(select(.createdAt >= $since and (.conclusion | IN("failure", "timed_out", "startup_failure")))) | group_by(.workflowName)
    | map("\(.[0].workflowName): \(length) failed in 24 h, latest \(max_by(.createdAt).url)")')
  watch=$(jq -nc --argjson main "$main" --argjson claims "$claims" --argjson failed "$failed" '
    [$main[] | select(.conclusion != "success") | "main is red: check \(.conclusion) on \(.headSha[:7])"] + $claims + $failed')

  jq -nr --arg at "$(at "$now" '%Y-%m-%d %H:%M')" --arg merged "$merged" --arg tokens "$tokens" \
    --argjson activity "${ACTIVITY:-[]}" --arg codex "${CODEX_STATUS:-}" --argjson needs "$needs" --argjson blocked "$blocked" --argjson watch "$watch" '
    def cell: if length == 0 then "nothing" else map(gsub("\\|"; "\\|")) | join("<br>") end;
    "<!-- loop-status -->",
    "Loop status at \($at) UTC. This body updates on run events and every five minutes; at 08:03 UTC the same table lands as a comment.", "",
    "| Row | Now |", "|---|---|",
    "| Merged | \($merged) |", $tokens,
    "| Running / queued | \($activity | cell) |",
    (if $codex == "" then empty else "| Codex | " + ([$codex | fromjson |
      "\(.ids): \(.state); started \(.started | strftime("%Y-%m-%d %H:%M UTC")); " +
      (if .state == "running" then "deadline " + (.deadline | strftime("%H:%M UTC"))
       else "\(.usage.input // 0) input + \(.usage.cache_read // 0) cached + \(.usage.output // 0) output tokens" end)] | cell) + " |" end),
    "| Needs you | \($needs | cell) |", "| Blocked | \($blocked | cell) |", "| Watch | \($watch | cell) |"'
}

case "${1:-}" in
  issue) issue ;;
  page) [[ ${2:-} =~ ^(--cost)?$ ]] || { sed -n '3p' "$0" >&2; exit 2; }; page "${2:-}" ;;
  *) sed -n '3p' "$0" >&2; exit 2 ;;
esac
