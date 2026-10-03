#!/usr/bin/env bash
# Machine checks for AGENTS.md rules 1, 3, 6 and 8. Usage: loop/rules.sh size|trailers|vocab [base-ref] | delta <base-dir> <head-dir> | carry <pr>
# Scans: `vocab` reads public Rust items and fields, TS exports, and non-comment text under contracts/.
# It does not read imports, enum variants or UI strings.
set -euo pipefail
caller_dir=$PWD
cd "$(dirname "$0")/.."

base=${2:-origin/main}
size_guide=2000
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
  # Lines come from a rename-aware diff (a moved file is not delete plus add); modules from a
  # rename-blind one (a move across modules touches both).
  numstat=$(git diff --numstat -M "$base"...HEAD)
  while read -r add del file; do
    [ -n "$file" ] || continue
    is_generated "$file" && continue
    [ "$add" = - ] || lines=$((lines + add + del))
  done <<<"$numstat"
  numstat=$(git diff --numstat --no-renames "$base"...HEAD)
  while read -r _ _ file; do
    [ -n "$file" ] || continue
    is_generated "$file" && continue
    modules="$modules$(module_of "$file")"$'\n'
  done <<<"$numstat"
  local distinct
  distinct=$(printf '%s' "$modules" | sort -u | g -c .)
  [ "$lines" -le "$size_guide" ] || echo "rule 3 size guide: $lines changed lines, guide is about $size_guide; not a failure, the Reviewer notes it" >&2
  [ "$distinct" -le 1 ] || { echo "rule 3 size guide: touches $distinct module directories: $(printf '%s' "$modules" | sort -u | tr '\n' ' '); not a failure, the Reviewer notes it" >&2; }
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

visual_check_ids="D1 D2 D3 D4 D5 D6 D7 D8 D9 D10"

# A directory argument names a path from where the script was run, not from the repo root it moves to.
caller_path() { (cd "$caller_dir" && cd "$1" 2>/dev/null && pwd); }

checks_files() { (cd "$1" && find . -name '*.checks.json' | sed 's|^\./||'); }

# The status of check $2 in checks file $1. Anything but pass or fail is an error, never a pass.
check_status() {
  local status
  status=$(jq -r --arg id "$2" '.[$id].status // "missing"' "$1") || { echo "delta: $1 is not JSON" >&2; return 2; }
  case $status in
    pass | fail) echo "$status" ;;
    *) echo "delta: $1 check $2 has status '$status', not pass or fail" >&2; return 2 ;;
  esac
}

# Rule 8: compares two builds' design checks (docs/design-system.md). Exit 1 when a check regressed, 2 when the input cannot be compared.
delta() {
  local base_dir head_dir files out="" regressed=0 file dir id was now verdict
  base_dir=$(caller_path "$1") && head_dir=$(caller_path "$2") || { echo "delta: usage: delta <base-dir> <head-dir>, both directories" >&2; return 2; }
  files=$(sort -u <(checks_files "$base_dir") <(checks_files "$head_dir"))
  [ -n "$files" ] || { echo "delta: no .checks.json files in $base_dir or $head_dir" >&2; return 2; }
  while read -r file; do
    for dir in "$base_dir" "$head_dir"; do
      [ -f "$dir/$file" ] || { echo "delta: $file is missing from $dir" >&2; return 2; }
    done
    for id in $visual_check_ids; do
      was=$(check_status "$base_dir/$file" "$id") || return 2
      now=$(check_status "$head_dir/$file" "$id") || return 2
      case "$was $now" in
        "fail pass") verdict=fixed ;;
        "pass fail") verdict=regressed; regressed=1 ;;
        "fail fail") verdict=still-failing ;;
        *) verdict=still-passing ;;
      esac
      out+="${file%.checks.json} $id $verdict"$'\n'
    done
  done <<<"$files"
  printf '%s' "$out"
  return "$regressed"
}

