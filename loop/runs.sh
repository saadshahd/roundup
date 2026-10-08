#!/usr/bin/env bash
# The steps between GitHub's events and one-shot agent runs; each workflow step calls one subcommand.
# Usage: loop/runs.sh queue [max] | claim | unclaim | recover | stopped <branch>... | builders | slug <subject> | prompt <role> | review-due <pr> <head> | verdict <pr> <head> <reviewer-id> | swept <sha>
# Exit 4 is a gh failure, never read as "nothing to do".
set -euo pipefail
cd "$(dirname "$0")/.."

# shellcheck source=loop/lib.sh
. loop/lib.sh

# A subject (a Work row's ids or a PR number) as one word for a branch or an artifact name.
slug_def='def slug: gsub("[^A-Za-z0-9]+"; "-") | ltrimstr("-") | rtrimstr("-");'

# L23: the slug of each Builder job (`build (<ids>, <file>, <slug>)`) still queued or running in any build.yml run, one
# per line; they hold slots of max across ticks.
builders() {
  local ids id
  ids=$(gh_or_4 run list --workflow build.yml --limit 50 --json databaseId,status --jq '.[] | select(.status != "completed") | .databaseId') || exit 4
  for id in $ids; do
    gh_or_4 run view "$id" --json jobs --jq '.jobs[] | select((.name | startswith("build (")) and .status != "completed" and (.name | endswith(" / after") | not)) | .name | sub(" / build$"; "") | capture(", (?<slug>[^ ,]+)\\)$").slug' || exit 4
  done
}

