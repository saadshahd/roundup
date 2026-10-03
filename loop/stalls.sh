#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [ "${1:-}" != check ] || [ "$#" -ne 1 ]; then
  echo 'usage: loop/stalls.sh check' >&2
  exit 2
fi
python3 - <<'PY'
import datetime as dt
import json
import os
import pathlib
import re
import subprocess
import sys
import tempfile

UTC = dt.timezone.utc
ROOT = pathlib.Path('loop/out')
STALLS = ROOT / 'stalls'
VERDICTS = ROOT / 'verdicts'
SHA = re.compile(r'(?<![0-9a-f])[0-9a-f]{40}(?![0-9a-f])')
AUTHOR = re.compile(r'^Author-Agent: ([A-Za-z0-9_.-]+)$', re.M)
REVIEWER = re.compile(r'^Reviewed-by-Agent: ([A-Za-z0-9_.-]+)[ \t]*$', re.M)
RED = {'failure', 'timed_out', 'cancelled', 'action_required', 'startup_failure', 'stale'}


def when(value):
    return dt.datetime.fromisoformat(value.replace('Z', '+00:00')).astimezone(UTC)


def stamp(value):
    return value.astimezone(UTC).strftime('%Y-%m-%dT%H:%M:%SZ')


def call(tool, *args, allow_unprotected=False):
    try:
        result = subprocess.run([tool, *args], capture_output=True, text=True,
                                timeout=int(os.environ.get('BOXD_GH_TIMEOUT', '20')), check=True)
        return json.loads(result.stdout)
    except subprocess.CalledProcessError as error:
        if allow_unprotected and 'Branch not protected' in error.stderr and '404' in error.stderr:
            return None
        print(f'stalls: {tool} failed: {error}', file=sys.stderr)
        raise SystemExit(4) from error
    except (subprocess.TimeoutExpired, OSError, ValueError) as error:
        print(f'stalls: {tool} failed: {error}', file=sys.stderr)
        raise SystemExit(4) from error


def pages(value):
    if not isinstance(value, list):
        raise ValueError('expected paginated array')
    return [item for page in value for item in (page if isinstance(page, list) else [page])]


def check_runs(value):
    return [run for page in value for run in page['check_runs']]


def failed_names(runs, required):
    latest = {}
    for run in runs:
        name = run['name']
        order = (run.get('started_at') or run.get('completed_at') or '', run['id'])
        if name in required and (name not in latest or order > latest[name][0]):
            latest[name] = (order, run['conclusion'])
    return {name for name, (_, conclusion) in latest.items() if conclusion in RED}


def reviewed_head(body):
    explicit = re.findall(r'^(?:Reviewed[- ]head|Head(?: reviewed)?): ([0-9a-f]{40})[ \t]*$', body, re.M)
    if explicit:
        return explicit[0] if len(explicit) == 1 else None
    for line in body.splitlines():
        if line.lstrip().startswith('Diff-base:'):
            continue
        found = SHA.search(line)
        if found:
            return found.group()
    return None


def reviewer_id(body):
    ids = REVIEWER.findall(body)
    return ids[0] if len(ids) == 1 else None


def verdicts(comments, authors, reviewable_heads):
    valid = []
    for comment in comments:
        body = comment['body']
        kind = re.match(r'^VERDICT: (approve|reject)\b', body)
        reviewer = reviewer_id(body)
        if kind and reviewed_head(body) in reviewable_heads and reviewer and reviewer not in authors:
            valid.append(kind.group(1))
    return valid


def existing_since(path):
    if not path.exists():
        return now
    lines = dict(line.split('=', 1) for line in path.read_text().splitlines() if '=' in line)
    return when(lines['since'])


def record(kind, subject, owner, extra=None):
    name = f'{kind}-{subject}'
    since = existing_since(STALLS / name)
    data = f'since={stamp(since)}\nowner={owner}\ndeadline={stamp(since + dt.timedelta(minutes=30))}\n'
    if extra:
        data += f'except={extra}\n'
    desired[name] = data


