#!/usr/bin/env python3
"""L88: reconcile PR work; a durable per-PR Claim owns review and repair together."""
import hashlib
import json
import os
import subprocess
import sys
import time
from datetime import datetime, timezone

import trail


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
    current = [v for v in verdicts if v['current'] and ('planning' not in kinds or v['head'] == pr['head']['sha'])]
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
        lanes = {}
        for item in checks:
            if item['head_sha'] == pr['head']['sha'] and item.get('app', {}).get('slug') == 'github-actions' and item['name'] in ('rust', 'web'):
                if item['name'] not in lanes or item['id'] > lanes[item['name']]['id']:
                    lanes[item['name']] = item
        if any(item['status'] != 'completed' for item in lanes.values()):
            return None
        broken = any(item.get('conclusion') in ('failure', 'timed_out') for item in lanes.values())
        cancelled = check['conclusion'] == 'cancelled' or any(item.get('conclusion') == 'cancelled' for item in lanes.values())
        return ('ci', 'cancelled-ci') if cancelled and not broken else ('fix', 'check')
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


def due(number, busy=None, reserved_ci=False):
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
    _, record = claim_record(number)
    current = [v for v in verdicts if v['current']]
    review_after = review_request(pr['head']['sha'], current[-1]) if current and current[-1]['verdict'] == 'reject' else None
    rereview = review_after and (record or {}).get('review_after') == review_after
    decision_verdicts = [{**v, 'current': False} for v in verdicts] if rereview else verdicts
    checks = gh('api', f"repos/{{owner}}/{{repo}}/commits/{pr['head']['sha']}/check-runs?per_page=100&filter=latest", '--paginate', '--slurp')
    selected = decision(pr, [c for page in checks for c in page['check_runs']], decision_verdicts, kinds)
    if selected is None:
        return None
    role, cause = selected
    ci_run = None
    if role == 'ci':
        import re
        latest = max((c for page in checks for c in page['check_runs'] if c['name'] == 'check'
                      and c['head_sha'] == pr['head']['sha'] and c.get('app', {}).get('slug') == 'github-actions'), key=lambda c: c['id'])
        match = re.search(r'/actions/runs/(\d+)', latest.get('details_url') or '')
        if not match:
            raise ValueError('Cancelled CI has no Actions run link')
        ci_run = int(match[1])
        if api(f'actions/runs/{ci_run}')['status'] != 'completed':
            return None
        _, record = claim_record(number)
        if not reserved_ci and record and record.get('ci_head') == pr['head']['sha'] and record.get('ci_attempts', 0) >= 2:
            role, cause = 'fix', 'ci'
    base = api('git/ref/heads/main')['object']['sha'] if cause == 'conflict' else pr['base']['sha']
    return dict(pr=number, role=role, cause=cause, head=pr['head']['sha'], base=base, branch=pr['head']['ref'], ci_run=ci_run,
                **({'planning': True} if 'planning' in kinds else {}),
                **({'review_after': review_after} if rereview else {}),
                **({'repair_verdict': review_after['verdict']} if cause == 'reject' and review_after else {}))


def review_request(head, verdict):
    return dict(head=head, verdict=hashlib.sha256(json.dumps(verdict, sort_keys=True).encode()).hexdigest())


def repaired(number):
    """A successful owned repair requests review; it never approves its delivery."""
    old, record = claim_record(number)
    if not record or record['run'] != int(os.environ['GITHUB_RUN_ID']) or record['key'][0] != 'fix':
        raise ValueError('repair completion does not own this PR')
    jobs = api(f"actions/runs/{record['run']}/jobs?per_page=100")['jobs']
    if not any(step['name'] == 'Run anthropics/claude-code-action@v1' and step.get('conclusion') == 'success'
               for job in jobs for step in job.get('steps', [])):
        return
    pr = api(f'pulls/{number}')
    if not eligible(pr) or pr['draft']:
        return
    verdicts = [json.loads(line) for line in run('bash', 'loop/rules.sh', 'verdicts', str(number)).splitlines()]
    current = [v for v in verdicts if v['current']]
    if not current or current[-1]['verdict'] != 'reject':
        return
    request = review_request(pr['head']['sha'], current[-1])
    if request['verdict'] != record.get('repair_verdict'):
        return
    if not write_claim(number, old, {**record, 'review_after': request}):
        raise ValueError('repair owner changed before review handoff')


