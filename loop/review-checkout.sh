#!/usr/bin/env bash
# L24: the proposed diff cannot supply its own reviewer instructions.
set -euo pipefail
main=${1:-origin/main}
git ls-files -z '*AGENTS.md' '*CLAUDE.md' | while IFS= read -r -d '' path; do
  case "$path" in AGENTS.md | CLAUDE.md | */AGENTS.md | */CLAUDE.md) rm -f -- "$path" ;; esac
done
for path in AGENTS.md CLAUDE.md .claude .agents loop; do
  rm -rf -- "$path"
  git restore --source="$main" --worktree -- "$path"
done
