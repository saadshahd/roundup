#!/usr/bin/env bash
# L28: detect, own and report each Stall. Usage: loop/stalls.sh check | report
# `check` writes loop/out/stalls/<kind>-<subject> for each Stall that holds, deletes the rest, and exits 1 naming the files.
# Verdicts, round caps and readiness come from loop/rules.sh, never a second parser. `report` prints each file once.
set -euo pipefail
cd "$(dirname "$0")/.."

case "${1:-}" in
  check | report) ;;
  *) sed -n '2p' "$0" >&2; exit 2 ;;
esac

exec python3 - "$1" <<'PY'
import json
import os
import re
import subprocess
import sys
from datetime import datetime, timedelta, timezone

command = sys.argv[1]
stalls = 'loop/out/stalls'
reported = 'loop/out/stalls-reported'
stamp = '%Y-%m-%dT%H:%M:%SZ'

def fields(path):
    with open(path) as file:
        return dict(line.rstrip('\n').split('=', 1) for line in file)

def report():
    names = sorted(os.listdir(stalls)) if os.path.isdir(stalls) else []
    os.makedirs(reported, exist_ok=True)
    for name in os.listdir(reported):
        if name not in names:
            os.remove(f'{reported}/{name}')
    for name in names:
        held = fields(f'{stalls}/{name}')
        mark = f'{reported}/{name}'
        if os.path.exists(mark) and open(mark).read() == held['since']:
            continue
        kind, subject = name.split('-', 1)
        print(kind, subject, 'owner', held['owner'], 'due', held['deadline'])
        with open(mark, 'w') as file:
            file.write(held['since'])

if command == 'report':
    report()
    sys.exit(0)

def refuse(why):
    print(f'stalls: {why}', file=sys.stderr)
    sys.exit(4)

try:
    limit = int(os.environ.get('BOXD_GH_TIMEOUT', '20'))
    if limit <= 0:
        raise ValueError
except ValueError:
    refuse('gh: BOXD_GH_TIMEOUT must be a positive integer')

def run(argv, what):
    try:
        return subprocess.run(argv, text=True, capture_output=True, timeout=limit)
    except (subprocess.SubprocessError, OSError) as error:
        refuse(f'{what}: {error}')

def gh(route):
    done = run(['gh', 'api', route, '--paginate', '--slurp'], f'gh: {route}')
    if done.returncode:
        refuse(f'gh: {route}: {done.stderr.strip()}')
    try:
        return [item for page in json.loads(done.stdout) for item in (page if isinstance(page, list) else [page])]
    except (ValueError, TypeError):
        refuse(f'gh: {route}: invalid JSON')

def rules(*args):
    done = run(['bash', 'loop/rules.sh', *args], 'loop/rules.sh ' + ' '.join(args))
    if done.returncode not in (0, 1):
        refuse(f'loop/rules.sh {" ".join(args)} exited {done.returncode}: {done.stderr.strip()}')
    return done

def verdicts(number):
    done = rules('verdicts', number)
    if done.returncode:
        refuse(f'loop/rules.sh verdicts {number}: {done.stderr.strip()}')
    return [line.split() for line in done.stdout.splitlines()]

def when(text):
    return datetime.strptime(text, stamp).replace(tzinfo=timezone.utc)

now = datetime.now(timezone.utc)
holding = {}

# a: red main.
runs = [run_ for item in gh('repos/{owner}/{repo}/commits/main/check-runs?per_page=100&filter=latest')
        for run_ in item.get('check_runs', []) if run_.get('name') in ('check', 'rules') and run_.get('status') == 'completed']
if runs and max(runs, key=lambda r: r['completed_at']).get('conclusion') != 'success':
    holding['a-main'] = 'Triage'

# Open PRs: b, c and d.
for pr in gh('repos/{owner}/{repo}/pulls?state=open&per_page=100'):
    number, head = str(pr['number']), pr['head']['sha']
    ready = rules('merge-ready', number)
    if ready.returncode == 0 and ready.stdout.strip() == f'ready {head}':
        approves = [v[2] for v in verdicts(number) if v[0] == 'approve']
        if approves and now - when(max(approves)) >= timedelta(minutes=20):
            holding[f'b-{number}'] = 'Merger'
    capped = rules('rounds', number)
    if capped.returncode and 'round cap' in capped.stderr:
        messages = [c['commit']['message'] for c in gh(f'repos/{{owner}}/{{repo}}/pulls/{number}/commits?per_page=100')]
        authors = sorted({a for m in messages for a in re.findall(r'^Author-Agent: (\S+)\s*$', m, re.M)})
        holding[f'c-{number}'] = 'Architect except ' + ','.join(authors)
    lost = False
    if os.path.isdir('loop/out/verdicts'):
        for name in os.listdir('loop/out/verdicts'):
            path = f'loop/out/verdicts/{name}'
            old = now.timestamp() - os.path.getmtime(path) >= 300
            lost = lost or (old and head in open(path, errors='replace').read())
    if lost and all(v[1] != head for v in verdicts(number)):
        holding[f'd-{number}'] = 'Driver'

# e: a post-lane PR merged after this script landed and still without a verdict on its head 60 minutes later.
added = run(['git', 'log', '--diff-filter=A', '--format=%ct', '--', 'loop/stalls.sh'], 'git log').stdout.split()
landed = datetime.fromtimestamp(int(added[-1]), timezone.utc) if added else None
for pr in gh('repos/{owner}/{repo}/pulls?state=closed&sort=updated&direction=desc&per_page=100'):
    if not (landed and pr.get('merged_at')) or when(pr['merged_at']) < landed:
        continue
    if now - when(pr['merged_at']) >= timedelta(minutes=60) and rules('class', str(pr['number'])).stdout.strip() == 'post':
        if all(v[1] != pr['head']['sha'] for v in verdicts(str(pr['number']))):
            holding[f'e-{pr["number"]}'] = 'Driver'

# Every read is done: change the files.
os.makedirs(stalls, exist_ok=True)
for name in os.listdir(stalls):
    if name not in holding:
        os.remove(f'{stalls}/{name}')
        if os.path.exists(f'{reported}/{name}'):
            os.remove(f'{reported}/{name}')
for name, owner in holding.items():
    path = f'{stalls}/{name}'
    if not os.path.exists(path):
        with open(path, 'w') as file:
            file.write(f'since={now.strftime(stamp)}\nowner={owner}\ndeadline={(now + timedelta(minutes=30)).strftime(stamp)}\n')
if holding:
    print('stalls: ' + ' '.join(f'{stalls}/{name}' for name in sorted(holding)), file=sys.stderr)
    sys.exit(1)
PY