def key(task):
    # A new main only changes a conflict repair, never resets a failed review's retry budget.
    return [task['role'], task['cause'], task['head'], task['base'] if task['cause'] == 'conflict' else '']


def claim_record(number, namespace="loop-pr"):
    refs = api(f'git/matching-refs/heads/{namespace}/{number}')
    exact = [r for r in refs if r['ref'] == f'refs/heads/{namespace}/{number}']
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
    invoked = record.get('phase') == 'issued' if record['key'][0] == 'ci' else started(record)
    return record['attempt'] if invoked else record['attempt'] - 1


def fixes(record):
    if not record:
        return 0
    return record.get('fixes', 0) + int(record['key'][0] == 'fix' and started(record))


def repair_history(record, owner):
    """Carry recent charged repairs through reviews and deterministic CI recovery."""
    if not record:
        return []
    if not record.get('repairs') and fixes(record) == 0:
        return []
    at = datetime.fromisoformat(owner['updated_at'].replace('Z', '+00:00')).timestamp()
    if 'repairs' not in record:
        # Old Claims only kept a lifetime total: conservatively date that total at the last owner.
        return [dict(run=record['run'], head=record['key'][2], cause=record['key'][1], at=at)] * min(fixes(record), 6)
    history = list(record['repairs'])
    if record['key'][0] == 'fix' and started(record):
        history.append(dict(run=record['run'], head=record['key'][2], cause=record['key'][1], at=at))
    return history[-6:]


def repair_wait(task, history, now):
    recent = [item for item in history if item['at'] > now - 86400]
    if len(recent) >= 6:
        return min(item['at'] for item in recent) + 86400
    # The third repair diagnoses immediately. Further unchanged failures back off, at most two hours.
    if len(recent) >= 3 and history[-1]['head'] == task['head'] and history[-1]['cause'] == task['cause']:
        return max(now, history[-1]['at'] + min(7200, 900 * 2 ** (len(recent) - 3)))
    return now


def available(task, record):
    if not record:
        return True
    owner = api(f"actions/runs/{record['run']}")
    if owner['status'] != 'completed':
        return False
    now = time.time()
    # Setup failure is throttled across changing heads too; it spends no model attempt.
    if owner.get('conclusion') != 'success' and owner.get('updated_at'):
        elapsed = now - datetime.fromisoformat(owner['updated_at'].replace('Z', '+00:00')).timestamp()
        if elapsed < 300:
            return False
    if task['role'] == 'ci' and record.get('ci_head') == task['head'] and record.get('ci_attempts', 0) >= 2:
        return False
    if task['role'] == 'fix':
        return repair_wait(task, repair_history(record, owner), now) <= now
    return record['key'] != key(task) or consumed(record) < 2


def write_claim(number, old, record, namespace="loop-pr"):
    commit = api('git/commits', dict(message=json.dumps(record), tree=run('git', 'rev-parse', 'HEAD^{tree}'),
                                   parents=[old or run('git', 'rev-parse', 'HEAD')]))
    path = f"git/refs/heads/{namespace}/{number}"
    try:
        if old:
            # PATCH without force refuses the second sibling, even if its response arrives first.
            gh('api', '--method', 'PATCH', 'repos/{owner}/{repo}/' + path, '--input', '-',
               data=json.dumps(dict(sha=commit['sha'], force=False)))
        else:
            api('git/refs', dict(ref=f"refs/heads/{namespace}/{number}", sha=commit['sha']))
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired):
        # A lost response may have applied our write. Observe before deciding.
        latest, _ = claim_record(number) if namespace == "loop-pr" else claim_record(number, namespace)
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
    history = repair_history(record, api(f"actions/runs/{record['run']}")) if record else []
    return write_claim(task['pr'], old, dict(key=key(task), run=int(os.environ['GITHUB_RUN_ID']), attempt=attempt,
                                           phase='claimed', fixes=fixes(record), repairs=history,
                                           review_after=task.get('review_after'), repair_verdict=task.get('repair_verdict'),
                                           ci_head=task['head'] if task['role'] == 'ci' else (record or {}).get('ci_head'),
                                           ci_attempts=((record or {}).get('ci_attempts', 0) if (record or {}).get('ci_head') == task['head'] else 0) + 1
                                           if task['role'] == 'ci' else (record or {}).get('ci_attempts', 0)))


