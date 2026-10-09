#!/usr/bin/env python3
"""L88: reconcile PR work; a durable per-PR Claim owns review and repair together."""
import hashlib
import json
import os
import subprocess
import sys
import time
from datetime import datetime, timezone


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
    """Choose repair from current evidence, without a dispatch side effect. Review is review_task: CI never gates it."""
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
    checks = gh('api', f"repos/{{owner}}/{{repo}}/commits/{pr['head']['sha']}/check-runs?per_page=100&filter=latest", '--paginate', '--slurp')
    selected = decision(pr, [c for page in checks for c in page['check_runs']], verdicts, kinds)
    if selected is None:
        return None
    role, cause = selected
    current = [v for v in verdicts if v['current']]
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
                **({'repair_verdict': review_request(pr['head']['sha'], current[-1])['verdict']} if cause == 'reject' else {}))


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
                                           review_after=(record or {}).get('review_after'), repair_verdict=task.get('repair_verdict'),
                                           ci_head=task['head'] if task['role'] == 'ci' else (record or {}).get('ci_head'),
                                           ci_attempts=((record or {}).get('ci_attempts', 0) if (record or {}).get('ci_head') == task['head'] else 0) + 1
                                           if task['role'] == 'ci' else (record or {}).get('ci_attempts', 0)))


def issued(number, namespace='loop-pr'):
    old, record = claim_record(number, namespace)
    if not record or record['run'] != int(os.environ['GITHUB_RUN_ID']):
        raise ValueError(f'Claim ownership changed for PR #{number}')
    if not write_claim(number, old, {**record, 'phase': 'issued'}, namespace):
        raise ValueError(f'Claim changed before starting PR #{number}')


