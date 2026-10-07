#!/usr/bin/env bash
# Machine checks for AGENTS.md. Usage: loop/rules.sh vocab | touches | ready [--offline] | delta <base-dir> <head-dir> | base|ci-trailers|verdicts|merge-ready <pr> | clean-merge <commit> [main-ref]
# Scans: `vocab` reads public Rust items and fields, TS exports, and non-comment text under contracts/.
# It does not read imports, enum variants or UI strings.
set -euo pipefail
caller_dir=$PWD
cd "$(dirname "$0")/.."

# grep that treats "no match" as success but a real error as failure.
g() { grep "$@" || [ $? -eq 1 ]; }

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

# L46: reads changed paths on stdin; prints `code` when one is product code (a Code PR), `ui` when one is under
# apps/desktop/src, and `loop` when one is loop machinery the user merges by hand. The one definition of each.
touches() {
  local path code="" ui="" loop=""
  while IFS= read -r path || [ -n "$path" ]; do
    case $path in apps/* | crates/* | contracts/*) code=1 ;; esac
    case $path in apps/desktop/src/*) ui=1 ;; esac
    case $path in loop/* | .github/* | .agents/* | .claude/* | AGENTS.md | */AGENTS.md | CLAUDE.md | */CLAUDE.md) loop=1 ;; esac
  done
  [ -z "$code" ] || echo code
  [ -z "$ui" ] || echo ui
  [ -z "$loop" ] || echo loop
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

# L54: a clean merge of main: two parents, the second on main, no conflict, and nothing beyond the trial merge.
# It adds no authored content, so it needs no Author-Agent, and an approve on its first parent still covers it.
clean_merge() {
  local c=$1 main_ref=$2 p1 p2 extra trial conflicted f
  read -r _ p1 p2 extra <<<"$(git rev-list --parents -n 1 "$c")"
  { [ -n "${p2:-}" ] && [ -z "${extra:-}" ]; } || { echo "clean-merge: $c: not a merge of main" >&2; return 1; }
  git merge-base --is-ancestor "$p2" "$main_ref" || { echo "clean-merge: $c: not a merge of main (second parent is not on $main_ref)" >&2; return 1; }
  local mt_rc=0
  trial=$(git merge-tree --write-tree --name-only --no-messages "$p1" "$p2" 2>/dev/null) || mt_rc=$?
  { [ "$mt_rc" -le 1 ] && [ -n "$trial" ]; } || { echo "clean-merge: $c: merge-tree failed (exit $mt_rc)" >&2; return 1; }
  conflicted=$(sed 1d <<<"$trial" | sed '/^$/d')
  trial=$(head -n 1 <<<"$trial")
  f=$(head -n 1 <<<"$conflicted")
  [ -z "$f" ] || { echo "clean-merge: $c: rule a: a conflict in $f" >&2; return 1; }
  local differs
  differs=$(git diff --name-only "$trial" "$c") || { echo "clean-merge: $c: could not compare with the trial merge" >&2; return 1; }
  f=$(head -n 1 <<<"$differs")
  [ -z "$f" ] || { echo "clean-merge: $c: rule c: $f differs from the trial merge" >&2; return 1; }
}