def issued(number):
    old, record = claim_record(number)
    if not record or record['run'] != int(os.environ['GITHUB_RUN_ID']):
        raise ValueError(f'Claim ownership changed for PR #{number}')
    if not write_claim(number, old, {**record, 'phase': 'issued'}):
        raise ValueError(f'Claim changed before starting PR #{number}')


def start(number, role, head):
    task = due(number)
    if not task or task['role'] != role or (head and task['head'] != head):
        print(f'PR #{number}: no eligible {role} for requested head {head or "current"}; no agent started.')
        return
    if role == 'fix' and task.get('planning'):
        run('gh', 'workflow', 'run', 'feedback.yml', '-f', f'pr={number}', '-f', f"head={task['head']}")
        return
    if not acquire(task):
        print(f'PR #{number}: active owner or retry wait; no agent started.')
        return
    fresh = due(number)
    if not fresh or key(fresh) != key(task):
        return
    if role == 'review':
        prompt = run('bash', 'loop/runs.sh', 'review-due', str(number), task['head'], *([task['review_after']['verdict']] if task.get('review_after') else []), timeout=660)
        prompt = ('Review ' + prompt + '\nIf an earlier independent approval names an ancestor, focus on the changes since that approved head, including conflict resolutions and their effects. Your verdict must still cover the current head; do not repeat unchanged findings.')
    else:
        evidence = {'ci': 'Two controller CI reruns did not recover this head. Diagnose runner, concurrency and workflow failures before changing product code.',
                    'conflict': 'Merge origin/main into the PR branch and resolve conflicts; never rebase or force-push. Read the independent verdicts and address every unresolved reject before marking ready; a base merge alone does not resolve a finding. Regenerate conflicted lockfiles as .agents/data/pr.md prescribes.',
                    'check': 'Inspect the failed required CI check on this exact head, read its failing job logs, and fix the cause.',
                    'rules': 'Inspect the failed rules job and repair its concrete finding. For a missing Author-Agent trailer on your latest commit only, amend its metadata, prove git diff between old and amended commits is empty, and push with an explicit force-with-lease matching the expected head; never rewrite code under that exception.',
                    'reject': 'Read the independent reject findings with loop/rules.sh verdicts and address each one.',
                    'cutoff': 'Continue the interrupted Builder work; inspect the PR body and linked run for what remains.'}[task['cause']]
        prompt = (f"Fix PR #{number} on branch {task['branch']}, expected head {task['head']}. {evidence}\n"
                  f"Your Author-Agent id is builder-{os.environ['GITHUB_RUN_ID']}. Mark the PR a draft while editing. "
                  "Run the required checks, remove a resolved Stopped: line, and mark it ready. Ordinary repairs never force-push; the rules task permits only its explicit metadata-only exception. "
                  "If the remote head moved, stop without overwriting it. Leave a precise Stopped: question only for an unresolved product decision.")
        _, record = claim_record(number)
        history = record.get('repairs', [])
        if record.get('fixes', 0) >= 2:
            prior = ', '.join(dict.fromkeys(str(item['run']) for item in history))
            prompt += (f"\nDiagnostic repair: earlier repair runs {prior}. Legacy history may omit earlier run IDs; inspect the PR timeline for those. "
                       + trail.DIAGNOSTIC + " CI reruns belong to the trusted controller; do not spend a model run toggling draft status to rerun CI.")
        prompt += trail.pr_section(number)
    print(f'PR #{number}: starting {role} for {task["cause"]} at {task["head"][:7]}.')
    issued(number)
    run('bash', 'loop/runs.sh', 'prompt', 'builder', data=prompt + '\n')
    with open(os.environ['GITHUB_OUTPUT'], 'a') as out:
        for name in ('pr', 'head', 'branch', 'cause'):
            out.write(f'{name}={task[name]}\n')