# L23: eligible GitHub Issues, ordered by their priority, with room for at most <max> Builders.
# Existing branch/owner Claims retain their identity across migration. Reads only.
queue() {
  local max=${1:-4} rows branches merged running
  rows=$(loop/rules.sh ready --json)
  branches=$(git ls-remote origin 'refs/heads/build/*' 'refs/heads/loop-row/*') || { echo "git ls-remote origin failed" >&2; exit 4; }
  merged=$(merged_heads)
  running=$(builders | wc -l | tr -d ' ')
  printf '%s\n' "$rows" | jq -c --argjson max "$((max > running ? max - running : 0))" --arg branches "$branches" --arg merged "$merged" "$slug_def"'
    ($branches | [scan("refs/heads/loop-row/(\\S+)")[0]]) as $owned
    | ($merged | split("\n")) as $merged
    | ($branches | split("\n") | map(select(length > 0) | . as $line
        | capture("^(?<sha>[^\\s]+)\\s+refs/heads/build/(?<slug>\\S+)$")
        | . + {merged: ($merged | index($line) != null)})) as $claims
    | (($claims | map(select(.merged | not) | .slug)) + $owned) as $taken
    | ($claims | map(select(.merged) | {key: .slug, value: .sha}) | from_entries) as $reusable
    | [ .[] | select(.state == "ready")
       | select(.slug as $s | $taken | any(. == $s) | not)
       | if $reusable[.slug] then . + {reclaim: $reusable[.slug]} else . end]
    | sort_by(.priority // 100)
    | map(del(.state, .reason, .priority))
    | .[:$max]'
}

# L23: pushes each queued row's Claim (stdin: the queue), build/<slug> at HEAD, the main commit its rows were read
# from, and prints the rows it claimed. Only a new branch is a Claim: one that exists, which git reports as up to date
# or as a lease miss (stale info), belongs to another run, so its row is skipped. Any other result exits 4, a remote
# rejection included, so a push the token may not make never reads as an empty queue.
claim() {
  local sha claimed owner
  sha=$(git rev-parse HEAD)
  claimed=$(jq -c '.[]' | while IFS= read -r row; do
    slug=$(jq -r .slug <<<"$row")
    reclaim=$(jq -r '.reclaim // empty' <<<"$row")
    if [ -n "$reclaim" ]; then
      # Only retire the exact merged head observed by queue. A competing new Claim wins.
      retired=$(git push --porcelain --force-with-lease="refs/heads/build/$slug:$reclaim" origin ":refs/heads/build/$slug" 2>&1) || true
      if ! grep -q '^-' <<<"$retired"; then
        if grep -qE '^!.*\[rejected\] \(stale info\)' <<<"$retired"; then continue; fi
        echo "retiring build/$slug failed: $retired" >&2
        exit 4
      fi
    fi
    owner=$(jq -nc --arg slug "$slug" --arg head "$sha" --arg run "${GITHUB_RUN_ID:-}" --arg nonce "$(od -An -N16 -tx1 /dev/urandom)" '{slug:$slug,claim_head:$head,run:$run,nonce:$nonce}' |
      git -c user.name=roundup -c user.email=loop@roundup.invalid commit-tree "$(git rev-parse HEAD^{tree})")
    pushed=$(git push --atomic --porcelain --force-with-lease="refs/heads/build/$slug:" --force-with-lease="refs/heads/loop-row/$slug:" origin "$sha:refs/heads/build/$slug" "$owner:refs/heads/loop-row/$slug" 2>&1) || true
    if awk -F '\t' -v ref="refs/heads/build/$slug" '$1 == "*" && substr($2, index($2, ":") + 1) == ref {won=1} END {exit !won}' <<<"$pushed"; then
      jq -c --arg sha "$sha" --arg owner "$owner" 'del(.reclaim) + {claim_head: $sha, claim_owner: $owner}' <<<"$row"
    elif grep -q '^\*' <<<"$pushed"; then
      # Git treats an existing branch at the same SHA as up-to-date even with an empty lease.
      # Retire only our new owner ref; the pre-existing branch belongs to another run.
      git push --porcelain --force-with-lease="refs/heads/loop-row/$slug:$owner" origin ":refs/heads/loop-row/$slug" >/dev/null || exit 4
    elif ! grep -qE '^(=|!.*\[rejected\] \((stale info|atomic push failed)\))' <<<"$pushed"; then
      echo "git push of build/$slug failed: $pushed" >&2
      exit 4
    fi
  done)
  jq -sc . <<<"$claimed"
}

# L23: after a tick's Builders end, however they end, deletes the Claim of each row (stdin: the claimed queue) whose
# branch has no open PR, so the next tick can build an unfinished row. Historical PRs do not hold a new Claim.
unclaim() {
  local rows row slug expected owner refs current prs retired
  local -a leases updates
  rows=$(jq -c '.[]')
  while IFS= read -r row; do
    slug=$(jq -r .slug <<<"$row")
    expected=$(jq -r '.claim_head // empty' <<<"$row")
    owner=$(jq -r '.claim_owner // empty' <<<"$row")
    # Older runs cannot prove ownership. They leave recovery to a current run.
    [ -n "$expected" ] && [ -n "$owner" ] || continue
    refs=$(git ls-remote origin "refs/heads/loop-row/$slug" "refs/heads/build/$slug") || exit 4
    current=$(awk -v ref="refs/heads/loop-row/$slug" '$2 == ref {print $1}' <<<"$refs")
    [ "$current" = "$owner" ] || continue
    prs=$(gh_or_4 pr list --head "build/$slug" --state open --json number --jq length)
    leases=("--force-with-lease=refs/heads/loop-row/$slug:$owner")
    updates=(":refs/heads/loop-row/$slug")
    current=$(awk -v ref="refs/heads/build/$slug" '$2 == ref {print $1}' <<<"$refs")
    if [ "$prs" = 0 ] && [ "$current" = "$expected" ]; then
      leases+=("--force-with-lease=refs/heads/build/$slug:$expected")
      updates+=(":refs/heads/build/$slug")
    fi
    retired=$(git push --atomic --porcelain "${leases[@]}" origin "${updates[@]}" 2>&1) || true
    if grep -q '^-' <<<"$retired"; then
      [ "${#updates[@]}" = 1 ] || echo "freed build/$slug"
    elif ! grep -qE '^!.*\[rejected\] \((stale info|atomic push failed)\)' <<<"$retired"; then
      echo "git push --delete build/$slug failed: $retired" >&2
      exit 4
    fi
  done <<<"$rows"
}

# Recover a cloud Builder whose model job ended before its completion job released ownership.
# Local owners are recovered by the local controller, which can observe its process.
recover() {
  local refs owner ref slug record run terminal
  refs=$(git ls-remote --heads origin 'loop-row/*') || exit 4
  while read -r owner ref; do
    [ -n "$owner" ] || continue
    slug=${ref#refs/heads/loop-row/}
    record=$(gh_or_4 api "repos/{owner}/{repo}/git/commits/$owner" --jq .message)
    record=$(jq -ce --arg slug "$slug" --arg owner "$owner" 'select(.slug == $slug and (.claim_head | test("^[0-9a-f]{40}$")) and (.run | type == "string")) + {claim_owner:$owner}' <<<"$record") || { echo "invalid Claim owner $ref" >&2; exit 4; }
    run=$(jq -r .run <<<"$record")
    [ -n "$run" ] || continue
    terminal=$(gh_or_4 run view "$run" --json status,jobs --jq ".status == \"completed\" or ([.jobs[] | select(.name | endswith(\", $slug) / build\") or endswith(\", $slug)\"))] | length > 0 and all(.status == \"completed\"))")
    [ "$terminal" = true ] || continue
    jq -sc . <<<"$record" | unclaim
  done <<<"$refs"
}

# L81: after a Builder or fix run ends, however it ends: gives the open draft PR of each <branch> that has no `Stopped:`
# line one naming the run, as a run cut off by its time or turns leaves none, and dispatches merge-ready, which asks the
# user (L79). A branch with no open PR, or a ready one, is left alone.
stopped() {
  local branch view pr body run=$GITHUB_SERVER_URL/$GITHUB_REPOSITORY/actions/runs/$GITHUB_RUN_ID
  for branch in "$@"; do
    view=$(gh_or_4 pr list --head "$branch" --state open --json number,isDraft,body --jq '.[0] // empty')
    [ -n "$view" ] || continue
    pr=$(jq -r .number <<<"$view")
    body=$(jq -r '.body // ""' <<<"$view")
    if [ "$(jq -r .isDraft <<<"$view")" != true ] || grep -q '^Stopped: *[^[:space:]]' <<<"$body"; then continue; fi
    gh_or_4 api -X PATCH "repos/{owner}/{repo}/pulls/$pr" -f body="$body"$'\n\n'"Stopped: the run ended without marking it ready, $run." >/dev/null
    gh_or_4 workflow run merge-ready.yml -f pr="$pr"
    echo "stopped #$pr"
  done
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

# L24: prints the Reviewer's task and exits 0 when <head> of a ready Code PR needs a verdict; prints why not and exits 1.
# A draft is skipped: its Builder is still pushing, and `gh pr ready` reruns check on the head it finished with.
review_due() {
  local pr=$1 head=$2 view paths touched verdicts code build
  view=$(gh_or_4 pr view "$pr" --json state,headRefOid,body,isDraft)
  [ "$(jq -r '"\(.state) \(.headRefOid)"' <<<"$view")" = "OPEN $head" ] || { echo "PR #$pr is not open at $head"; return 1; }
  [ "$(jq -r .isDraft <<<"$view")" = false ] || { echo "PR #$pr is a draft"; return 1; }
  paths=$(gh_or_4 pr diff "$pr" --name-only)
  touched=$(printf '%s\n' "$paths" | loop/rules.sh touches)
  grep -Eq '^(code|loop)$' <<<"$touched" || { echo "PR #$pr is not a Code PR"; return 1; }
  verdicts=$(loop/rules.sh verdicts "$pr") || exit 4
  ! jq -se 'any(.current)' <<<"$verdicts" >/dev/null || { echo "a verdict already covers $head"; return 1; }
  printf 'PR #%s, head %s.\n' "$pr" "$head"
  jq -r '.body // ""' <<<"$view" | { grep -E '^(Scenarios|Moves|macOS):' || true; }
  if grep -qx ui <<<"$touched"; then
    code=0
    build=$(loop/percy.sh build "$head") || code=$?
    case $code in
      0) printf 'It touches apps/desktop/src: run percy-review on Percy build %s, made for this head; with a build id it needs no .percy/config.yml. Each diff must match a then-clause of the scenarios above or a D check the Moves: line names; nothing the author wrote counts as intent (rule 5).\n' "$build" ;;
      1) echo 'It touches apps/desktop/src, but no Percy build exists for this head: review the code alone and say so in a NOTE.' ;;
      *) exit 4 ;;
    esac
  fi
  jq -sr 'if length == 0 then empty else "\nEarlier verdicts, oldest first:\n\n" + (map(.body) | join("\n\n---\n\n")) end' <<<"$verdicts"
}

