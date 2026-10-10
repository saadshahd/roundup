#!/usr/bin/env bash
# Machine checks for AGENTS.md. Usage: loop/rules.sh vocab | tokens | delta <base-dir> <head-dir> | base|ci-trailers <pr> | architect-words | clean-merge <commit> [main-ref]
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

# L95: the role word "architect" in scenarios/ or docs/ fails as `<file>:<line>`; a branch `architect/…`, a host `architect-swarm|-b|-c` and the L95 block are exempt.
architect_words() {
  git rev-parse --is-inside-work-tree >/dev/null 2>&1 || { echo "architect-words: not a git checkout" >&2; return 2; }
  python3 -I - <<'PY'
import os
import re
import sys

WORD = re.compile(r'\b[Aa]rchitects?\b')
EXEMPT = re.compile(r'/|-(swarm|b|c)(?![\w-])')
bad = 0
for root in ('scenarios', 'docs'):
    for dirpath, _, names in os.walk(root):
        for name in sorted(names):
            path = os.path.join(dirpath, name)
            try:
                lines = open(path, encoding='utf-8').read().split('\n')
            except (UnicodeDecodeError, OSError):
                continue
            in_block = False
            for number, line in enumerate(lines, 1):
                if line.startswith('**L95 '):
                    in_block = True
                if in_block:
                    in_block = not line.startswith('Tests:')
                    continue
                for match in WORD.finditer(line):
                    if not EXEMPT.match(line, match.end()):
                        print(f'{path}:{number}')
                        bad = 1
                        break
sys.exit(bad)
PY
}

