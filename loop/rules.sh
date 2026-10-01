#!/usr/bin/env bash
# Machine checks for AGENTS.md rules 1, 3 and 6. Usage: loop/rules.sh size|trailers|vocab [base-ref]
# Scans: `vocab` reads public Rust items and fields, TS exports, and non-comment text under contracts/.
# It does not read imports, enum variants or UI strings.
set -euo pipefail
cd "$(dirname "$0")/.."

base=${2:-origin/main}
max_lines=400
unit=$'\x1f'

# grep that treats "no match" as success but a real error as failure.
g() { grep "$@" || [ $? -eq 1 ]; }

is_generated() { case $1 in *.lock | *pnpm-lock.yaml | */generated/*) return 0 ;; *) return 1 ;; esac; }

# A module directory is crates/<x>, apps/<x>, ext/<x>, or the first path segment; root files count as "root".
module_of() {
  case $1 in
    crates/* | apps/* | ext/*) echo "$1" | cut -d/ -f1-2 ;;
    */*) echo "${1%%/*}" ;;
    *) echo root ;;
  esac
}

size() {
  local numstat lines=0 modules="" file add del
  numstat=$(git diff --numstat --no-renames "$base"...HEAD)
  while read -r add del file; do
    [ -n "$file" ] || continue
    is_generated "$file" && continue
    modules="$modules$(module_of "$file")"$'\n'
    [ "$add" = - ] || lines=$((lines + add + del))
  done <<<"$numstat"
  local distinct
  distinct=$(printf '%s' "$modules" | sort -u | g -c .)
  [ "$lines" -le "$max_lines" ] || { echo "rule 3: $lines changed lines, max $max_lines" >&2; return 1; }
  [ "$distinct" -le 1 ] || { echo "rule 3: touches $distinct module directories: $(printf '%s' "$modules" | sort -u | tr '\n' ' ')" >&2; return 1; }
}

# Rule 1: every authored commit names its agent. An approval is an empty commit carrying only
# Reviewed-by-Agent, it must be the newest commit (anything pushed after it needs a new approval),
# and its id must differ from every author id. Both ids are self-asserted strings, not identities.
trailers() {
  local log authors="" reviewers="" bad="" hash author reviewer first=1 newest_is_approval=0
  log=$(git log --no-merges --format="%h$unit%(trailers:key=Author-Agent,valueonly,separator=%x2C)$unit%(trailers:key=Reviewed-by-Agent,valueonly,separator=%x2C)" "$base"..HEAD)
  while IFS="$unit" read -r hash author reviewer; do
    [ -n "$hash" ] || continue
    author=$(echo "$author" | tr -d '[:space:]')
    reviewer=$(echo "$reviewer" | tr -d '[:space:]')
    if [ -n "$reviewer" ]; then
      if [ -n "$author" ] || [ -n "$(git diff-tree --no-commit-id --name-only -r "$hash")" ]; then
        bad="$bad $hash"
      else
        reviewers="$reviewers$(echo "$reviewer" | tr ',' '\n')"$'\n'
        [ "$first" -eq 0 ] || newest_is_approval=1
      fi
    elif [ -n "$author" ]; then
      authors="$authors$(echo "$author" | tr ',' '\n')"$'\n'
    else
      bad="$bad $hash"
    fi
    first=0
  done <<<"$log"
  [ -z "$bad" ] || { echo "rule 1: commits that are neither authored nor a clean approval (an approval must be empty and carry only Reviewed-by-Agent):$bad" >&2; return 1; }
  [ -n "$reviewers" ] || { echo "rule 1: no approval commit (Reviewed-by-Agent)" >&2; return 1; }
  [ "$newest_is_approval" -eq 1 ] || { echo "rule 1: commits were pushed after the last approval" >&2; return 1; }
  local overlap
  overlap=$(comm -12 <(printf '%s' "$authors" | sort -u) <(printf '%s' "$reviewers" | sort -u))
  [ -z "$overlap" ] || { echo "rule 1: reviewer is also author: $overlap" >&2; return 1; }
}

# Avoid words from CONTEXT.md, lowercased; a two-word term joins with `_`.
avoid_phrases() {
  local phrases
  phrases=$(sed -n 's/.*_Avoid:_ \([^.;]*\).*/\1/p' CONTEXT.md | tr ',' '\n' |
    awk 'NF { print tolower(($2 != "" && $2 != "in") ? $1 "_" $2 : $1) }')
  [ -n "$phrases" ] || { echo "rule 6: no _Avoid:_ words found in CONTEXT.md" >&2; return 1; }
  echo "$phrases"
}

# Lowercased snake tokens of public names.
public_names() {
  local rust_pub='pub(\([a-z ]+\))? +((async|const|unsafe|extern) +)*(fn|struct|enum|trait|const|static|type|mod|union) +[A-Za-z0-9_]+'
  local rust_field='^ *pub(\([a-z ]+\))? +[A-Za-z0-9_]+ *:'
  local ts='export +(declare +)?(async +)?(function|const|type|interface|class|enum) +[A-Za-z0-9_]+'
  local dirs="" d
  for d in contracts apps ext; do [ ! -d "$d" ] || dirs="$dirs $d"; done
  {
    find crates -path crates/agents/claude_code -prune -o -name '*.rs' -print0 |
      xargs -0 grep -hoE "$rust_pub|$rust_field" | tr -d ':' | awk '{ print $NF }'
    # shellcheck disable=SC2086 # $dirs is a word list on purpose
    if [ -n "$dirs" ]; then
      g -rhoE "$ts" $dirs --include='*.ts' --include='*.tsx' | awk '{ print $NF }'
      g -rhvE '^\s*(//|/\*|\*)' contracts | g -oE '[A-Za-z][A-Za-z0-9_.-]*'
    fi
  } | sed -E 's/([a-z0-9])([A-Z])/\1_\2/g' | tr 'A-Z.-' 'a-z__' | sort -u
}

vocab() {
  local phrases names bad=0 phrase name
  phrases=$(avoid_phrases)
  names=$(public_names)
  while read -r phrase; do
    while read -r name; do
      case "_${name}_" in *"_${phrase}_"* | *"_${phrase}s_"* | *"_${phrase}es_"*) echo "rule 6: '$name' uses Avoid word '$phrase'" >&2; bad=1 ;; esac
    done <<<"$names"
  done <<<"$phrases"
  return "$bad"
}

case "${1:-}" in
  size) size ;;
  trailers) trailers ;;
  vocab) vocab ;;
  *) sed -n '2p' "$0" >&2; exit 2 ;;
esac
