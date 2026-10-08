#!/usr/bin/env python3
"""L88: reconcile PR work; a durable per-PR Claim owns review and repair together."""
import json
import os
import subprocess
import sys


def run(*args, data=None, timeout=120):
    return subprocess.run(args, input=data, text=True, capture_output=True, check=True, timeout=timeout).stdout.strip()


def gh(*args, data=None):
    return json.loads(run('gh', *args, data=data))


def api(path, payload=None):
    return gh('api', 'repos/{owner}/{repo}/' + path, *(['--input', '-'] if payload is not None else []),
              data=json.dumps(payload) if payload is not None else None)


def active_builds():
    return set(run('bash', 'loop/runs.sh', 'builders').splitlines())


def local_branch():
    import time
    status = json.loads(os.environ.get('CODEX_STATUS') or '{}')
    if status.get('state') == 'running' and status.get('deadline', 0) > time.time():
        return 'build/' + status['slug']
    return None


def eligible(pr):
    return pr['state'] == 'open' and pr['base']['ref'] == 'main' and bool(pr['head'].get('repo')) and pr['head']['repo']['full_name'] == pr['base']['repo']['full_name']


def decision(pr, checks, verdicts, kinds):
    """Choose from current evidence, without a dispatch side effect."""
    if not eligible(pr):
        return None
    can_repair = pr['head']['ref'].startswith('build/')
    if pr['draft']:
        # Explicit product questions stay with their decision maker. A cut-off is engineering work.
        body = pr.get('body') or ''
        if can_repair and 'Stopped: the run ended without marking it ready,' in body:
            return 'fix', 'cutoff'
        return None
    if can_repair and pr.get('mergeable') is False:
        return 'fix', 'conflict'
    current = [v for v in verdicts if v['current']]
    if can_repair and current and current[-1]['verdict'] == 'reject':
        return 'fix', 'reject'
    relevant = [c for c in checks if c['name'] == 'check' and c.get('conclusion') != 'skipped'
                and c['head_sha'] == pr['head']['sha'] and c.get('app', {}).get('slug') == 'github-actions']
    if not relevant:
        return None
    check = max(relevant, key=lambda c: c['id'])
    if check['status'] != 'completed':
        return None
    if check['conclusion'] in ('failure', 'timed_out', 'cancelled') and can_repair:
        return 'fix', 'check'
    if check['conclusion'] == 'success':
        for name in ('rules',) + (('percy',) if 'ui' in kinds else ()):
            results = [c for c in checks if c['name'] == name and c['head_sha'] == pr['head']['sha']
                       and c.get('app', {}).get('slug') == 'github-actions' and c.get('conclusion') != 'skipped']
            latest = max(results, key=lambda c: c['id']) if results else None
            if not latest or latest['status'] != 'completed':
                return None
            if latest['conclusion'] != 'success':
                return ('fix', 'rules' if name == 'rules' else 'check') if can_repair else None
    if check['conclusion'] == 'success' and ('code' in kinds or 'loop' in kinds) and not current:
        return 'review', 'review'
    return None


def due(number, busy=None):
    pr = api(f'pulls/{number}')
    if pr.get('mergeable') is None and not pr['draft']:
        # The first REST read can start GitHub's asynchronous mergeability calculation.
        import time
        time.sleep(1)
        pr = api(f'pulls/{number}')
    if not eligible(pr):
        return None
    if busy is None:
        busy = {'build/' + s for s in active_builds()} | {local_branch()}
    if pr['head']['ref'] in busy:
        return None
    verdicts = [json.loads(line) for line in run('bash', 'loop/rules.sh', 'verdicts', str(number)).splitlines()]
    paths = run('gh', 'pr', 'diff', str(number), '--name-only')
    kinds = run('bash', 'loop/rules.sh', 'touches', data=paths).splitlines()
    checks = gh('api', f"repos/{{owner}}/{{repo}}/commits/{pr['head']['sha']}/check-runs?per_page=100&filter=latest", '--paginate', '--slurp')
    selected = decision(pr, [c for page in checks for c in page['check_runs']], verdicts, kinds)
    if selected is None:
        return None
    role, cause = selected
    base = api('git/ref/heads/main')['object']['sha'] if cause == 'conflict' else pr['base']['sha']
    return dict(pr=number, role=role, cause=cause, head=pr['head']['sha'], base=base, branch=pr['head']['ref'])