def start(number, role, head):
    if role == 'review':
        return start_review(number, head)
    task = due(number)
    if not task or task['role'] != role or (head and task['head'] != head):
        return
    if role == 'fix' and task.get('planning'):
        run('gh', 'workflow', 'run', 'feedback.yml', '-f', f'pr={number}', '-f', f"head={task['head']}")
        return
    if not acquire(task):
        return
    fresh = due(number)
    if not fresh or key(fresh) != key(task):
        return
    evidence = {'ci': 'Two controller CI reruns did not recover this head. Diagnose runner, concurrency and workflow failures before changing product code.',
                'conflict': 'Merge origin/main into the PR branch and resolve conflicts; never rebase or force-push. Regenerate conflicted lockfiles as .agents/data/pr.md prescribes.',
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
        prompt += (f"\nDiagnostic repair: earlier repair runs {prior}. Read their job logs and retained outcome.json/Ledger artifacts first. Legacy history may omit earlier run IDs; inspect the PR timeline for those. "
                   "Distinguish failed tests from cancelled CI, setup/authentication failure, stale work and publication failure. "
                   "Record the observed cause and a changed approach on the PR before editing; do not repeat an unsuccessful approach or invent unavailable evidence. "
                   "Reproduce the failure, make one bounded correction, and verify the failing observer. If the blocker is shared loop machinery, "
                   "create or reuse one linked engineering work Issue with reproduction and acceptance, label loop:work and ready-for-agent only when specified, "
                   "and keep this PR linked; ordinary engineering failure is not a product question. "
                   "Conclude with evidence of what changed and what remains. CI reruns belong to the trusted controller; do not spend a model run toggling draft status to rerun CI.")
    issued(number)
    run('bash', 'loop/runs.sh', 'prompt', 'builder', data=prompt + '\n')
    with open(os.environ['GITHUB_OUTPUT'], 'a') as out:
        for name in ('pr', 'head', 'branch'):
            out.write(f'{name}={task[name]}\n')


REVIEW = 'loop-review'
MODEL_STEP = 'Run anthropics/claude-code-action@v1'
LABELS = {'queued': ({'needs-review'}, {'reviewing', 'needs-fix'}),
          'reviewing': ({'reviewing'}, {'needs-review', 'needs-fix'}),
          'approved': (set(), {'needs-review', 'reviewing', 'needs-fix'}),
          'rejected': ({'needs-fix'}, {'needs-review', 'reviewing'})}
TITLES = {'reviewing': 'Reviewing', 'approved': 'Approved', 'rejected': 'Rejected', 'failed': 'Review failed', 'cancelled': 'Review cancelled'}


def review_task(number):
    """L24: an open same-repository Code or Loop PR head with no covering verdict is due, draft or ready, whatever CI says."""
    pr = api(f'pulls/{number}')
    if not eligible(pr):
        return None
    kinds = run('bash', 'loop/rules.sh', 'touches', data=run('gh', 'pr', 'diff', str(number), '--name-only')).splitlines()
    if not {'code', 'loop'} & set(kinds):
        return None
    verdicts = [json.loads(line) for line in run('bash', 'loop/rules.sh', 'verdicts', str(number)).splitlines()]
    current = [v for v in verdicts if v['current'] and ('planning' not in kinds or v['head'] == pr['head']['sha'])]
    after = review_request(pr['head']['sha'], current[-1]) if current and current[-1]['verdict'] == 'reject' else None
    if current:
        # Only a repair's handoff re-reviews an unchanged head under a reject.
        _, record = claim_record(number)
        if not after or (record or {}).get('review_after') != after:
            return None
    return dict(pr=number, role='review', cause='review', head=pr['head']['sha'], branch=pr['head']['ref'],
                **({'review_after': after} if current else {}), **({'planning': True} if 'planning' in kinds else {}))


def review_spent(record):
    # A Claim reserves the next attempt; preflight or setup failures spend no model attempt.
    return record['attempt'] if started(record) else record['attempt'] - 1


def review_available(head, record):
    """One reviewer per head: a live or spent owner of this head blocks; any owner of an older head never does."""
    if not record or record['head'] != head:
        return True
    owner = api(f"actions/runs/{record['run']}")
    if owner['status'] != 'completed':
        return False
    if owner.get('conclusion') != 'success' and owner.get('updated_at'):
        if time.time() - datetime.fromisoformat(owner['updated_at'].replace('Z', '+00:00')).timestamp() < 300:
            return False
    return review_spent(record) < 2


def acquire_review(task):
    old, record = claim_record(task['pr'], REVIEW)
    if not review_available(task['head'], record):
        return False
    attempt = review_spent(record) + 1 if record and record['head'] == task['head'] else 1
    return write_claim(task['pr'], old, dict(head=task['head'], run=int(os.environ['GITHUB_RUN_ID']), attempt=attempt, phase='claimed'), REVIEW)


def project(number, head, state):
    """Labels are a projection of the exact head's review, never its lock. Returns False for a head that moved."""
    pr = api(f'pulls/{number}')
    if pr['state'] != 'open' or pr['head']['sha'] != head:
        return False
    have = {label['name'] for label in pr['labels']}
    add, drop = LABELS[state]
    if add - have:
        api(f'issues/{number}/labels', {'labels': sorted(add - have)})
    for name in sorted(drop & have):
        run('gh', 'api', '--method', 'DELETE', f'repos/{{owner}}/{{repo}}/issues/{number}/labels/{name}')
    return True


def run_link(run_id):
    jobs = api(f'actions/runs/{run_id}/jobs?per_page=100')['jobs']
    job = next((j for j in jobs if j['name'] == 'review'), None)
    return job['html_url'] if job else f"{os.environ['GITHUB_SERVER_URL']}/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{run_id}"


def activity_comment(number, head, run_id, attempt, title, detail=''):
    """One durable comment per PR head, found by its hidden marker and updated in place."""
    body = (f"**{TITLES[title]} · Claude · commit `{head[:7]}`**\n\n{detail + chr(10) if detail else ''}[GitHub Actions run]({run_link(run_id)})\n\n"
            f"<!-- review-activity {head} run={run_id} attempt={attempt} -->")
    pages = gh('api', f'repos/{{owner}}/{{repo}}/issues/{number}/comments?per_page=100', '--paginate', '--slurp')
    mine = [c for page in pages for c in page if c['user']['login'] == 'github-actions[bot]' and f'<!-- review-activity {head} ' in c['body']]
    if not mine:
        api(f'issues/{number}/comments', {'body': body})
    elif mine[-1]['body'] != body:
        gh('api', '--method', 'PATCH', f"repos/{{owner}}/{{repo}}/issues/comments/{mine[-1]['id']}", '--input', '-', data=json.dumps({'body': body}))


def start_review(number, head):
    task = review_task(number)
    if not task or (head and task['head'] != head) or not acquire_review(task):
        return
    fresh = review_task(number)
    if not fresh or fresh['head'] != task['head']:
        return
    project(number, task['head'], 'queued')
    prompt = run('bash', 'loop/runs.sh', 'review-due', str(number), task['head'], *([task['review_after']['verdict']] if task.get('review_after') else []), timeout=660)
    prompt = ('Review ' + prompt + '\nIf an earlier independent approval names an ancestor, focus on the changes since that approved head, including conflict resolutions and their effects. Your verdict must still cover the current head; do not repeat unchanged findings.')
    issued(number, REVIEW)
    run('bash', 'loop/runs.sh', 'prompt', 'builder', data=prompt + '\n')
    with open(os.environ['GITHUB_OUTPUT'], 'a') as out:
        for name in ('pr', 'head', 'branch'):
            out.write(f'{name}={task[name]}\n')


def model_running(run_id):
    """True once the reviewer's model step runs; None when its job ended without ever running it."""
    jobs = [j for j in api(f'actions/runs/{run_id}/jobs?per_page=100')['jobs'] if j['name'] == 'review']
    if any(s['name'] == MODEL_STEP and s['status'] in ('in_progress', 'completed') and s.get('conclusion') != 'skipped'
           for j in jobs for s in j.get('steps', [])):
        return True
    return None if jobs and all(j['status'] == 'completed' for j in jobs) else False


def announce(number, head, poll=20, patience=1800):
    """At actual model startup: `reviewing` and the activity comment. A queued job claims nothing."""
    run_id = int(os.environ['GITHUB_RUN_ID'])
    end = time.time() + patience
    while not (running := model_running(run_id)):
        if running is None:
            return
        if time.time() > end:
            raise ValueError('the reviewer model did not start in time')
        time.sleep(poll)
    _, record = claim_record(number, REVIEW)
    if not record or record['run'] != run_id or record['head'] != head:
        return
    activity_comment(number, head, run_id, record['attempt'], 'reviewing')
    project(number, head, 'reviewing')


def conclude(number, head, run_id, attempt, outcome, reason=''):
    """Update the head's one activity comment, then its labels. A lost review returns to needs-review."""
    state = {'approve': 'approved', 'reject': 'rejected'}.get(outcome, 'queued')
    title = {'approve': 'approved', 'reject': 'rejected'}.get(outcome, outcome)
    detail = reason or ('The independent verdict is posted separately.' if state != 'queued' else '')
    if state == 'queued':
        detail = f"{reason or 'The reviewer ended without a verdict.'} Back to `needs-review`; reconciliation retries a bounded number of times."
    activity_comment(number, head, run_id, attempt, title, detail)
    project(number, head, state)


def finish(number, head, review_result, post_result, verdict):
    run_id = int(os.environ['GITHUB_RUN_ID'])
    _, record = claim_record(number, REVIEW)
    attempt = record['attempt'] if record and record['run'] == run_id else 1
    if review_result == 'success' and post_result == 'success' and verdict in ('approve', 'reject'):
        return conclude(number, head, run_id, attempt, verdict)
    outcome = 'cancelled' if 'cancelled' in (review_result, post_result) else 'failed'
    conclude(number, head, run_id, attempt, outcome, f'Reviewer job {review_result}, verdict post {post_result}.')


def settle(number):
    """Repair stale activity: a head labelled `reviewing` whose owner run ended is judged by its verdict or its run."""
    pr = api(f'pulls/{number}')
    _, record = claim_record(number, REVIEW)
    if not record or record['head'] != pr['head']['sha']:
        return
    owner = api(f"actions/runs/{record['run']}")
    if owner['status'] != 'completed':
        return
    verdicts = [json.loads(line) for line in run('bash', 'loop/rules.sh', 'verdicts', str(number)).splitlines()]
    current = [v for v in verdicts if v['current'] and v['head'] == record['head']]
    if current:
        return conclude(number, record['head'], record['run'], record['attempt'], current[-1]['verdict'])
    outcome = 'cancelled' if owner.get('conclusion') == 'cancelled' else 'failed'
    conclude(number, record['head'], record['run'], record['attempt'], outcome, f"The reviewer run {owner.get('conclusion') or 'ended'} without a verdict.")


def reconcile():
    run('bash', 'loop/runs.sh', 'recover')
    prs = gh('pr', 'list', '--state', 'open', '--limit', '200', '--json', 'number,isDraft,labels')
    busy = {'build/' + s for s in active_builds()} | {local_branch()}
    for pr in prs:
        if 'reviewing' in {label['name'] for label in pr.get('labels', [])}:
            settle(pr['number'])
        review = review_task(pr['number'])
        if review and review_available(review['head'], claim_record(pr['number'], REVIEW)[1]):
            run('gh', 'workflow', 'run', 'review.yml', '-f', f"pr={pr['number']}", '-f', f"head={review['head']}")
            print(f"#{pr['number']}: queued review at {review['head'][:7]}")
        task = due(pr['number'], busy)
        if not task:
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
        workflow = 'feedback.yml' if task.get('planning') else 'build.yml'
        run('gh', 'workflow', 'run', workflow, '-f', f"pr={task['pr']}", '-f', f"head={task['head']}")
        print(f"#{task['pr']}: queued {task['cause']} at {task['head'][:7]}")
    # Queue entry is serialized and row Claims are atomic; an empty queue does not dispatch again.
    if json.loads(run('bash', 'loop/runs.sh', 'queue', '4')):
        run('gh', 'workflow', 'run', 'build.yml')


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
    prs = gh('pr', 'list', '--state', 'open', '--limit', '200', '--json', 'number,headRefOid,url,labels')
    for pr in prs:
        if 'needs-review' in {label['name'] for label in pr.get('labels', [])}:
            _, reviewer = claim_record(pr['number'], REVIEW)
            if reviewer and reviewer['head'] == pr['headRefOid']:
                owner = api(f"actions/runs/{reviewer['run']}")
                if owner['status'] == 'completed' and review_spent(reviewer) >= 2:
                    rows.append(f"[PR #{pr['number']}]({pr['url']}): review needs investigation after two attempts; [last run]({owner['html_url']})")
        _, record = claim_record(pr['number'])
        if not record:
            continue
        owner = api(f"actions/runs/{record['run']}")
        if owner['status'] == 'completed' and fixes(record) >= 2:
            history = repair_history(record, owner)
            task = dict(head=pr['headRefOid'], cause=record['key'][1])
            now = time.time()
            wait = repair_wait(task, history, now)
            state = 'diagnostic repair eligible when current gates require it' if wait <= now else 'repair eligible after ' + datetime.fromtimestamp(wait).astimezone().isoformat()
            rows.append(f"[PR #{pr['number']}]({pr['url']}): {state}; [last run]({owner['html_url']})")
    return rows


if __name__ == '__main__':
    try:
        if sys.argv[1] == 'reconcile':
            reconcile()
        elif sys.argv[1] == 'start':
            start(int(sys.argv[2]), sys.argv[3], sys.argv[4] if len(sys.argv) > 4 else '')
        elif sys.argv[1] == 'announce':
            announce(int(sys.argv[2]), sys.argv[3])
        elif sys.argv[1] == 'finish':
            finish(int(sys.argv[2]), sys.argv[3], sys.argv[4], sys.argv[5], sys.argv[6] if len(sys.argv) > 6 else '')
        elif sys.argv[1] == 'repaired':
            repaired(int(sys.argv[2]))
        elif sys.argv[1] == 'budget':
            budget()
        elif sys.argv[1] == 'activity':
            print(json.dumps(activity()))
        else:
            raise ValueError('expected reconcile, start, announce, finish, repaired, budget or activity')
    except (subprocess.SubprocessError, ValueError, KeyError, TypeError, OSError) as error:
        print(f'PR dispatch failed: {error}', file=sys.stderr)
        sys.exit(4)