# L54: does a PR's approval still hold after merges of main? Prints `carried <A> <H>` or names the commit and rule.
carry_allowed() { case $1 in .work/queue.md | .work/queue/* | scenarios/README.md) return 0 ;; *) return 1 ;; esac; }
lines_of() { git show "$1:$2" 2>/dev/null | sort; }
count_of() { printf '%s\n' "$1" | g -cxF -- "$2"; }

# Rule b for one file of merge $1 (parents $2 and $3): the result holds exactly the first parent's lines that main
# did not remove, plus the lines main added, each at most as often as either side has it.
carry_rule_b() {
  local merge=$1 p1=$2 p2=$3 file=$4 base keep result line
  base=$(git merge-base "$p1" "$p2")
  keep=$( { comm -23 <(lines_of "$p1" "$file") <(comm -23 <(lines_of "$base" "$file") <(lines_of "$p2" "$file") | sort -u) ; comm -13 <(lines_of "$base" "$file") <(lines_of "$p2" "$file"); } | sort -u)
  result=$(lines_of "$merge" "$file")
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    printf '%s\n' "$keep" | grep -qxF -- "$line" || { echo "carry: $merge: rule b: $file holds a line neither side may keep: $line" >&2; return 1; }
    [ "$(count_of "$result" "$line")" -le "$(( $(count_of "$(lines_of "$p1" "$file")" "$line") > $(count_of "$(lines_of "$p2" "$file")" "$line") ? $(count_of "$(lines_of "$p1" "$file")" "$line") : $(count_of "$(lines_of "$p2" "$file")" "$line") ))" ] || { echo "carry: $merge: rule b: $file repeats a line: $line" >&2; return 1; }
  done <<<"$result"
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    printf '%s\n' "$result" | grep -qxF -- "$line" || { echo "carry: $merge: rule b: $file lost a line a side keeps: $line" >&2; return 1; }
  done <<<"$keep"
}

carry_one() {
  local c=$1 p1 p2 extra trial conflicted f
  read -r _ p1 p2 extra <<<"$(git rev-list --parents -n 1 "$c")"
  { [ -n "${p2:-}" ] && [ -z "${extra:-}" ]; } || { echo "carry: $c: not a merge of main" >&2; return 1; }
  git merge-base --is-ancestor "$p2" origin/main || { echo "carry: $c: not a merge of main (second parent is not on origin/main)" >&2; return 1; }
  git log -1 --format=%B "$c" | grep -q '^Author-Agent: ' || { echo "carry: $c: no Author-Agent trailer" >&2; return 1; }
  trial=$(git merge-tree --write-tree --name-only --no-messages "$p1" "$p2" || true)
  conflicted=$(sed 1d <<<"$trial" | sed '/^$/d')
  trial=$(head -n 1 <<<"$trial")
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    carry_allowed "$f" || { echo "carry: $c: rule a: a conflict in $f" >&2; return 1; }
    carry_rule_b "$c" "$p1" "$p2" "$f" || return 1
  done <<<"$conflicted"
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    grep -qxF -- "$f" <<<"$conflicted" || { echo "carry: $c: rule c: $f differs from the trial merge" >&2; return 1; }
  done <<<"$(git diff --name-only "$trial" "$c")"
}

carry() {
  local pr=$1 head approval c chain=()
  [[ $pr =~ ^[0-9]+$ ]] || { echo "carry: <pr> must be digits" >&2; return 2; }
  head=$(gh pr view "$pr" --json headRefOid -q .headRefOid) || { echo "carry: gh failed reading PR $pr" >&2; return 4; }
  git cat-file -e "$head^{commit}" 2>/dev/null || { echo "carry: head $head is not fetched" >&2; return 1; }
  for c in $(git rev-list --first-parent "$head"); do
    if [ -z "$(git diff-tree --no-commit-id --name-only -r "$c^!" 2>/dev/null)" ] && [ "$(git rev-list --parents -n 1 "$c" | wc -w)" -eq 2 ] && git log -1 --format=%B "$c" | grep -q '^Reviewed-by-Agent: '; then approval=$c; break; fi
    chain=("$c" ${chain[@]+"${chain[@]}"})
  done
  [ -n "${approval:-}" ] || { echo "carry: no approval commit on $head" >&2; return 1; }
  for c in "${chain[@]+"${chain[@]}"}"; do carry_one "$c" || return 1; done
  echo "carried $approval $head"
}

case "${1:-}" in
  size) size ;;
  trailers) trailers ;;
  vocab) vocab ;;
  delta) delta "${2:-}" "${3:-}" ;;
  carry) carry "${2:-}" ;;
  *) sed -n '2p' "$0" >&2; exit 2 ;;
esac