def key(task):
    # A new main only changes a conflict repair, never resets a failed review's retry budget.
    return [task['role'], task['cause'], task['head'], task['base'] if task['cause'] == 'conflict' else '']


def claim_record(number):
    refs = api(f'git/matching-refs/heads/loop-pr/{number}')
    exact = [r for r in refs if r['ref'] == f'refs/heads/loop-pr/{number}']
    if not exact:
        return None, None
    sha = exact[0]['object']['sha']
    return sha, json.loads(api(f'git/commits/{sha}')['message'])


def started(record):
    if record.get('phase') != 'issued':
        return False
    jobs = api(f"actions/runs/{record['run']}/jobs?per_page=100")['jobs']
    return any(step['name'] == 'Run anthropics/claude-code-action@v1'
               and step['status'] in ('in_progress', 'completed') and step.get('conclusion') != 'skipped'
               for job in jobs for step in job.get('steps', []))


def consumed(record):
    # A Claim reserves the next attempt; preflight or setup failures spend no model attempt.
    return record['attempt'] if started(record) else record['attempt'] - 1


def fixes(record):
    if not record:
        return 0
    return record.get('fixes', 0) + int(record['key'][0] == 'fix' and started(record))


def available(task, record):
    if not record:
        return True
    owner = api(f"actions/runs/{record['run']}")
    if owner['status'] != 'completed':
        return False
    if task['role'] == 'fix' and fixes(record) >= 2:
        return False
    if record['key'] != key(task):
        return True
    # A failed setup must not create an event-triggered retry storm.
    if owner.get('conclusion') != 'success' and owner.get('updated_at'):
        from datetime import datetime, timezone
        elapsed = (datetime.now(timezone.utc) - datetime.fromisoformat(owner['updated_at'].replace('Z', '+00:00'))).total_seconds()
        if elapsed < 300:
            return False
    return consumed(record) < 2


def write_claim(number, old, record):
    commit = api('git/commits', dict(message=json.dumps(record), tree=run('git', 'rev-parse', 'HEAD^{tree}'),
                                   parents=[old or run('git', 'rev-parse', 'HEAD')]))
    path = f"git/refs/heads/loop-pr/{number}"
    try:
        if old:
            # PATCH without force refuses the second sibling, even if its response arrives first.
            gh('api', '--method', 'PATCH', 'repos/{owner}/{repo}/' + path, '--input', '-',
               data=json.dumps(dict(sha=commit['sha'], force=False)))
        else:
            api('git/refs', dict(ref=f"refs/heads/loop-pr/{number}", sha=commit['sha']))
    except subprocess.CalledProcessError:
        # A lost response may have applied our write. Observe before deciding.
        latest, _ = claim_record(number)
        if latest == commit['sha']:
            return True
        if latest != old:
            return False
        raise
    return True


def acquire(task):
    """Non-force ref update is compare-and-swap: competing commits are siblings."""
    old, record = claim_record(task['pr'])
    if not available(task, record):
        return False
    attempt = consumed(record) + 1 if record and record['key'] == key(task) else 1
    return write_claim(task['pr'], old, dict(key=key(task), run=int(os.environ['GITHUB_RUN_ID']), attempt=attempt, phase='claimed', fixes=fixes(record)))


def issued(number):
    old, record = claim_record(number)
    if not record or record['run'] != int(os.environ['GITHUB_RUN_ID']):
        raise ValueError(f'Claim ownership changed for PR #{number}')
    if not write_claim(number, old, {**record, 'phase': 'issued'}):
        raise ValueError(f'Claim changed before starting PR #{number}')