try:
    now = when(os.environ.get('L28_NOW', stamp(dt.datetime.now(UTC))))
    repo = call('gh', 'repo', 'view', '--json', 'nameWithOwner')['nameWithOwner']
    required_data = call('gh', 'api', f'repos/{repo}/branches/main/protection/required_status_checks', allow_unprotected=True)
    required = (set(required_data['contexts']) | {check['context'] for check in required_data['checks']}) if required_data else {'check'}
    if not required:
        raise ValueError('main has no required checks')
    main_runs = check_runs(call('gh', 'api', f'repos/{repo}/commits/main/check-runs', '--paginate', '--slurp'))
    pulls = pages(call('gh', 'api', f'repos/{repo}/pulls?state=open&per_page=100', '--paginate', '--slurp'))
    pr_data = []
    for pr in pulls:
        number = pr['number']
        comments = pages(call('gh', 'api', f'repos/{repo}/issues/{number}/comments?per_page=100', '--paginate', '--slurp'))
        commits = pages(call('gh', 'api', f'repos/{repo}/pulls/{number}/commits?per_page=100', '--paginate', '--slurp'))
        runs = check_runs(call('gh', 'api', f'repos/{repo}/commits/{pr["head"]["sha"]}/check-runs', '--paginate', '--slurp'))
        author_ids = [author for commit in commits for author in AUTHOR.findall(commit['commit']['message'])]
        known_heads = {pr['head']['sha']} | {commit['sha'] for commit in commits if 'sha' in commit}
        reviewable_heads = {pr['head']['sha']}
        commits_by_sha = {commit['sha']: commit for commit in commits if 'sha' in commit}
        approval = commits_by_sha.get(pr['head']['sha'])
        if approval:
            message = approval['commit']['message']
            parents = approval.get('parents', [])
            if reviewer_id(message) and not AUTHOR.search(message) and len(parents) == 1:
                parent = commits_by_sha.get(parents[0]['sha'])
                if parent and approval['commit']['tree']['sha'] == parent['commit']['tree']['sha']:
                    reviewable_heads.add(parents[0]['sha'])
        pr_data.append((pr, comments, set(author_ids), author_ids[0] if author_ids else pr['user']['login'], known_heads, reviewable_heads, failed_names(runs, required)))
    machines = call('boxd', 'machine', 'list', '--json')
    vm_count = sum(machine['name'].startswith('ru-') for machine in machines)
    verdict_files = [(path.read_text(), dt.datetime.fromtimestamp(path.stat().st_mtime, UTC))
                     for path in VERDICTS.iterdir() if path.is_file()] if VERDICTS.exists() else []
    desired = {}
    if failed_names(main_runs, required):
        record('a', 'main', 'Triage')
    if pr_data and any(all(name in failed for _, _, _, _, _, _, failed in pr_data) for name in required):
        record('a', 'all-prs', 'Triage')
    for pr, comments, authors, author_id, known_heads, reviewable_heads, _ in pr_data:
        number = pr['number']
        head = pr['head']['sha']
        valid = verdicts(comments, authors, reviewable_heads)
        if valid and valid[-1] == 'approve' and now - when(pr['created_at']) >= dt.timedelta(minutes=20):
            record('b', number, 'Merger')
        all_rejects = 0
        for comment in comments:
            body = comment['body']
            if re.match(r'^VERDICT: reject\b', body):
                reviewer = reviewer_id(body)
                if reviewer and reviewer not in authors and reviewed_head(body) in known_heads:
                    all_rejects += 1
        if all_rejects >= 3:
            record('c', number, 'Architect', author_id)
        has_verdict = any(head in SHA.findall(comment['body']) and
                          any(line.startswith('VERDICT:') for line in comment['body'].splitlines())
                          for comment in comments)
        if not has_verdict and any(head in SHA.findall(body) and now - modified >= dt.timedelta(minutes=5)
                                   for body, modified in verdict_files):
            record('d', number, 'Driver')
        if not valid and vm_count < min(8, int(os.environ.get('BOXD_MAX_VMS', '12'))):
            record('e', number, 'Driver')
    if STALLS.exists():
        for path in STALLS.iterdir():
            if path.is_file() and re.fullmatch(r'[a-e]-(?:main|all-prs|[0-9]+)', path.name):
                existing_since(path)
    STALLS.mkdir(parents=True, exist_ok=True)
    for name, data in desired.items():
        path = STALLS / name
        if not path.exists() or path.read_text() != data:
            with tempfile.NamedTemporaryFile('w', dir=STALLS, delete=False) as temporary:
                temporary.write(data)
            os.replace(temporary.name, path)
    for path in STALLS.iterdir():
        if path.is_file() and re.fullmatch(r'[a-e]-(?:main|all-prs|[0-9]+)', path.name) and path.name not in desired:
            path.unlink()
    if desired:
        print('\n'.join(sorted(desired)))
        raise SystemExit(1)
except (KeyError, TypeError, ValueError, OSError) as error:
    print(f'stalls: invalid input: {error}', file=sys.stderr)
    raise SystemExit(4) from error
PY
