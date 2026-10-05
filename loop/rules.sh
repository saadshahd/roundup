#!/usr/bin/env bash
# Machine checks for AGENTS.md. Usage: loop/rules.sh size|trailers|vocab [base-ref] | ready [--offline] | delta <base-dir> <head-dir> | base|class|rounds|merge-ready|proof|carry <pr>
# Scans: `vocab` reads public Rust items and fields, TS exports, and non-comment text under contracts/.
# It does not read imports, enum variants or UI strings.
set -euo pipefail
caller_dir=$PWD
cd "$(dirname "$0")/.."

base=${2:-origin/main}
size_guide=2000

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

# Avoid words from GLOSSARY.md, lowercased; a two-word term joins with `_`.
avoid_phrases() {
  local phrases
  phrases=$(sed -n 's/^_Avoid_: \([^.;]*\).*/\1/p' GLOSSARY.md | tr ',' '\n' |
    awk 'NF { print tolower(($2 != "" && $2 != "in") ? $1 "_" $2 : $1) }')
  [ -n "$phrases" ] || { echo "rule 6: no _Avoid_ words found in GLOSSARY.md" >&2; return 1; }
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
    find crates -path crates/agents/src/claude_code -prune -o -name '*.rs' -print0 |
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

# L54: does a PR's approval still hold after conflict-free merges of main? Prints `carried <A> <H>` or names the commit and rule.
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
  f=$(head -n 1 <<<"$conflicted")
  [ -z "$f" ] || { echo "carry: $c: rule a: a conflict in $f" >&2; return 1; }
  local differs
  differs=$(git diff --name-only "$trial" "$c") || { echo "carry: $c: could not compare with the trial merge" >&2; return 1; }
  f=$(head -n 1 <<<"$differs")
  [ -z "$f" ] || { echo "carry: $c: rule c: $f differs from the trial merge" >&2; return 1; }
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
if command != 'trailers' and not re.fullmatch(r'[0-9]+', number):
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
                events.append((verdict[1], named[0] if named[0] in current_heads else None,
                               'Visual: unchanged' in body.splitlines()))
            elif named and verdict[1] == 'reject':
                target = next((candidate for candidate in reversed(ordered_heads) if candidate in named), None)
                events.append(('reject', target, False))
        pick = re.match(r'ARCHITECT: (split|amend|retire)\b', body)
        ids = re.findall(r'^Architect: ([A-Za-z0-9_.-]+)\s*$', body, re.M)
        if pick and len(ids) == 1 and ids[0] in architects - authors:
            events.append((pick[1], None, False))
    return events

def rounds(events):
    count = 0
    for event, _, _ in events:
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
        if not parts or any(part in ('AGENTS.md', 'CLAUDE.md') for part in parts):
            return 'block'
        if any(part.startswith('.') for part in parts):
            return 'block'
        scenario = bool(re.fullmatch(r'scenarios/[^/]+\.md', path))
        if not (path.startswith('docs/') or scenario):
            return 'block'
        if path in {'docs/development-loop.md', 'docs/boxd.md', 'docs/squads.md', 'docs/design-system.md'} or path.startswith('docs/adr/'):
            return 'block'
        if scenario and (parts[-1].startswith('loop') or parts[-1] in {'daemon.md', 'rpc.md', 'control.md', 'agents.md', 'app.md', 'mcp.md'}):
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
    return 'post'

def section(body, title):
    match = re.search(r'^## '+title+r'\s*\n(.*?)(?=^## |\Z)', body, re.M | re.S)
    require(match, 'proof: missing ## '+title)
    return match[1]

def fences(text):
    return re.findall(r'^```[^\n]*\n(.*?)^```[ \t]*$', text, re.M | re.S)

def latest_tree_verdict(events, head):
    for verdict, reviewed, unchanged in reversed(events):
        if reviewed and ancestor(reviewed, head) and git('rev-parse', reviewed+'^{tree}') == git('rev-parse', head+'^{tree}'):
            return verdict, unchanged
    return None, False

def proof(pr, paths, head, base, events=None):
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
    scopes = re.findall(r'^Proof scope: (.*)$', body, re.M)
    require(not scopes or scopes == ['specification'], 'proof: invalid or repeated Proof scope')
    visible = any(path.startswith(('apps/desktop/src/', 'crates/desktop/')) or path in ('docs/motion.md', 'docs/design-system.md') for path in paths)
    texts = {}
    for path in git('ls-tree', '-r', '--name-only', head, '--', 'scenarios').splitlines():
        if path.endswith('.md'):
            text = git('show', head+':'+path)
            for match in re.finditer(r'^\*\*([A-Z][0-9]+)\b(.*?)(?=^\*\*|^#|\Z)', text, re.M | re.S):
                if match[1] in ids:
                    texts[match[1]] = match[0]
    require(ids <= texts.keys(), 'proof: unknown scenario ids: '+', '.join(sorted(ids - texts.keys())))
    windows = {id_: set(re.findall(r'\b(\d+)\s*(?:×|by|x)\s*(\d+)\b', text)) for id_, text in texts.items()}
    visual_ids = {id_ for id_, text in texts.items() if not id_.startswith('L') and
                  (windows[id_] or re.search(r'screenshot|motion|viewport', text, re.I))}
    visible |= bool(visual_ids)
    output = '\n'.join(fences(evidence))
    require(head in evidence, 'proof: test output must name the head SHA')
    if scopes:
        excluded = {'daemon.md', 'rpc.md', 'control.md', 'README.md', 'AGENTS.md', 'CLAUDE.md'}
        start = git('merge-base', base, head)
        require(paths, 'proof: specification has no changed paths')
        for path in paths:
            name = PurePosixPath(path).name
            product = (re.fullmatch(r'scenarios/[^/.][^/]*\.md', path) and
                       name not in excluded and not name.startswith('loop'))
            allowed = (product or path == 'docs/wireframes.md' or
                       re.fullmatch(r'\.work/prompts/[^/.][^/]*\.md', path))
            require(allowed and name not in {'AGENTS.md', 'CLAUDE.md'},
                    'proof: specification scope cannot include '+path)
            require(all(not entry or entry[0] == '100644'
                        for entry in (tree_entry(start, path), tree_entry(head, path))),
                    'proof: specification needs regular Markdown files: '+path)
        require(re.search(r'^just check: exit 0$', output, re.M), 'proof: missing successful just check baseline')
        require(re.search(r'^Specification consistency: \S.+$', evidence, re.M),
                'proof: missing specification consistency review')
        for id_ in sorted(ids):
            require(re.search(r'^Pending '+id_+r': \S.+$', evidence, re.M),
                    'proof: missing future observer for '+id_)
        if events is None:
            authors, records = history(base, head)
            events = comments(authors, head, base, records)
        require(latest_tree_verdict(events, head)[0] == 'approve',
                'proof: specification needs an independent approval on this tree')
        return
    for id_ in sorted(ids):
        require(re.search(r'^.*\b'+id_+r'(?:\b|_).*\b(?:ok|passed)\b|^.*\b(?:ok|passed)\b.*\b'+id_+r'(?:\b|_)', output, re.M | re.I), 'proof: missing test output for '+id_)
    if not visible:
        return
    if events is None:
        authors, records = history(base, head)
        events = comments(authors, head, base, records)
    if latest_tree_verdict(events, head) == ('approve', True):
        return
    require(visual_ids, 'proof: visible PR needs a named visual scenario')
    for id_ in sorted(visual_ids):
        require(windows[id_], 'proof: visual scenario has no window size: '+id_)
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
    for id_ in sorted(visual_ids):
        for when in ('before', 'after'):
            found = False
            for line in evidence.splitlines():
                if not re.search(r'\b'+id_+r'\b', line) or not re.search(r'\b'+when+r'\b', line, re.I):
                    continue
                link = re.search(r'https://github\.com/'+re.escape(repository)+r'/(?:blob|raw)/'+re.escape(branch)+r'/([^\s)]+)\)\s+(\d+)×(\d+)', line)
                if link and unquote(link[1]) in blobs:
                    width, height = link.group(2, 3)
                    require((width, height) in windows[id_], 'proof: window size not in '+id_)
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
    if command == 'trailers':
        head, base = git('rev-parse', 'HEAD'), git('rev-parse', '--verify', number+'^{commit}')
        authors, records = history(base, head)
        require(records[-1][3], 'trailers: newest commit is not an approval; use ci-trailers for L54 carry')
        trailer_gate(records, authors, 'block', head, base)
        sys.exit(0)
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
        proof(pr, paths, head, base)
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
            for event, named_head, _ in events:
                if named_head and event == 'reject':
                    rejected.add(named_head)
                elif named_head and event == 'approve':
                    rejected = {old for old in rejected if not ancestor(old, named_head)}
            gate(lambda: require(not rejected, 'newest independent verdict is reject'))
            gate(lambda: checks(head))
            lane = lane_of(files, paths, base, head)
            gate(lambda: trailer_gate(records, authors, lane, head, base))
            if lane == 'block':
                gate(lambda: proof(pr, paths, head, base, events))
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

# L34: the queue, derived. One line per `## Work` row of scenarios/*.md: done, in-flight, unspecified, waiting or ready.
ready() {
  python3 - "$@" <<'PY'
import glob, json, os, re, subprocess, sys
files = sorted(glob.glob('scenarios/*.md'))
heads = {m for f in files for m in re.findall(r'^\*\*([A-Z][0-9]+)[ .]', open(f).read(), re.M)}
def ids(text):
    out = []
    for a, b in re.findall(r'\b([A-Z][0-9]+)(?:\s*(?:–|-|to)\s*[A-Z]?([0-9]+))?\b', text):
        if b:
            out += [a[0] + str(n) for n in range(int(a[1:]), int(b) + 1)]
        else:
            out.append(a)
    return out
def tested(i):
    if i[0] == 'L':
        args = ['git', 'grep', '-qE', r'\b' + i + r'\b', '--', 'loop/*.test.sh']
    else:
        args = ['git', 'grep', '-qiE', r'(^|[^a-z0-9])' + i.lower() + '_', '--', ':!scenarios', ':!*.md']
    return subprocess.run(args).returncode == 0
prs = []
if sys.argv[1:] != ['--offline']:
    try:
        result = subprocess.run(['gh', 'pr', 'list', '--state', 'open', '--limit', '1000', '--json', 'number,title'],
                                text=True, stdout=subprocess.PIPE, timeout=int(os.environ.get('BOXD_GH_TIMEOUT', '20')), check=True)
        prs = [(pr['number'], set(ids(pr['title']))) for pr in json.loads(result.stdout)]
    except (subprocess.SubprocessError, OSError, ValueError, KeyError, TypeError) as error:
        print(f'gh: pr list: {error}', file=sys.stderr)
        sys.exit(4)
for f in files:
    lines = open(f).read().split('\n')
    if '## Work' not in lines:
        continue
    rows = [l for l in lines[lines.index('## Work'):] if l.startswith('|')]
    header = [c.strip() for c in rows[0].strip('|').split('|')] if rows else []
    if header != ['Ids', 'Item', 'Owns', 'Keeps green', 'After']:
        print(f'{f}: Work table needs Ids, Item, Owns, Keeps green, After', file=sys.stderr)
        sys.exit(2)
    for row in rows[2:]:
        cells = [c.strip() for c in row.strip().strip('|').split('|')]
        own = ids(cells[0])
        open_prs = ' '.join(f'#{n}' for n, named in prs if named & set(own))
        if own and all(tested(i) for i in own):
            state = 'done'
        elif open_prs:
            state = 'in-flight'
        elif not own or any(i not in heads for i in own):
            state = 'unspecified'
        elif any(not tested(i) for i in ids(cells[4])):
            state = 'waiting'
        else:
            state = 'ready'
        print(state, cells[0], f, *([open_prs] if state == 'in-flight' else []))
PY
}

case "${1:-}" in
  base | class | rounds | merge-ready | proof | ci-trailers)
    [[ ${2:-} =~ ^[0-9]+$ ]] || { echo "usage: loop/rules.sh $1 <pr>" >&2; exit 2; }
    pr_rule "$1" "$2" ;;
  size) size ;;
  trailers) pr_rule trailers "$base" ;;
  vocab) vocab ;;
  ready)
    [[ -z ${2:-} || $2 == --offline ]] || { echo "usage: loop/rules.sh ready [--offline]" >&2; exit 2; }
    ready "${@:2}" ;;
  delta) delta "${2:-}" "${3:-}" ;;
  carry) carry "${2:-}" "${3:-origin/main}" ;;
  *) sed -n '2p' "$0" >&2; exit 2 ;;
esac