def start(number, role, head):
    task = due(number)
    if not task or task['role'] != role or (head and task['head'] != head) or not acquire(task):
        return
    fresh = due(number)
    if not fresh or key(fresh) != key(task):
        return
    if role == 'review':
        prompt = run('bash', 'loop/runs.sh', 'review-due', str(number), task['head'], timeout=660)
        prompt = ('Review ' + prompt + '\nIf an earlier independent approval names an ancestor, focus on the changes since that approved head, including conflict resolutions and their effects. Your verdict must still cover the current head; do not repeat unchanged findings.')
    else:
        evidence = {'conflict': 'Merge origin/main into the PR branch and resolve conflicts; never rebase or force-push. Regenerate conflicted lockfiles as .agents/data/pr.md prescribes.',
                    'check': 'Inspect the failed required CI check on this exact head, read its failing job logs, and fix the cause.',
                    'rules': 'Inspect the failed rules job and repair its concrete finding. For a missing Author-Agent trailer on your latest commit only, amend its metadata, prove git diff between old and amended commits is empty, and push with an explicit force-with-lease matching the expected head; never rewrite code under that exception.',
                    'reject': 'Read the independent reject findings with loop/rules.sh verdicts and address each one.',
                    'cutoff': 'Continue the interrupted Builder work; inspect the PR body and linked run for what remains.'}[task['cause']]
        prompt = (f"Fix PR #{number} on branch {task['branch']}, expected head {task['head']}. {evidence}\n"
                  f"Your Author-Agent id is builder-{os.environ['GITHUB_RUN_ID']}. Mark the PR a draft while editing. "
                  "Run the required checks, remove a resolved Stopped: line, and mark it ready. Ordinary repairs never force-push; the rules task permits only its explicit metadata-only exception. "
                  "If the remote head moved, stop without overwriting it. Leave a precise Stopped: question only for an unresolved product decision.")
    issued(number)
    run('bash', 'loop/runs.sh', 'prompt', 'builder', data=prompt + '\n')
    with open(os.environ['GITHUB_OUTPUT'], 'a') as out:
        for name in ('pr', 'head', 'branch'):
            out.write(f'{name}={task[name]}\n')


def reconcile():
    prs = gh('pr', 'list', '--state', 'open', '--limit', '200', '--json', 'number,isDraft')
    busy = {'build/' + s for s in active_builds()} | {local_branch()}
    for pr in prs:
        task = due(pr['number'], busy)
        if not task:
            continue
        _, record = claim_record(task['pr'])
        if not available(task, record):
            print(f"#{task['pr']}: claimed or retry limit reached")
            continue
        workflow = 'review.yml' if task['role'] == 'review' else 'build.yml'
        run('gh', 'workflow', 'run', workflow, '-f', f"pr={task['pr']}", '-f', f"head={task['head']}")
        print(f"#{task['pr']}: queued {task['cause']} at {task['head'][:7]}")
    # Queue entry is serialized and row Claims are atomic; an empty queue does not dispatch again.
    if json.loads(run('bash', 'loop/runs.sh', 'queue', '4')):
        run('gh', 'workflow', 'run', 'build.yml')


def activity():
    rows = []
    for workflow in ('build', 'review', 'qa', 'retro', 'check', 'loop', 'visual'):
        runs = gh('run', 'list', '--workflow', workflow + '.yml', '--limit', '100',
                  '--json', 'databaseId,status,url,displayTitle')
        for item in runs:
            if item['status'] == 'completed':
                continue
            jobs = gh('run', 'view', str(item['databaseId']), '--json', 'jobs')['jobs']
            active = [j for j in jobs if j['status'] != 'completed']
            for job in active:
                rows.append(f"[{item['displayTitle']} · {job['name']}]({job.get('url') or item['url']}): {job['status']}")
            if not active:
                rows.append(f"[{item['displayTitle']}]({item['url']}): {item['status']}")
    prs = gh('pr', 'list', '--state', 'open', '--limit', '200', '--json', 'number,headRefOid,url')
    for pr in prs:
        _, record = claim_record(pr['number'])
        if not record:
            continue
        owner = api(f"actions/runs/{record['run']}")
        spent_review = record['key'][0] == 'review' and record['key'][2] == pr['headRefOid'] and consumed(record) >= 2
        if owner['status'] == 'completed' and (fixes(record) >= 2 or spent_review):
            rows.append(f"[PR #{pr['number']}]({pr['url']}): needs investigation after two attempts; [last run]({owner['html_url']})")
    return rows


if __name__ == '__main__':
    try:
        if sys.argv[1] == 'reconcile':
            reconcile()
        elif sys.argv[1] == 'start':
            start(int(sys.argv[2]), sys.argv[3], sys.argv[4] if len(sys.argv) > 4 else '')
        elif sys.argv[1] == 'activity':
            print(json.dumps(activity()))
        else:
            raise ValueError('expected reconcile, start or activity')
    except (subprocess.SubprocessError, ValueError, KeyError, TypeError, OSError) as error:
        print(f'PR dispatch failed: {error}', file=sys.stderr)
        sys.exit(4)
