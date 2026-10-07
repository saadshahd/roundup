#!/usr/bin/env bash
# The steps between GitHub's events and one-shot agent runs; each workflow step calls one subcommand.
# Usage: loop/runs.sh queue [max] | slug <subject> | prompt <role> | review-due <pr> <head> | verdict <pr> <head> <reviewer-id> | swept <sha>
# Exit 4 is a gh failure, never read as "nothing to do".
set -euo pipefail
cd "$(dirname "$0")/.."

gh_or_4() { gh "$@" || { echo "gh $1 $2 failed" >&2; exit 4; }; }

# A subject (a Work row's ids or a PR number) as one word for a branch or an artifact name.
slug_def='def slug: gsub("[^A-Za-z0-9]+"; "-") | ltrimstr("-") | rtrimstr("-");'

# L23: up to <max> ready Work rows of `loop/rules.sh ready` as the build matrix [{ids, file, slug}]. It skips loop
# machinery (scenarios/loop-*.md: the user's or a Retro's) and a row whose branch build/<slug> exists with no open PR:
# its Builder pushed and stopped, and a new run would pay for the same row again.
queue() {
  local max=${1:-4} rows branches
  rows=$(loop/rules.sh ready)
  branches=$(git ls-remote --heads origin 'build/*') || { echo "git ls-remote origin failed" >&2; exit 4; }
  printf '%s\n' "$rows" | jq -R -s -c --argjson max "$max" --arg branches "$branches" "$slug_def"'
    ($branches | [scan("refs/heads/build/(\\S+)")[0]]) as $taken
    | [split("\n")[] | capture("^ready (?<ids>.+) (?<file>scenarios/[^ ]+)$")
       | select(.file | startswith("scenarios/loop-") | not)
       | .slug = (.ids | slug)
       | select(.slug as $s | $taken | any(. == $s) | not)]
    | .[:$max]'
}

# Sets the step output `prompt`: `.agents/<role>.md` byte for byte, then the task read from stdin, so every run of a
# role shares the longest prefix a prompt cache can reuse.
prompt() {
  local role=$1 delimiter
  [ -f ".agents/$role.md" ] || { echo "no .agents/$role.md" >&2; exit 2; }
  : "${GITHUB_OUTPUT:?prompt writes a step output}"
  delimiter="PROMPT_$(od -An -N8 -tx1 /dev/urandom | tr -d ' \n')"
  {
    echo "prompt<<$delimiter"
    cat ".agents/$role.md"
    printf '\n## Task\n\n'
    cat
    echo "$delimiter"
  } >>"$GITHUB_OUTPUT"
}

# L24: prints the Reviewer's task and exits 0 when <head> of a Code PR needs a verdict; prints why not and exits 1.
review_due() {
  local pr=$1 head=$2 view paths touched verdicts
  view=$(gh_or_4 pr view "$pr" --json state,headRefOid,body)
  [ "$(jq -r '"\(.state) \(.headRefOid)"' <<<"$view")" = "OPEN $head" ] || { echo "PR #$pr is not open at $head"; return 1; }
  paths=$(gh_or_4 pr diff "$pr" --name-only)
  touched=$(printf '%s\n' "$paths" | loop/rules.sh touches)
  grep -qx code <<<"$touched" || { echo "PR #$pr is not a Code PR"; return 1; }
  verdicts=$(loop/rules.sh verdicts "$pr") || exit 4
  [ "$(jq -s 'map(select(.verdict == "reject")) | length' <<<"$verdicts")" -lt 2 ] || { echo "second reject: the user decides"; return 1; }
  ! jq -se 'any(.current)' <<<"$verdicts" >/dev/null || { echo "a verdict already covers $head"; return 1; }
  printf 'PR #%s, head %s.\n' "$pr" "$head"
  jq -r '.body // ""' <<<"$view" | { grep -E '^(Scenarios|Moves|macOS):' || true; }
  if grep -qx ui <<<"$touched"; then echo 'It touches apps/desktop/src: read its Percy build with percy-review.'; fi
  jq -sr 'if length == 0 then empty else "\nEarlier verdicts, oldest first:\n\n" + (map(.body) | join("\n\n---\n\n")) end' <<<"$verdicts"
}

# L24: posts the Reviewer's structured output (stdin: {verdict, findings}) as the trusted comment merge-ready reads,
# then dispatches merge-ready, and on a first reject one fix run. Lines that would forge a field are dropped.
verdict() {
  local pr=$1 head=$2 reviewer=$3 out verdict findings rejects
  out=$(cat)
  verdict=$(jq -r '.verdict // empty' <<<"$out" 2>/dev/null) || true
  case $verdict in approve | reject) ;; *) echo "L24: the Reviewer gave no verdict: $out" >&2; exit 1 ;; esac
  findings=$(jq -r '.findings // ""' <<<"$out" | { grep -vE '^(VERDICT|Head|Reviewed-by-Agent):' || true; })
  rejects=$(loop/rules.sh verdicts "$pr" | jq -s 'map(select(.verdict == "reject")) | length') || exit 4
  printf 'VERDICT: %s\nHead: %s\n\n%s\n\nReviewed-by-Agent: %s\n' "$verdict" "$head" "$findings" "$reviewer" |
    gh_or_4 pr comment "$pr" --body-file -
  gh_or_4 workflow run merge-ready.yml -f pr="$pr"
  if [ "$verdict" = reject ] && [ "$rejects" -eq 0 ]; then gh_or_4 workflow run build.yml -f pr="$pr"; fi
  echo "$verdict"
}

# L66: exits 0 when a completed qa run already swept <sha>, so a tick sweeps main only after it moved.
swept() {
  local runs
  runs=$(gh_or_4 run list --workflow qa.yml --status completed --limit 20 --json headSha,conclusion)
  jq -e --arg sha "$1" 'any(.[]; .headSha == $sha and (.conclusion == "success" or .conclusion == "failure"))' <<<"$runs" >/dev/null
}

case "${1:-}" in
  queue) queue "${2:-4}" ;;
  slug) [ -n "${2:-}" ] || { sed -n '2p' "$0" >&2; exit 2; }; jq -rn --arg s "$2" "$slug_def"' $s | slug' ;;
  prompt) [ -n "${2:-}" ] || { sed -n '2p' "$0" >&2; exit 2; }; prompt "$2" ;;
  review-due) [[ ${2:-} =~ ^[0-9]+$ && ${3:-} =~ ^[0-9a-f]{40}$ ]] || { sed -n '2p' "$0" >&2; exit 2; }; review_due "$2" "$3" ;;
  verdict) [[ ${2:-} =~ ^[0-9]+$ && ${3:-} =~ ^[0-9a-f]{40}$ && ${4:-} =~ ^[A-Za-z0-9_.-]+$ ]] || { sed -n '2p' "$0" >&2; exit 2; }; verdict "$2" "$3" "$4" ;;
  swept) [[ ${2:-} =~ ^[0-9a-f]{40}$ ]] || { sed -n '2p' "$0" >&2; exit 2; }; swept "$2" ;;
  *) sed -n '2p' "$0" >&2; exit 2 ;;
esac