# L41: a look literal (colour, size, radius, shadow, duration) outside tokens.css fails as `<file>:<line> <property>`.
tokens() {
  python3 - <<'PY'
import os
import re
import sys

ROOT = 'apps/desktop/src'
NAMES = set('''aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond blue blueviolet brown burlywood
cadetblue chartreuse chocolate coral cornflowerblue cornsilk crimson cyan darkblue darkcyan darkgoldenrod darkgray darkgreen
darkgrey darkkhaki darkmagenta darkolivegreen darkorange darkorchid darkred darksalmon darkseagreen darkslateblue darkslategray
darkslategrey darkturquoise darkviolet deeppink deepskyblue dimgray dimgrey dodgerblue firebrick floralwhite forestgreen fuchsia
gainsboro ghostwhite gold goldenrod gray green greenyellow grey honeydew hotpink indianred indigo ivory khaki lavender
lavenderblush lawngreen lemonchiffon lightblue lightcoral lightcyan lightgoldenrodyellow lightgray lightgreen lightgrey lightpink
lightsalmon lightseagreen lightskyblue lightslategray lightslategrey lightsteelblue lightyellow lime limegreen linen magenta
maroon mediumaquamarine mediumblue mediumorchid mediumpurple mediumseagreen mediumslateblue mediumspringgreen mediumturquoise
mediumvioletred midnightblue mintcream mistyrose moccasin navajowhite navy oldlace olive olivedrab orange orangered orchid
palegoldenrod palegreen paleturquoise palevioletred papayawhip peachpuff peru pink plum powderblue purple rebeccapurple red
rosybrown royalblue saddlebrown salmon sandybrown seagreen seashell sienna silver skyblue slateblue slategray slategrey snow
springgreen steelblue tan teal thistle tomato turquoise violet wheat white whitesmoke yellow yellowgreen'''.split())
COLOUR = re.compile(r'^(color|background|background-color|border.*|outline.*|fill|stroke|caret-color|text-decoration-color)$')
TS_COLOUR = {'color', 'background', 'backgroundColor', 'borderColor'}
TS_KEYS = TS_COLOUR | {'fontSize', 'borderRadius', 'boxShadow'}
HEX = re.compile(r'#[0-9a-fA-F]{3,8}\b')
FUNC = re.compile(r'(?<![\w-])(rgb|rgba|hsl|hsla|oklch|color-mix)\(', re.I)
UNIT = re.compile(r'(?<![\w.#-])[+-]?(\d*\.?\d+)([a-z%]+)', re.I)
DURATION = re.compile(r'(?<![\w.#-])(\d*\.?\d+)(ms|s)\b', re.I)
BARE = re.compile(r'(?<![\w#-])[A-Za-z]+(?![\w(-])')


def blank(text, slashes):
    # Comments become spaces; newlines and strings stay.
    out, i, n, quote = [], 0, len(text), None
    while i < n:
        c = text[i]
        if quote:
            out.append(c)
            if c == '\\' and i + 1 < n:
                out.append(text[i + 1])
                i += 1
            elif c == quote:
                quote = None
        elif c in '"\'`':
            quote = c
            out.append(c)
        elif text.startswith('/*', i):
            j = text.find('*/', i + 2)
            j = n if j < 0 else j + 2
            out.append(re.sub(r'[^\n]', ' ', text[i:j]))
            i = j
            continue
        elif slashes and text.startswith('//', i):
            j = text.find('\n', i)
            j = n if j < 0 else j
            out.append(' ' * (j - i))
            i = j
            continue
        else:
            out.append(c)
        i += 1
    return ''.join(out)


def strip_calls(value, names):
    # Remove name(...) calls, balanced.
    while True:
        m = re.search(r'(?<![\w-])(%s)\(' % '|'.join(names), value, re.I)
        if not m:
            return value
        depth, j = 1, m.end()
        while j < len(value) and depth:
            depth += {'(': 1, ')': -1}.get(value[j], 0)
            j += 1
        value = value[:m.start()] + ' ' + value[j:]


def has_colour(v):
    if HEX.search(v) or FUNC.search(v):
        return True
    return any(w.lower() in NAMES for w in BARE.findall(v))


def nonzero(v, pattern):
    return any(float(m.group(1)) != 0 for m in pattern.finditer(v))


def bad(prop, value, ts):
    raw = value
    v = strip_calls(strip_calls(value, ['url']), ['var'])
    if prop in ('fontSize', 'borderRadius', 'font-size', 'border-radius'):
        if ts and re.fullmatch(r'\s*[+-]?\d*\.?\d+\s*', v) and float(v) != 0:
            return True
        return nonzero(v, UNIT)
    if prop in ('boxShadow', 'box-shadow'):
        if has_colour(v):
            return True
        return 'var(' not in raw and v.strip().strip('"\'`').lower() not in ('none', 'inherit', 'initial', 'unset', '')
    if prop.startswith(('transition', 'animation')) and not ts:
        return nonzero(v, DURATION)
    if (ts and prop in TS_COLOUR) or (not ts and COLOUR.match(prop)):
        return has_colour(v)
    return False


def line_of(text, pos):
    return text.count('\n', 0, pos) + 1


def css(text):
    for m in re.finditer(r'(?<![\w-])([a-zA-Z-]+)\s*:\s*([^;{}]*)', text):
        if bad(m.group(1).lower(), m.group(2), False):
            yield line_of(text, m.start(1)), m.group(1)


def style_objects(text):
    for m in re.finditer(r'style\s*=\s*\{\s*\{', text):
        depth, j, quote = 2, m.end(), None
        while j < len(text) and depth > 1:
            c = text[j]
            if quote:
                if c == '\\':
                    j += 1
                elif c == quote:
                    quote = None
            elif c in '"\'`':
                quote = c
            elif c == '{':
                depth += 1
            elif c == '}':
                depth -= 1
            j += 1
        yield m.end(), text[m.end():j - 1]


def ts(text):
    for start, body in style_objects(text):
        # Entries `key: value` at the object's top level.
        depth, quote, i, entries, last = 0, None, 0, [], 0
        while i <= len(body):
            c = body[i] if i < len(body) else ','
            if quote:
                if c == '\\':
                    i += 1
                elif c == quote:
                    quote = None
            elif c in '"\'`':
                quote = c
            elif c in '([{':
                depth += 1
            elif c in ')]}':
                depth -= 1
            elif c == ',' and depth == 0:
                entries.append((last, body[last:i]))
                last = i + 1
            i += 1
        for off, entry in entries:
            m = re.match(r'\s*["\']?([A-Za-z]+)["\']?\s*:\s*(.*)', entry, re.S)
            if m and m.group(1) in TS_KEYS and bad(m.group(1), m.group(2), True):
                yield line_of(text, start + off + m.start(1)), m.group(1)


def skipped(path):
    rel = os.path.relpath(path, ROOT).split(os.sep)
    return (rel == ['tokens.css'] or rel == ['terminal', 'emulator.ts'] or 'testing' in rel[:-1]
            or '.test.' in rel[-1])


found = []
for dirpath, dirs, files in os.walk(ROOT):
    dirs.sort()
    for name in sorted(files):
        path = os.path.join(dirpath, name)
        if skipped(path) or not name.endswith(('.css', '.ts', '.tsx')):
            continue
        with open(path, encoding='utf-8') as f:
            text = f.read()
        hits = css(blank(text, False)) if name.endswith('.css') else ts(blank(text, True))
        found += [f'{path}:{line} {prop}' for line, prop in hits]
if found:
    print('\n'.join(found))
    sys.exit(1)
PY
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

repo = 'repos/{owner}/{repo}'
pull = f'{repo}/pulls/{number}'

def git(*args):
    return subprocess.check_output(['git', *args], text=True, timeout=limit).rstrip('\n')

def present(value):
    return subprocess.run(['git', 'cat-file', '-e', value+'^{commit}'], stderr=subprocess.DEVNULL).returncode == 0

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
    return records

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

try:
    pr = one(pull)
    base_gate(pr)
    if command == 'ci-trailers':
        head, base = sha(pr['head']['sha']), sha(pr['base']['sha'])
        fetch(head)
        fetch(base)
        trailer_gate(history(base, head), base)
except (Refused, KeyError, TypeError, ValueError, OSError, subprocess.SubprocessError) as error:
    print(f'{command}: {error}', file=sys.stderr)
    sys.exit(1)
PY
}

case "${1:-}" in
  base | ci-trailers)
    [[ ${2:-} =~ ^[0-9]+$ ]] || { echo "usage: loop/rules.sh $1 <pr>" >&2; exit 2; }
    pr_rule "$1" "$2" ;;
  vocab) vocab ;;
  architect-words) [ $# -eq 1 ] || { echo "usage: loop/rules.sh architect-words" >&2; exit 2; }
    architect_words ;;
  tokens) [ $# -eq 1 ] || { echo "usage: loop/rules.sh tokens" >&2; exit 2; }
    tokens ;;
  delta) delta "${2:-}" "${3:-}" ;;
  clean-merge) [ -n "${2:-}" ] || { echo "usage: loop/rules.sh clean-merge <commit> [main-ref]" >&2; exit 2; }
    clean_merge "$2" "${3:-origin/main}" ;;
  *) sed -n '2p' "$0" >&2; exit 2 ;;
esac
