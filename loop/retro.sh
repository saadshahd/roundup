#!/usr/bin/env bash
# What a Retro reads: the Ledger, merged PRs and their rejects, each reject classified by Jev.
# Usage: loop/retro.sh ledger <role> <subject> <execution-file> <conclusion> | due | report | classify | cost <since>
# Exit 4 is a gh, git or Jev failure, never read as "nothing to do".
set -euo pipefail
cd "$(dirname "$0")/.."

# shellcheck source=loop/lib.sh
. loop/lib.sh
every=20
jev_model=jev-1.13.0
# jq: a Ledger row's weighted tokens, a count in millions, and weighted tokens per merged product PR of $prs (from
# `kinds`) and $rows (Ledger rows). Weights: input 1, cache write 1.25, cache read 0.1, output 5.
# shellcheck disable=SC2016 # jq variables, not the shell's
weights='def weighted: .input + 1.25 * .cache_write + 0.1 * .cache_read + 5 * .output;
  def m: if . >= 1000000 then (. / 100000 | round / 10 | tostring) + "M"
    elif . >= 1000 then (. / 100 | round / 10 | tostring) + "K"
    else (. * 10 | round / 10 | tostring) end;
  def recorded: (.usage // (if (.model // "") != "" then "legacy" else "unavailable" end)) != "unavailable";
  def per_product($prs; $rows): ($prs | map(select(.kind == "product")) | length) as $n
    | ($rows | map(select(recorded))) as $known
    | if ($known | length) == 0 then "unavailable: no recorded usage (\($n) product PRs)"
      else ($known | map(weighted) | add) as $total
        | (if $n > 0 then "≥\($total / $n | m) recorded/product PR" else "\($total | m) recorded; no product PR merged" end)
          + "; \($n) product PRs, \($known | length)/\($rows | length) usage records; partial coverage"
      end;'

# L29: one Ledger row for an agent run. Its models, turns and tokens come from the run's `result` entry; a run cut off before it
# has one sums its assistant messages, each message id once, since a message repeats per content block.
ledger() {
  local role=$1 subject=$2 file=$3 conclusion=$4
  { if [ -n "$file" ] && [ -s "$file" ]; then cat "$file"; else echo '[]'; fi; } |
    jq -c --arg role "$role" --arg subject "$subject" --arg run "${GITHUB_RUN_ID:-local}" --arg conclusion "$conclusion" '
    def tokens: {input: (.input_tokens // 0), output: (.output_tokens // 0),
                 cache_write: (.cache_creation_input_tokens // 0), cache_read: (.cache_read_input_tokens // 0)};
    def total: {input: (map(.input) | add // 0), output: (map(.output) | add // 0),
                cache_write: (map(.cache_write) | add // 0), cache_read: (map(.cache_read) | add // 0)};
    (map(select(.type == "result")) | last) as $result
    | (map(select(.type == "assistant" and .message.id)) | unique_by(.message.id)) as $said
    | {role: $role, subject: $subject, run: $run,
       model: ((if $result then ($result.modelUsage // {} | keys) else ($said | map(.message.model // empty) | unique) end) | join(","))}
    + if $result then {turns: ($result.num_turns // 0), exit: ($result.subtype // $conclusion)} + ($result.usage // {} | tokens)
      else {turns: ($said | length), exit: (if length == 0 then "missing-output" else $conclusion end)} + ($said | map(.message.usage // {} | tokens) | total)
      end
    | . + {usage: (if $result then [$result.usage // {}] else $said | map(.message.usage // {}) end
        | map([.input_tokens, .output_tokens, .cache_creation_input_tokens, .cache_read_input_tokens]) | flatten
        | if length == 0 or all(. == null) then "unavailable"
          elif all(type == "number") then "recorded" else "partial" end)}'
}

# The time the last Retro PR merged, else the time this script was added: the Ledger starts with it.
since() {
  local merged
  merged=$(gh_or_4 pr list --state merged --limit 1000 --json headRefName,mergedAt \
    --jq '[.[] | select(.headRefName | startswith("retro/")) | .mergedAt] | max // empty') || exit 4
  [ -n "$merged" ] || merged=$(TZ=UTC git log --diff-filter=A -1 --format=%cd --date=format-local:%Y-%m-%dT%H:%M:%SZ \
    -- loop/retro.sh) || { echo "git log failed" >&2; exit 4; }
  [ -n "$merged" ] || { echo "no Retro PR merged and loop/retro.sh is not committed" >&2; exit 4; }
  echo "$merged"
}

# Merged PRs after <since> as [{number, title, mergedAt, files}].
merged_since() {
  gh_or_4 pr list --state merged --search "merged:>=$1" --limit 1000 --json number,title,mergedAt,files |
    jq -c --arg since "$1" '[.[] | select(.mergedAt > $since) | {number, title, mergedAt, files: [.files[].path]}]'
}

# Merged PRs after <since> as [{number, title, kind}]: product touches code, loop touches loop machinery, the rest spec.
kinds() {
  merged_since "$1" | jq -c '.[]' | while IFS= read -r pr; do
    jq -r '.files[]' <<<"$pr" | loop/rules.sh touches |
      jq -R -s -c --argjson pr "$pr" '($pr | {number, title}) + {kind: (split("\n") | if index("code") then "product" elif index("loop") then "loop" else "spec" end)}'
  done | jq -s -c .
}

# L27: exits 0 when <every> PRs merged since the last Retro and no Retro PR is open; prints why either way.
due() {
  local start count open
  start=$(since)
  count=$(merged_since "$start" | jq length)
  open=$(gh_or_4 pr list --state open --json headRefName --jq '[.[] | select(.headRefName | startswith("retro/"))] | length')
  [ "$open" -eq 0 ] || { echo "a Retro PR is open"; return 1; }
  [ "$count" -ge "$every" ] || { echo "$count of $every PRs merged since $start"; return 1; }
  echo "$count PRs merged since $start"
}

# L27: reads {id, text} lines; prints {id, kind, class} with Jev's confidence for each. Options go in a fresh random
# order per call, since Jev leans to the first one.
classify() {
  [ -n "${TYPESAFE_API_KEY:-}" ] || { echo "classify calls Jev: set TYPESAFE_API_KEY" >&2; exit 2; }
  local line kinds classes body answer
  kinds='real-defect	the change breaks behaviour, a test, a rule or a scenario clause
wording	only naming, prose or format; the change works
process-rule	only loop paperwork: the PR body, trailers, a branch or a label'
  classes='behaviour	wrong or missing behaviour against the scenario
test	a missing, weak or misnamed test
slop	dead code, duplication or an unused abstraction
vocabulary	a name outside GLOSSARY.md or an Avoid word
scope	files or changes beyond the scenario
contract	a contract, a caller or a generated file out of step
visual	a design-system Check, a Token or a Percy diff
security	a secret, a token, an identity or a permission
process	PR body, trailers, branch or other loop paperwork'
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    body=$(jq -nc --argjson item "$line" --arg model "$jev_model" \
      --arg kinds "$(sort -R <<<"$kinds")" --arg classes "$(sort -R <<<"$classes")" '
      def options: split("\n") | map(split("\t") | {key: .[0], value: .[1]}) | from_entries;
      {model: $model, state: $item.text, questions: {
        kind: {type: "choice", instructions: "Why did the reviewer reject this pull request?", criteria: ($kinds | options)},
        class: {type: "choice", instructions: "Which failure class fits the main finding?", criteria: ($classes | options)}}}')
    answer=$(curl -sS --fail-with-body --max-time 30 --retry 2 https://api.typesafe.ai/v1/systemone \
      -H "Authorization: Bearer $TYPESAFE_API_KEY" -H 'Content-Type: application/json' -d "$body") ||
      { echo "Jev failed: $answer" >&2; exit 4; }
    jq -ce --argjson item "$line" '.answers | select(.kind.choice and .class.choice) | {id: $item.id,
      kind: .kind.choice, kind_confidence: .kind.confidence, class: .class.choice, class_confidence: .class.confidence}' <<<"$answer" ||
      { echo "Jev answered without choices: $answer" >&2; exit 4; }
  done
}

# Ledger rows of runs after <since>, one per line, from the ledger-* artifacts.
ledger_rows() {
  local dir run name
  dir=$(mktemp -d)
  gh_or_4 api 'repos/{owner}/{repo}/actions/artifacts?per_page=100' --paginate \
    --jq ".artifacts[] | select((.name | startswith(\"ledger-\")) and .expired == false and .created_at > \"$1\") | \"\(.workflow_run.id) \(.name)\"" >"$dir/list"
  while read -r run name; do
    gh_or_4 run download "$run" --name "$name" --dir "$dir/$name" >/dev/null
    cat "$dir/$name/ledger.json" || exit 4
  done <"$dir/list"
  rm -rf "$dir"
}

# L80: weighted tokens per merged product PR since <since>, as the Status issue shows them.
cost() {
  local prs rows
  prs=$(kinds "$1")
  rows=$(ledger_rows "$1" | jq -s -c .)
  jq -nr --argjson prs "$prs" --argjson rows "$rows" "$weights"' per_product($prs; $rows)'
}

# L27: the Retro's input as markdown: PRs by kind, Ledger totals by role, tokens per merged product PR, each reject
# classified, and the costliest runs. Runs group by role and model, so a Retro can weigh Sonnet against Opus.
report() {
  local start pr kinds rows rejects unread dir code
  start=$(since)
  kinds=$(kinds "$start")
  rows=$(ledger_rows "$start" | jq -s -c .)
  dir=$(mktemp -d)
  : >"$dir/rejects"
  : >"$dir/unread"
  jq -r '.[] | select(.kind == "product") | .number' <<<"$kinds" >"$dir/product"
  while read -r pr; do
    code=0
    loop/rules.sh verdicts "$pr" >"$dir/verdicts" 2>"$dir/error" || code=$?
    case $code in
      0) jq -c --arg pr "$pr" 'select(.verdict == "reject") | {id: "#\($pr) \(.at)", text: .body}' "$dir/verdicts" >>"$dir/rejects" ;;
      1) jq -nc --arg pr "$pr" --rawfile error "$dir/error" '{pr: $pr, error: $error}' >>"$dir/unread" ;;
      *) cat "$dir/error" >&2; exit 4 ;;
    esac
  done <"$dir/product"
  rejects=$(classify <"$dir/rejects" | jq -s -c .) || exit 4
  unread=$(jq -s -c . "$dir/unread")
  rm -rf "$dir"
  jq -nr --arg since "$start" --argjson prs "$kinds" --argjson rows "$rows" --argjson rejects "$rejects" --argjson unread "$unread" "$weights"'
    "# Retro: \($prs | length) PRs merged since \($since)", "",
      "| Kind | PRs |", "|---|---|",
      ($prs | group_by(.kind)[] | "| \(.[0].kind) | \(length) |"), "",
      "| Role | Model | Runs | Not success | Turns | Weighted tokens |", "|---|---|---|---|---|---|",
      ($rows | group_by([.role, .model])[] | "| \(.[0].role) | \(.[0].model) | \(length) | \(map(select(.exit != "success")) | length) | \(map(.turns) | add) | \(map(weighted) | add | m) |"), "",
      "Weighted tokens per merged product PR: \(per_product($prs; $rows))", "",
      "## Rejects", "",
      "| Kind | Class | Confidence | Verdict |", "|---|---|---|---|",
      ($rejects[] | "| \(.kind) | \(.class) | \(.kind_confidence * 100 | round)%, \(.class_confidence * 100 | round)% | \(.id) |"), "",
      ($unread[] | "Verdicts unread on PR #\(.pr): \(.error | split("\n")[0])"), "",
      "## Costliest runs", "",
      "| Run | Role | Subject | Weighted tokens | Exit |", "|---|---|---|---|---|",
      ($rows | sort_by(weighted) | reverse | .[:5][] | "| \(.run) | \(.role) | \(.subject) | \(weighted | m) | \(.exit) |")'
}

case "${1:-}" in
  ledger) [ $# -eq 5 ] && [[ $2 =~ ^(builder|reviewer|retro)$ ]] || { sed -n '3p' "$0" >&2; exit 2; }; ledger "$2" "$3" "$4" "$5" ;;
  due) due ;;
  report) report ;;
  classify) classify ;;
  cost) [[ ${2:-} =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9:]{8}Z$ ]] || { sed -n '3p' "$0" >&2; exit 2; }; cost "$2" ;;
  *) sed -n '3p' "$0" >&2; exit 2 ;;
esac