# L24: posts the Reviewer's structured output (stdin: {verdict, findings}) as the trusted comment merge-ready reads,
# then dispatches merge-ready, and on a first reject one fix run. Lines that would forge a field are dropped.
verdict() {
  local pr=$1 head=$2 reviewer=$3 out verdict findings rejects body view
  out=$(cat)
  verdict=$(jq -r '.verdict // empty' <<<"$out" 2>/dev/null) || true
  case $verdict in approve | reject) ;; *) echo "L24: the Reviewer gave no verdict: $out" >&2; exit 1 ;; esac
  view=$(gh_or_4 pr view "$pr" --json state,headRefOid,body,isDraft)
  [ "$(jq -r '"\(.state) \(.headRefOid)"' <<<"$view")" = "OPEN $head" ] || { echo 'stale review: no verdict posted'; return 0; }
  findings=$(jq -r '.findings // ""' <<<"$out" | { grep -vE '^(VERDICT|Head|Reviewed-by-Agent):' || true; })
  rejects=$(loop/rules.sh verdicts "$pr" | jq -s 'map(select(.verdict == "reject")) | length') || exit 4
  printf 'VERDICT: %s\nHead: %s\n\n%s\n\nReviewed-by-Agent: %s\n' "$verdict" "$head" "$findings" "$reviewer" |
    gh_or_4 pr comment "$pr" --body-file -
  gh_or_4 workflow run merge-ready.yml -f pr="$pr"
  if [ "$verdict" = reject ] && [ "$rejects" -eq 0 ]; then
    # L81: the fix run's `gh pr ready --undo` makes the PR a draft again; an old `Stopped:` line would ask the user.
    body=$(gh_or_4 pr view "$pr" --json body --jq '.body // ""')
    if grep -q '^Stopped:' <<<"$body"; then
      gh_or_4 api -X PATCH "repos/{owner}/{repo}/pulls/$pr" -f body="$(grep -v '^Stopped:' <<<"$body")" >/dev/null
    fi
    gh_or_4 workflow run build.yml -f pr="$pr"
  fi
  echo "$verdict"
}