def reconcile():
    run('bash', 'loop/runs.sh', 'recover')
    prs = gh('pr', 'list', '--state', 'open', '--limit', '200', '--json', 'number,isDraft')
    busy = {'build/' + s for s in active_builds()} | {local_branch()}
    for pr in prs:
        task = due(pr['number'], busy)
        if not task:
            print(f'PR #{pr["number"]}: no eligible review or repair on this sweep.')
            continue
        _, record = claim_record(task['pr'])
        if not available(task, record):
            print(f"#{task['pr']}: owner active or bounded retry waiting")
            continue
        if task['role'] == 'ci':
            if acquire(task):
                fresh = due(task['pr'], busy, reserved_ci=True)
                if fresh and key(fresh) == key(task) and fresh['ci_run'] == task['ci_run']:
                    # Reserve before mutation: a lost rerun response cannot cause an unbounded retry storm.
                    issued(task['pr'])
                    run('gh', 'run', 'rerun', str(task['ci_run']))
                    print(f"#{task['pr']}: reran cancelled CI {task['ci_run']}; no model invoked")
            continue
        workflow = 'review.yml' if task['role'] == 'review' else ('feedback.yml' if task.get('planning') else 'build.yml')
        run('gh', 'workflow', 'run', workflow, '-f', f"pr={task['pr']}", '-f', f"head={task['head']}")
        print(f"#{task['pr']}: queued {task['cause']} at {task['head'][:7]}")
    # Queue entry is serialized and row Claims are atomic; an empty queue does not dispatch again.
    if json.loads(run('bash', 'loop/runs.sh', 'queue', '4')):
        run('gh', 'workflow', 'run', 'build.yml')
        print('Queued Builder work; next: build run claims eligible Issues.')
    else:
        print('No Builder work eligible; active Claims, dependencies and retry waits remain in force.')


def budget():
    """Defer full scans when the authenticated token has little API capacity left."""
    resources = gh('api', 'rate_limit')['resources']
    limits = {name: resources[name] for name in ('core', 'graphql')}
    if any(type(limit.get(field)) is not int or limit[field] < 0
           for limit in limits.values() for field in ('remaining', 'reset')):
        raise ValueError('invalid GitHub API budget')
    low = {name: limit for name, limit in limits.items() if limit['remaining'] < 100}
    with open(os.environ['GITHUB_OUTPUT'], 'a') as output:
        output.write('available=' + str(not low).lower() + '\n')
    if low:
        reset = datetime.fromtimestamp(max(limit['reset'] for limit in low.values()), tz=timezone.utc).isoformat()
        message = 'Full API scan deferred: ' + ', '.join(f"{name}={limit['remaining']} remaining" for name, limit in low.items()) + f'. Budget resets at {reset}; a later scheduled run retries.'
        print(message)
        if os.environ.get('GITHUB_STEP_SUMMARY'):
            with open(os.environ['GITHUB_STEP_SUMMARY'], 'a') as summary:
                summary.write(message + '\n')


def activity():
    import feedback
    rows = feedback.activity()
    for workflow in ('build', 'review', 'feedback', 'qa', 'retro', 'check', 'loop', 'visual'):
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
        if owner['status'] == 'completed' and fixes(record) >= 2:
            history = repair_history(record, owner)
            task = dict(head=pr['headRefOid'], cause=record['key'][1])
            now = time.time()
            wait = repair_wait(task, history, now)
            state = 'diagnostic repair eligible when current gates require it' if wait <= now else 'repair eligible after ' + datetime.fromtimestamp(wait).astimezone().isoformat()
            rows.append(f"[PR #{pr['number']}]({pr['url']}): {state}; [last run]({owner['html_url']})")
        elif owner['status'] == 'completed' and spent_review:
            rows.append(f"[PR #{pr['number']}]({pr['url']}): review needs investigation after two attempts; [last run]({owner['html_url']})")
    return rows


if __name__ == '__main__':
    try:
        if sys.argv[1] == 'reconcile':
            reconcile()
        elif sys.argv[1] == 'start':
            start(int(sys.argv[2]), sys.argv[3], sys.argv[4] if len(sys.argv) > 4 else '')
        elif sys.argv[1] == 'repaired':
            repaired(int(sys.argv[2]))
        elif sys.argv[1] == 'budget':
            budget()
        elif sys.argv[1] == 'activity':
            print(json.dumps(activity()))
        else:
            raise ValueError('expected reconcile, start, repaired, budget or activity')
    except (subprocess.SubprocessError, ValueError, KeyError, TypeError, OSError) as error:
        from outcome import describe  # outcome imports this module
        print(f'PR dispatch failed: {describe(error)}', file=sys.stderr)
        sys.exit(4)