# PR trees are data: never check them out or execute their copy of this script.
pr_rule() {
  python3 - "$1" "$2" <<'PY'
import json
from datetime import datetime
import os
import re
import subprocess
import sys
import tempfile

command, number = sys.argv[1:]

class Refused(Exception):
    pass

def require(ok, reason):
    if not ok:
        raise Refused(reason)

try:
    limit = float(os.environ.get('LOOP_GH_TIMEOUT', '20'))
    require(limit > 0, 'LOOP_GH_TIMEOUT must be positive')
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

def present(value):
    return subprocess.run(['git', 'cat-file', '-e', value+'^{commit}'], stderr=subprocess.DEVNULL).returncode == 0

def ancestor(older, newer):
    result = subprocess.run(['git', 'merge-base', '--is-ancestor', older, newer], timeout=limit)
    require(result.returncode in (0, 1), f'cannot compare {older} to {newer}')
    return result.returncode == 0

# A SHA this repo does not hold is in no PR history: a rewritten branch dropped it.
def reaches(older, newer):
    return present(older) and present(newer) and ancestor(older, newer)

def sha(value):
    require(isinstance(value, str) and re.fullmatch(r'[0-9a-f]{40}', value), 'missing or invalid SHA')
    return value

def fetch(value):
    if not present(value):
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

COPILOT = 'Co-authored-by: Copilot Autofix powered by AI <62310815+github-advanced-security[bot]@users.noreply.github.com>'

# L37: a Copilot Autofix commit accepted on GitHub is authored by `copilot`: it carries Copilot's trailer, GitHub
# committed it, and its signature verifies against GitHub's web-flow key in loop/web-flow.asc and no other key.
def copilot(commit):
    message = git('show', '-s', '--format=%B', commit)
    if COPILOT not in message.splitlines() or git('show', '-s', '--format=%ce', commit) != 'noreply@github.com':
        return False
    with tempfile.TemporaryDirectory() as home:
        env = {**os.environ, 'GNUPGHOME': home}
        imported = subprocess.run(['gpg', '--batch', '--quiet', '--import', 'loop/web-flow.asc'], env=env,
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=limit)
        return imported.returncode == 0 and subprocess.run(
            ['git', '-c', 'gpg.program=gpg', '-c', 'gpg.ssh.allowedSignersFile=/dev/null', 'verify-commit', commit], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            timeout=limit).returncode == 0

def history(base, head):
    commits = git('rev-list', '--reverse', base+'..'+head).splitlines()
    require(commits, 'no PR commits')
    authors, records = set(), []
    for commit in commits:
        parents = git('rev-list', '--parents', '-n', '1', commit).split()[1:]
        author, reviewer = trailers(commit, 'Author-Agent'), trailers(commit, 'Reviewed-by-Agent')
        if not author and not reviewer and len(parents) == 1 and copilot(commit):
            author = {'copilot'}
        authors.update(author)
        records.append((commit, parents, author, reviewer))
    require(authors, 'no Author-Agent trailers')
    return authors, records

def clean_merge(commit, base):
    try:
        return subprocess.run(['bash', 'loop/rules.sh', 'clean-merge', commit, base], stdout=subprocess.DEVNULL,
                              stderr=subprocess.DEVNULL, timeout=limit).returncode == 0
    except subprocess.TimeoutExpired:
        return False

# L2: an approval is a VERDICT comment, never a commit.
def trailer_gate(records, base):
    for commit, parents, author, reviewer in records:
        require(not reviewer, f'trailers: {commit} carries Reviewed-by-Agent; an approval is a VERDICT comment')
        if len(parents) == 1:
            require(author, f'trailers: {commit} has no Author-Agent')
        else:
            require(author or clean_merge(commit, base), f'trailers: merge {commit} needs Author-Agent or must be a clean merge of main')

# The named SHA is the head, or the head adds only clean merges of main on top of it (L54).
def covers(named, head, base):
    if named == head:
        return True
    if not reaches(named, head):
        return False
    for commit in git('rev-list', '--first-parent', named+'..'+head).splitlines():
        if len(git('rev-list', '--parents', '-n', '1', commit).split()) != 3 or not clean_merge(commit, base):
            return False
    return True

# Only a workflow on main or a person with write access can post a verdict; anyone can comment on a public repo.
TRUSTED = {'OWNER', 'MEMBER', 'COLLABORATOR'}

def verdicts(authors, head, base):
    data = listing(f'{repo}/issues/{number}/comments?per_page=100')
    require(all(isinstance(c.get('body'), str) and isinstance(c.get('created_at'), str)
                and isinstance(c.get('id'), int) and isinstance(c.get('user'), dict) for c in data), 'invalid comment data')
    for item in data:
        require(re.fullmatch(r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z', item['created_at']), 'invalid comment time')
        datetime.fromisoformat(item['created_at'].replace('Z', '+00:00'))
    events = []
    for item in sorted(data, key=lambda c: (c['created_at'], c['id'])):
        if item['user'].get('login') != 'github-actions[bot]' and item.get('author_association') not in TRUSTED:
            continue
        body = item['body']
        verdict = re.match(r'VERDICT: (approve|reject)\b', body)
        ids = re.findall(r'^Reviewed-by-Agent: ([A-Za-z0-9_.-]+)\s*$', body, re.M)
        if not verdict or len(ids) != 1 or ids[0] in authors | {'copilot'}:
            continue
        named = set(re.findall(r'^Head: ([0-9a-f]{40})\s*$', body, re.M)) or \
            set(re.findall(r'(?<![0-9a-f])[0-9a-f]{40}(?![0-9a-f])', body))
        named_head = next(iter(named)) if len(named) == 1 else None
        events.append({'verdict': verdict[1], 'head': named_head, 'current': bool(named_head) and covers(named_head, head, base),
                       'at': item['created_at'], 'agent': ids[0], 'body': body})
    return events

# A reject blocks until a later approve names a head holding the rejected one; a reject naming no single SHA blocks
# until any later approve that names one.
def rejected(events, head):
    blocking = set()
    for event in events:
        if event['verdict'] == 'reject':
            blocking.add(event['head'])
        elif event['head']:
            blocking = {held for held in blocking if held is not None and not reaches(held, event['head'])}
    return {held for held in blocking if held is None or reaches(held, head)}

def changed(base, head):
    files = listing(pull+'/files?per_page=100')
    require(all(isinstance(f.get('filename'), str) for f in files), 'invalid changed files')
    paths = git('diff', '--name-only', '--no-renames', '-z', base+'...'+head).split('\0')
    paths = set(filter(None, paths))
    reported = {p for f in files for p in (f['filename'], f.get('previous_filename')) if p}
    require(paths == reported, 'incomplete or changed PR file list')
    return paths

def touches(paths):
    result = subprocess.run(['bash', 'loop/rules.sh', 'touches'], input='\n'.join(sorted(paths))+'\n',
                            text=True, stdout=subprocess.PIPE, timeout=limit, check=True)
    return set(result.stdout.split())

def checks(head, names):
    pages = gh(f'{repo}/commits/{head}/check-runs?per_page=100&filter=latest')
    require(all(isinstance(page, dict) and isinstance(page.get('check_runs'), list) for page in pages), 'invalid check runs')
    runs = [run for page in pages for run in page['check_runs']]
    require(all(isinstance(run, dict) for run in runs), 'invalid check run')
    for name in names:
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
    authors, records = history(base, head)
    if command == 'ci-trailers':
        base_gate(pr)
        trailer_gate(records, base)
        sys.exit(0)
    events = verdicts(authors, head, base)
    if command == 'verdicts':
        for event in events:
            print(json.dumps(event))
        sys.exit(0)
    paths = changed(base, head)
    touched = touches(paths)
    body = pr['body'] if isinstance(pr.get('body'), str) else ''
    errors = []
    def gate(call):
        try:
            call()
        except Refused as error:
            errors.append(str(error))
    gate(lambda: base_gate(pr))
    gate(lambda: require(pr['state'] == 'open' and pr['draft'] is False, 'PR is closed or draft'))
    gate(lambda: require('loop' not in touched, 'the user merges a PR touching loop/, .github/, .agents/, .claude/, AGENTS.md or CLAUDE.md'))
    gate(lambda: checks(head, ('check', 'rules') + (('percy',) if 'ui' in touched else ())))
    gate(lambda: trailer_gate(records, base))
    gate(lambda: require(sum(event['verdict'] == 'reject' for event in events) < 2, 'second reject: the user decides'))
    gate(lambda: require(not rejected(events, head), 'newest independent verdict on this head is reject'))
    if 'code' in touched:
        gate(lambda: require(re.search(r'^Scenarios:.*\b[A-Z][0-9]+\b', body, re.M), 'a Code PR needs a Scenarios: line'))
        gate(lambda: require(any(event['verdict'] == 'approve' and event['current'] for event in events),
                             'a Code PR needs an independent VERDICT: approve naming its head'))
    if any(path.startswith('crates/desktop/') for path in paths):
        gate(lambda: require(re.search(r'^macOS: \S.+$', body, re.M), 'a crates/desktop PR needs a macOS: line'))
    latest = one(pull)
    gate(lambda: require(latest == pr, 'PR changed during merge-ready'))
    require(not errors, '\n'.join(errors))
    print('ready '+head)
except (Refused, KeyError, TypeError, ValueError, OSError, subprocess.SubprocessError) as error:
    print(f'{command}: {error}', file=sys.stderr)
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
                                text=True, stdout=subprocess.PIPE, timeout=float(os.environ.get('LOOP_GH_TIMEOUT', '20')), check=True)
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
  base | ci-trailers | verdicts | merge-ready)
    [[ ${2:-} =~ ^[0-9]+$ ]] || { echo "usage: loop/rules.sh $1 <pr>" >&2; exit 2; }
    pr_rule "$1" "$2" ;;
  touches) touches ;;
  vocab) vocab ;;
  ready)
    [[ -z ${2:-} || $2 == --offline ]] || { echo "usage: loop/rules.sh ready [--offline]" >&2; exit 2; }
    ready "${@:2}" ;;
  delta) delta "${2:-}" "${3:-}" ;;
  clean-merge) [ -n "${2:-}" ] || { echo "usage: loop/rules.sh clean-merge <commit> [main-ref]" >&2; exit 2; }
    clean_merge "$2" "${3:-origin/main}" ;;
  *) sed -n '2p' "$0" >&2; exit 2 ;;
esac
