# shellcheck shell=bash
# Sourced by the loop scripts from the repository root.

# A gh call whose failure exits 4, which a caller never reads as "nothing to do".
gh_or_4() { gh "$@" || { echo "gh $1 $2 failed" >&2; exit 4; }; }
