#!/usr/bin/env bash
# Machine checks for AGENTS.md. Usage: loop/rules.sh size|trailers|vocab [base-ref] | delta <base-dir> <head-dir> | base|class|rounds|merge-ready|proof|carry <pr>
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
  if ! base_dir=$(caller_path "$1") || ! head_dir=$(caller_path "$2"); then
    echo "delta: usage: delta <base-dir> <head-dir>, both directories" >&2; return 2
  fi
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
carry_allowed() { case $1 in .work/queue.md | .work/queue/*) return 0 ;; *) return 1 ;; esac; }

# Rule b for one file of merge $1 (parents $2 and $3): the result holds exactly the first parent's lines that main
# did not remove, plus the lines main added, each at most as often as either side has it.
carry_rule_b() {
  local merge=$1 p1=$2 p2=$3 file=$4 base
  base=$(git merge-base "$p1" "$p2")
  python3 - "$merge" "$p1" "$p2" "$base" "$file" <<'PY'
from collections import Counter
import subprocess
import sys

merge, first_ref, main_ref, base_ref, path = sys.argv[1:]

def lines(ref):
    result = subprocess.run(['git', 'show', f'{ref}:{path}'], capture_output=True)
    if result.returncode:
        return None
    content = result.stdout.split(b'\n')
    if content[-1] == b'':
        content.pop()
    return Counter(content)

result = lines(merge)
if result is None:
    sys.exit(f'carry: {merge}: rule b: {path} conflicted and is gone from the merge (one side deleted it)')
base = lines(base_ref) or Counter()
first = lines(first_ref) or Counter()
main = lines(main_ref) or Counter()
keep = (set(first) - (set(base) - set(main))) | (set(main) - set(base))
for line, count in result.items():
    if line not in keep:
        sys.exit(f'carry: {merge}: rule b: {path} holds a line neither side may keep: {line!r}')
    cap = 1 if not base[line] and first[line] and main[line] else max(first[line], main[line])
    if count > cap:
        sys.exit(f'carry: {merge}: rule b: {path} repeats a line: {line!r}')
for line in keep - set(result):
    sys.exit(f'carry: {merge}: rule b: {path} lost a line a side keeps: {line!r}')
PY
}

carry_one() {
  local c=$1 main_ref=$2 p1 p2 extra trial conflicted f
  read -r _ p1 p2 extra <<<"$(git rev-list --parents -n 1 "$c")"
  { [ -n "${p2:-}" ] && [ -z "${extra:-}" ]; } || { echo "carry: $c: not a merge of main" >&2; return 1; }
  git merge-base --is-ancestor "$p2" "$main_ref" || { echo "carry: $c: not a merge of main (second parent is not on $main_ref)" >&2; return 1; }
  git log -1 --format=%B "$c" | grep -q '^Author-Agent: ' || { echo "carry: $c: no Author-Agent trailer" >&2; return 1; }
  local mt_rc=0
  trial=$(git merge-tree --write-tree --name-only --no-messages "$p1" "$p2" 2>/dev/null) || mt_rc=$?
  { [ "$mt_rc" -le 1 ] && [ -n "$trial" ]; } || { echo "carry: $c: merge-tree failed (exit $mt_rc)" >&2; return 1; }
  conflicted=$(sed 1d <<<"$trial" | sed '/^$/d')
  trial=$(head -n 1 <<<"$trial")
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    carry_allowed "$f" || { echo "carry: $c: rule a: a conflict in $f" >&2; return 1; }
    carry_rule_b "$c" "$p1" "$p2" "$f" || return 1
  done <<<"$conflicted"
  local differs
  differs=$(git diff --name-only "$trial" "$c") || { echo "carry: $c: could not compare with the trial merge" >&2; return 1; }
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    grep -qxF -- "$f" <<<"$conflicted" || { echo "carry: $c: rule c: $f differs from the trial merge" >&2; return 1; }
  done <<<"$differs"
}

carry() {
  local pr=$1 main_ref=${2:-origin/main} head approval c chain=()
  [[ $pr =~ ^[0-9]+$ ]] || { echo "carry: <pr> must be digits" >&2; return 2; }
  head=$(gh pr view "$pr" --json headRefOid -q .headRefOid) || { echo "carry: gh failed reading PR $pr" >&2; return 4; }
  git cat-file -e "$head^{commit}" 2>/dev/null || { echo "carry: head $head is not fetched" >&2; return 1; }
  for c in $(git rev-list --first-parent "$head"); do
    if [ -z "$(git diff-tree --no-commit-id --name-only -r "$c^!" 2>/dev/null)" ] && [ "$(git rev-list --parents -n 1 "$c" | wc -w)" -eq 2 ] && git log -1 --format=%B "$c" | grep -q '^Reviewed-by-Agent: '; then approval=$c; break; fi
    chain=("$c" ${chain[@]+"${chain[@]}"})
  done
  [ -n "${approval:-}" ] || { echo "carry: no approval commit on $head" >&2; return 1; }
  for c in "${chain[@]+"${chain[@]}"}"; do carry_one "$c" "$main_ref" || return 1; done
  echo "carried $approval $head"
}

# PR trees are data: never check them out or execute their copy of this script.
pr_rule() {
  python3 - "$1" "${2:-}" <<'PY'
import json
from datetime import datetime
import os
import re
import subprocess
import sys
import unicodedata
from pathlib import PurePosixPath
from urllib.parse import unquote

command, number = sys.argv[1:]
if not re.fullmatch(r'[0-9]+', number):
    sys.exit(f'usage: loop/rules.sh {command} <pr>')

class Refused(Exception):
    pass

def require(ok, reason):
    if not ok:
        raise Refused(reason)

try:
    limit = int(os.environ.get('BOXD_GH_TIMEOUT', '20'))
    require(limit > 0, 'BOXD_GH_TIMEOUT must be positive')
except (ValueError, Refused) as error:
    print(error, file=sys.stderr)
    sys.exit(4)

def gh(route):
    try:
        result = subprocess.run(['gh', 'api', route, '--paginate', '--slurp'],
                                text=True, stdout=subprocess.PIPE, timeout=limit, check=True)
        pages = json.loads(result.stdout)
        require(isinstance(pages, list) and bool(pages), 'gh returned no pages')
        return pages
    except (subprocess.SubprocessError, OSError, ValueError, Refused) as error:
        print(f'gh: {route}: {error}', file=sys.stderr)
        sys.exit(4)

def one(route):
    pages = gh(route)
    require(len(pages) == 1 and isinstance(pages[0], dict), f'invalid object: {route}')
    return pages[0]

def listing(route):
    pages = gh(route)
    require(all(isinstance(page, list) for page in pages), f'invalid list: {route}')
    items = [item for page in pages for item in page]
    require(all(isinstance(item, dict) for item in items), f'invalid entries: {route}')
    return items

repo = 'repos/{owner}/{repo}'
pull = f'{repo}/pulls/{number}'

def git(*args):
    return subprocess.check_output(['git', *args], text=True, timeout=limit).rstrip('\n')

def ancestor(older, newer):
    result = subprocess.run(['git', 'merge-base', '--is-ancestor', older, newer], timeout=limit)
    require(result.returncode in (0, 1), f'cannot compare {older} to {newer}')
    return result.returncode == 0

def sha(value):
    require(isinstance(value, str) and re.fullmatch(r'[0-9a-f]{40}', value), 'missing or invalid SHA')
    return value

def fetch(value):
    found = subprocess.run(['git', 'cat-file', '-e', value+'^{commit}'], stderr=subprocess.DEVNULL)
    if found.returncode:
        # Fetch objects only; hooks, checkout filters and PR executables never run.
        subprocess.run(['git', '-c', 'core.hooksPath=/dev/null', 'fetch', '--no-tags', 'origin', value],
                       timeout=limit, check=True, stdout=subprocess.DEVNULL)

def base_gate(pr):
    ref = pr['base']['ref']
    require(ref == 'main', f'base is {ref}, not main')

def trailers(commit, key):
    value = git('show', '-s', f'--format=%(trailers:key={key},valueonly)', commit)
    ids = value.splitlines()
    require(all(re.fullmatch(r'[A-Za-z0-9_.-]+', agent) for agent in ids), f'invalid {key} on {commit}')
    return set(ids)

def history(base, head):
    commits = git('rev-list', '--reverse', base+'..'+head).splitlines()
    require(commits, 'no PR commits')
    authors, records = set(), []
    for commit in commits:
        parents = git('rev-list', '--parents', '-n', '1', commit).split()[1:]
        author, reviewer = trailers(commit, 'Author-Agent'), trailers(commit, 'Reviewed-by-Agent')
        authors.update(author)
        records.append((commit, parents, author, reviewer))
    require(authors, 'no Author-Agent trailers')
    return authors, records

def trailer_gate(records, authors, lane, head, base):
    for commit, parents, author, reviewer in records:
        if len(parents) != 1:
            require(not reviewer and (lane == 'post' or author),
                    f'trailers: merge {commit} has an invalid trailer')
        elif lane == 'post':
            require(author and not reviewer, f'trailers: {commit} needs Author-Agent only')
        elif reviewer:
            require(not author and not (reviewer & authors) and
                    git('rev-parse', commit+'^{tree}') == git('rev-parse', parents[0]+'^{tree}'),
                    f'trailers: {commit} is not a clean independent approval')
        else:
            require(author, f'trailers: {commit} has no Author-Agent')
    if lane == 'block' and not records[-1][3]:
        approval = next((commit for commit, _, _, reviewer in reversed(records) if reviewer), None)
        require(approval, 'trailers: no approval commit')
        try:
            result = subprocess.run(['bash', 'loop/rules.sh', 'carry', number, base],
                                    text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=limit)
        except subprocess.TimeoutExpired as error:
            print(f'trailers: carry timed out: {error}', file=sys.stderr)
            sys.exit(4)
        if result.returncode == 4:
            print('trailers: carry: '+result.stderr.strip(), file=sys.stderr)
            sys.exit(4)
        require(result.returncode == 0 and result.stdout.strip() == f'carried {approval} {head}',
                'trailers: approval was not carried: '+result.stderr.strip())
    elif lane == 'block':
        require(records[-1][0] == head and len(records[-1][1]) == 1,
                'trailers: head is not an empty approval commit')

def comments(authors, head, base, records):
    data = listing(f'{repo}/issues/{number}/comments?per_page=100')
    require(all(isinstance(c.get('body'), str) and isinstance(c.get('created_at'), str)
                and isinstance(c.get('id'), int) for c in data), 'invalid comment data')
    architects_text = git('show', base+':docs/squads.md')
    line = next((line for line in architects_text.splitlines() if line.startswith('Three architects:')), '')
    architects = set(re.findall(r'`([^`]+)`', line))
    require(architects, 'missing Three architects in trusted docs/squads.md')
    events = []
    approval = next((record for record in reversed(records) if record[3]), None)
    reviewed_head = approval[1][0] if approval else head
    ordered_heads = [reviewed_head]
    if approval:
        ordered_heads.extend(record[0] for record in records[records.index(approval):])
    else:
        ordered_heads.append(head)
    current_heads = set(ordered_heads)
    for item in data:
        require(re.fullmatch(r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z', item['created_at']), 'invalid comment time')
        datetime.fromisoformat(item['created_at'].replace('Z', '+00:00'))
    for item in sorted(data, key=lambda c: (c['created_at'], c['id'])):
        body = item['body']
        verdict = re.match(r'VERDICT: (approve|reject)\b', body)
        ids = re.findall(r'^Reviewed-by-Agent: ([A-Za-z0-9_.-]+)\s*$', body, re.M)
        if verdict and len(ids) == 1 and ids[0] not in authors:
            explicit = re.findall(r'^(?:Reviewed-head|Head): ([0-9a-f]{40})\s*$', body, re.M)
            named = explicit if explicit else re.findall(r'(?<![0-9a-f])[0-9a-f]{40}(?![0-9a-f])', body)
            if len(set(named)) == 1:
                events.append((verdict[1], named[0] if named[0] in current_heads else None))
            elif named and verdict[1] == 'reject':
                target = next((candidate for candidate in reversed(ordered_heads) if candidate in named), None)
                events.append(('reject', target))
        pick = re.match(r'ARCHITECT: (split|amend|retire)\b', body)
        ids = re.findall(r'^Architect: ([A-Za-z0-9_.-]+)\s*$', body, re.M)
        if pick and len(ids) == 1 and ids[0] in architects - authors:
            events.append((pick[1], None))
    return events

def rounds(events):
    count = 0
    for event, _ in events:
        require(event != 'retire', 'retired')
        if event in ('split', 'amend'):
            count = 0
        elif event == 'reject':
            count += 1
    require(count < 3, f'round cap: {count} rejects, an Architect must pick split, amend or retire')
    return count

def changed(base, head):
    files = listing(pull+'/files?per_page=100')
    require(all(isinstance(f.get('filename'), str) for f in files), 'invalid changed files')
    paths = git('diff', '--name-only', '--no-renames', '-z', base+'...'+head).split('\0')
    paths = set(filter(None, paths))
    reported = {p for f in files for p in (f['filename'], f.get('previous_filename')) if p}
    require(paths == reported, 'incomplete or changed PR file list')
    return files, paths

def tree_entry(ref, path):
    return git('ls-tree', ref, '--', path).split('\t')[0].split()

def lane_of(files, paths, base, head):
    if not files or any(not isinstance(f.get('patch'), str) or not f['patch'] for f in files):
        return 'block'
    start = git('merge-base', base, head)
    for path in paths:
        parts = PurePosixPath(path).parts
        queue = path == '.work/queue.md' or bool(re.fullmatch(r'\.work/queue/[^/]+\.md', path))
        if not parts or any(part in ('AGENTS.md', 'CLAUDE.md') for part in parts):
            return 'block'
        if any(part.startswith('.') for part in (parts[1:] if queue else parts)):
            return 'block'
        scenario = bool(re.fullmatch(r'scenarios/[^/]+\.md', path))
        if not (queue or path.startswith('docs/') or scenario):
            return 'block'
        if path in {'docs/development-loop.md', 'docs/boxd.md', 'docs/squads.md', 'docs/design-system.md'} or path.startswith('docs/adr/'):
            return 'block'
        if scenario and parts[-1] in {'loop.md', 'daemon.md', 'rpc.md', 'control.md', 'agents.md', 'app.md', 'mcp.md'}:
            return 'block'
        old, new = tree_entry(start, path), tree_entry(head, path)
        if any(entry and entry[0] not in ('100644', '100755') for entry in (old, new)) or (old and new and old[0] != new[0]):
            return 'block'
        diff = git('diff', '--no-ext-diff', '--no-textconv', '--no-renames', '--unified=0', start, head, '--', path)
        if 'Binary files ' in diff or 'GIT binary patch' in diff:
            return 'block'
        if scenario:
            lines, in_hunk = [], False
            for line in diff.splitlines():
                if line.startswith('@@'):
                    in_hunk = True
                elif in_hunk and line[:1] in ('+', '-'):
                    lines.append(line[1:])
            folded = unicodedata.normalize('NFKC', '\n'.join(lines)).lower()
            if re.search(r'secret|token|credential|password|oauth|identity|impersonat|daemon|rupd|api key|auth|author-agent|reviewed-by|verdict|architect|approve|merger|rpc|seam|(?<!\w)(card|loop|key)(?!\w)', folded):
                return 'block'
        if path == '.work/queue.md':
            for ref in (start, head):
                entry = tree_entry(ref, path)
                if not entry:
                    return 'block'
            old_text, new_text = (git('show', ref+':'+path) for ref in (start, head))
            heading = '## Swarm protocol'
            if heading not in old_text.splitlines() or heading not in new_text.splitlines():
                return 'block'
            if old_text[old_text.index(heading):] != new_text[new_text.index(heading):]:
                return 'block'
    return 'post'

def section(body, title):
    match = re.search(r'^## '+title+r'\s*\n(.*?)(?=^## |\Z)', body, re.M | re.S)
    require(match, 'proof: missing ## '+title)
    return match[1]

def fences(text):
    return re.findall(r'^```[^\n]*\n(.*?)^```[ \t]*$', text, re.M | re.S)

def proof(pr, paths, head):
    body = pr['body']
    require(isinstance(body, str), 'proof: missing body')
    missing = []
    for title in ('Shape', 'Proof'):
        try:
            section(body, title)
        except Refused as error:
            missing.append(str(error))
    require(not missing, '\n'.join(missing))
    shape = fences(section(body, 'Shape'))
    require(len(shape) == 1 and 0 < len(shape[0].splitlines()) <= 40, 'proof: Shape needs one fence of 1 to 40 lines')
    evidence = section(body, 'Proof')
    named = re.search(r'^Scenarios:\s*(.+)$', body, re.M)
    ids = set(re.findall(r'\b[A-Z][0-9]+\b', named[1])) if named else set()
    require(ids, 'proof: no scenario ids')
    visible = any(path.startswith(('apps/desktop/src/', 'crates/desktop/')) or path in ('docs/motion.md', 'docs/design-system.md') for path in paths)
    texts = {}
    for path in git('ls-tree', '-r', '--name-only', head, '--', 'scenarios').splitlines():
        if path.endswith('.md'):
            text = git('show', head+':'+path)
            for match in re.finditer(r'^\*\*([A-Z][0-9]+)\b(.*?)(?=^\*\*|^#|\Z)', text, re.M | re.S):
                if match[1] in ids:
                    texts[match[1]] = match[0]
    require(ids <= texts.keys(), 'proof: unknown scenario ids: '+', '.join(sorted(ids - texts.keys())))
    visible |= any(re.search(r'screenshot|motion', text, re.I) for id_, text in texts.items() if not id_.startswith('L'))
    output = '\n'.join(fences(evidence))
    require(head in evidence, 'proof: test output must name the head SHA')
    for id_ in sorted(ids):
        require(re.search(r'^.*\b'+id_+r'(?:\b|_).*\b(?:ok|passed)\b|^.*\b(?:ok|passed)\b.*\b'+id_+r'(?:\b|_)', output, re.M | re.I), 'proof: missing test output for '+id_)
    if not visible:
        return
    require(re.search(r'^Shows: .+', evidence, re.M), 'proof: missing Shows:')
    captions = re.sub(r'^```[^\n]*\n.*?^```[ \t]*$', '', evidence, flags=re.M | re.S)
    require(not re.search(r'/Users/|/home/|secret|token|credential|password|oauth', captions, re.I), 'proof: private path or secret word in caption')
    repository = one(repo)['full_name']
    require(re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repository), 'proof: invalid repository')
    branch = f'proof/pr-{number}'
    tree = one(f'{repo}/git/trees/{branch}?recursive=1')
    require(tree.get('truncated') is False and isinstance(tree.get('tree'), list), 'proof: incomplete branch tree')
    blobs = {}
    for item in tree['tree']:
        require(item['path'] == 'proof' or item['path'].startswith('proof/'), 'proof: file outside proof/')
        if item['type'] == 'tree':
            continue
        require(item['type'] == 'blob' and item['mode'] == '100644' and isinstance(item['size'], int), 'proof: invalid file')
        require(PurePosixPath(item['path']).suffix in ('.png', '.webp', '.mp4', '.webm'), 'proof: unsupported file')
        require(0 < item['size'] <= 5*1024*1024, 'proof: file exceeds 5 MiB or is empty')
        blobs[item['path']] = item
    require(sum(item['size'] for item in blobs.values()) <= 20*1024*1024, 'proof: branch exceeds 20 MiB')
    for id_ in sorted(ids):
        for when in ('before', 'after'):
            found = False
            for line in evidence.splitlines():
                if not re.search(r'\b'+id_+r'\b', line) or not re.search(r'\b'+when+r'\b', line, re.I):
                    continue
                link = re.search(r'https://github\.com/'+re.escape(repository)+r'/(?:blob|raw)/'+re.escape(branch)+r'/([^\s)]+)\)\s+(\d+)×(\d+)', line)
                if link and unquote(link[1]) in blobs:
                    width, height = link.group(2, 3)
                    require(re.search(width+r'\s*(?:×|by|x)\s*'+height, texts[id_]), 'proof: window size not in '+id_)
                    found = True
            require(found, f'proof: missing {id_} {when} image/video and window size')

def checks(head):
    pages = gh(f'{repo}/commits/{head}/check-runs?per_page=100&filter=latest')
    require(all(isinstance(page, dict) and isinstance(page.get('check_runs'), list) for page in pages), 'invalid check runs')
    runs = [run for page in pages for run in page['check_runs']]
    require(all(isinstance(run, dict) for run in runs), 'invalid check run')
    for name in ('check', 'rules'):
        matches = [run for run in runs if run.get('name') == name]
        require(matches and all(run.get('head_sha') == head and run.get('status') == 'completed'
                and run.get('conclusion') == 'success' and run.get('app', {}).get('slug') == 'github-actions'
                for run in matches), f'{name}: missing or not successful on head {head}')

try:
    pr = one(pull)
    if command == 'base':
        base_gate(pr)
        sys.exit(0)
    head, base = sha(pr['head']['sha']), sha(pr['base']['sha'])
    fetch(head)
    fetch(base)
    if command in ('class', 'proof', 'merge-ready', 'ci-trailers'):
        files, paths = changed(base, head)
    if command == 'class':
        print(lane_of(files, paths, base, head))
    elif command == 'proof':
        proof(pr, paths, head)
    elif command == 'ci-trailers':
        authors, records = history(base, head)
        base_gate(pr)
        trailer_gate(records, authors, lane_of(files, paths, base, head), head, base)
    else:
        authors, records = history(base, head)
        events = comments(authors, head, base, records)
        if command == 'rounds':
            print(rounds(events))
        else:
            errors = []
            def gate(call):
                try:
                    call()
                except Refused as error:
                    errors.append(str(error))
            gate(lambda: base_gate(pr))
            gate(lambda: require(pr['state'] == 'open' and pr['draft'] is False, 'PR is closed or draft'))
            gate(lambda: rounds(events))
            rejected = set()
            for event, named_head in events:
                if named_head and event == 'reject':
                    rejected.add(named_head)
                elif named_head and event == 'approve':
                    rejected = {old for old in rejected if not ancestor(old, named_head)}
            gate(lambda: require(not rejected, 'newest independent verdict is reject'))
            gate(lambda: checks(head))
            lane = lane_of(files, paths, base, head)
            gate(lambda: trailer_gate(records, authors, lane, head, base))
            if lane == 'block':
                gate(lambda: proof(pr, paths, head))
            latest = one(pull)
            gate(lambda: require(latest == pr, 'PR changed during merge-ready'))
            require(not errors, '\n'.join(errors))
            print('ready '+head)
except (Refused, KeyError, TypeError, ValueError, OSError, subprocess.SubprocessError) as error:
    print(f'{command}: {error}', file=sys.stderr)
    if command == 'class':
        print('block')
        sys.exit(0)
    sys.exit(1)
PY
}

case "${1:-}" in
  base | class | rounds | merge-ready | proof | ci-trailers)
    [[ ${2:-} =~ ^[0-9]+$ ]] || { echo "usage: loop/rules.sh $1 <pr>" >&2; exit 2; }
    pr_rule "$1" "$2" ;;
  size) size ;;
  trailers) trailers ;;
  vocab) vocab ;;
  delta) delta "${2:-}" "${3:-}" ;;
  carry) carry "${2:-}" "${3:-origin/main}" ;;
  *) sed -n '2p' "$0" >&2; exit 2 ;;
esac