# L66: exits 0 when a qa run already swept <sha> to the end, which its step `a complete sweep record` passing proves,
# so a tick sweeps main only after it moved. A run that failed before or during the sweep does not count.
swept() {
  local ids id complete
  ids=$(gh_or_4 run list --workflow qa.yml --status completed --limit 20 --json databaseId,headSha --jq ".[] | select(.headSha == \"$1\") | .databaseId")
  for id in $ids; do
    complete=$(gh_or_4 run view "$id" --json jobs --jq '[.jobs[].steps[]? | select(.name == "a complete sweep record" and .conclusion == "success")] | length > 0') || exit 4
    [ "$complete" != true ] || return 0
  done
  return 1
}

case "${1:-}" in
  queue) [[ ${2:-4} =~ ^[0-9]+$ ]] || { sed -n '2p' "$0" >&2; exit 2; }; queue "${2:-4}" ;;
  claim) claim ;;
  unclaim) unclaim ;;
  recover) recover ;;
  stopped) shift; stopped "$@" ;;
  builders) builders ;;
  slug) [ -n "${2:-}" ] || { sed -n '2p' "$0" >&2; exit 2; }; jq -rn --arg s "$2" "$slug_def"' $s | slug' ;;
  prompt) [ -n "${2:-}" ] || { sed -n '2p' "$0" >&2; exit 2; }; prompt "$2" ;;
  review-due) [[ ${2:-} =~ ^[0-9]+$ && ${3:-} =~ ^[0-9a-f]{40}$ ]] || { sed -n '2p' "$0" >&2; exit 2; }; review_due "$2" "$3" ;;
  verdict) [[ ${2:-} =~ ^[0-9]+$ && ${3:-} =~ ^[0-9a-f]{40}$ && ${4:-} =~ ^[A-Za-z0-9_.-]+$ ]] || { sed -n '2p' "$0" >&2; exit 2; }; verdict "$2" "$3" "$4" ;;
  swept) [[ ${2:-} =~ ^[0-9a-f]{40}$ ]] || { sed -n '2p' "$0" >&2; exit 2; }; swept "$2" ;;
  *) sed -n '2p' "$0" >&2; exit 2 ;;
esac
