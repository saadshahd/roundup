#!/usr/bin/env bash
# Machine checks for AGENTS.md rules 1, 3 and 6. Usage: loop/rules.sh size|trailers|vocab [base-ref]
set -euo pipefail
cd "$(dirname "$0")/.."

base=${2:-origin/main}
max_lines=400

changed_files() { git diff --name-only "$base"...HEAD; }

is_generated() { case $1 in *.lock | pnpm-lock.yaml | */generated/*) return 0 ;; *) return 1 ;; esac; }

# A module directory is crates/<x>, apps/<x>, ext/<x>, or the first path segment; root files count as "root".
module_of() {
  case $1 in
    crates/* | apps/* | ext/*) echo "$1" | cut -d/ -f1-2 ;;
    */*) echo "${1%%/*}" ;;
    *) echo root ;;
  esac
}

size() {
  local lines=0 modules="" file add del
  while read -r add del file; do
    is_generated "$file" && continue
    [ "$add" = - ] && continue
    lines=$((lines + add + del))
    modules="$modules$(module_of "$file")"$'\n'
  done < <(git diff --numstat "$base"...HEAD)
  local distinct
  distinct=$(printf '%s' "$modules" | sort -u | grep -c . || true)
  [ "$lines" -le "$max_lines" ] || { echo "rule 3: $lines changed lines, max $max_lines" >&2; return 1; }
  [ "$distinct" -le 1 ] || { echo "rule 3: touches $distinct module directories: $(printf '%s' "$modules" | sort -u | tr '\n' ' ')" >&2; return 1; }
}

# Rule 1: every commit names its author agent; the reviewing agent must differ from all of them.
trailers() {
  local authors reviewers
  authors=$(git log --format='%(trailers:key=Author-Agent,valueonly)' "$base"..HEAD | grep . | sort -u || true)
  reviewers=$(git log --format='%(trailers:key=Reviewed-by-Agent,valueonly)' "$base"..HEAD | grep . | sort -u || true)
  local missing
  missing=$(git log --format='%h %(trailers:key=Author-Agent,valueonly)' "$base"..HEAD | awk 'NF == 1 { print $1 }')
  [ -z "$missing" ] || { echo "rule 1: commits without Author-Agent: $missing" >&2; return 1; }
  [ -n "$reviewers" ] || { echo "rule 1: no Reviewed-by-Agent trailer" >&2; return 1; }
  local overlap
  overlap=$(comm -12 <(echo "$authors") <(echo "$reviewers"))
  [ -z "$overlap" ] || { echo "rule 1: reviewer is also author: $overlap" >&2; return 1; }
}

# Rule 6: tokens of Avoid words, from CONTEXT.md, must not appear in public names.
avoid_phrases() {
  sed -n 's/.*_Avoid:_ \([^.;]*\).*/\1/p' CONTEXT.md | tr ',' '\n' | awk '{ print tolower($1 == "" ? "" : ($2 != "" && $2 != "in" ? $1 "_" $2 : $1)) }' | grep .
}

# Identifier-ish tokens of public names: Rust `pub` items, TS `export`s, and everything non-comment under contracts/.
public_names() {
  { grep -rhoE 'pub (async )?(fn|struct|enum|trait|const|static|type|mod) [A-Za-z0-9_]+' crates --include='*.rs' --exclude-dir=claude_code | awk '{ print $NF }'
    grep -rhoE 'export (declare )?(async )?(function|const|type|interface|class|enum) [A-Za-z0-9_]+' contracts apps ext --include='*.ts' --include='*.tsx' 2>/dev/null | awk '{ print $NF }'
    grep -rhvE '^\s*(//|/\*|\*)' contracts 2>/dev/null | grep -oE '[A-Za-z][A-Za-z0-9_.-]*'
  } | sed -E 's/([a-z0-9])([A-Z])/\1_\2/g' | tr 'A-Z.-' 'a-z__' | sort -u
}

vocab() {
  local bad=0 phrase name
  while read -r phrase; do
    while read -r name; do
      case "_${name}_" in *"_${phrase}_"*) echo "rule 6: '$name' uses Avoid word '$phrase'" >&2; bad=1 ;; esac
    done < <(public_names)
  done < <(avoid_phrases)
  return "$bad"
}

case "${1:-}" in
  size) size ;;
  trailers) trailers ;;
  vocab) vocab ;;
  *) sed -n '2p' "$0" >&2; exit 2 ;;
esac
