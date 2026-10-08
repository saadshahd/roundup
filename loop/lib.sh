# shellcheck shell=bash
# Sourced by the loop scripts from the repository root.

# A gh call whose failure exits 4, which a caller never reads as "nothing to do".
gh_or_4() { gh "$@" || { echo "gh $1 $2 failed" >&2; exit 4; }; }

# Exact merged heads identify retired Claims, not completed scenarios.
merged_heads() {
  gh_or_4 api 'repos/{owner}/{repo}/pulls?state=closed&per_page=100' --paginate \
    --jq '.[] | select(.merged_at != null and .base.ref == "main" and .head.repo.id == .base.repo.id) | [.head.sha, ("refs/heads/" + .head.ref)] | @tsv'
}
