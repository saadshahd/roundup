#!/usr/bin/env python3
"""L87: one plan-authenticated local Builder; launchd invokes one tick every five minutes.

Usage: python3 loop/codex.py <state-directory>
Run from a dedicated, clean clone. The state directory holds private logs and Attempt clones.
"""
import fcntl
import json
import os
from pathlib import Path
import subprocess
import sys
import time


def command(*args, cwd=None, data=None, timeout=60):
    return subprocess.run(args, cwd=cwd, input=data, text=True, capture_output=True,
                          check=True, timeout=timeout).stdout.strip()


def save(path, value):
    temporary = path.with_suffix('.tmp')
    temporary.write_text(json.dumps(value) + '\n')
    temporary.replace(path)


def publish(record):
    public = {key: record[key] for key in ('ids', 'slug', 'started', 'deadline', 'state', 'prs', 'usage') if key in record}
    command('gh', 'variable', 'set', 'LOOP_CODEX_STATUS', '--body', json.dumps(public))
    command('gh', 'workflow', 'run', 'status.yml')


def usage(path):
    totals = dict(input=0, output=0, cache_read=0, turns=0)
    lines = path.read_text().splitlines()
    for index, line in enumerate(lines):
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            if index != len(lines) - 1:
                raise
            # A killed CLI can leave its final event half-written; completed turns remain countable.
            break
        if event.get('type') == 'turn.completed':
            tokens = event['usage']
            cached = tokens.get('cached_input_tokens', 0)
            totals['input'] += tokens.get('input_tokens', 0) - cached
            totals['cache_read'] += cached
            totals['output'] += tokens.get('output_tokens', 0)
            totals['turns'] += 1
    return totals


def finish(record, state, code):
    prs = json.loads(command('gh', 'pr', 'list', '--head', 'build/' + record['slug'],
                             '--state', 'open', '--json', 'number,isDraft,url'))
    ready = len(prs) == 1 and not prs[0]['isDraft']
    record.update(state='ready' if code == 0 and ready else 'failed',
                  exit=code, ended=int(time.time()), prs=prs)
    events = Path(record['events'])
    if events.exists():
        record['usage'] = usage(events)
    save(state / 'last.json', record)
    save(Path(record['events']).parent / 'record.json', record)
    # L23 retains every PR and frees only Claims without one. Local code/logs are always kept.
    command('loop/runs.sh', 'unclaim', data=json.dumps([record]))
    publish(record)
    (state / 'active.json').unlink()
    return record['state'] == 'ready'


def tick(state):
    active = state / 'active.json'
    if active.exists():
        record = json.loads(active.read_text())
        # An orphan is still occupying the local slot. gtimeout bounds its whole process group.
        if record.get('pid'):
            try:
                os.kill(record['pid'], 0)
            except ProcessLookupError:
                pass
            else:
                return
        finish(record, state, interrupted_exit)
        return
    if command('gh', 'variable', 'get', 'LOOP_CODEX_ENABLED') != 'true':
        return
    if (state / 'last.json').exists():
        previous = json.loads((state / 'last.json').read_text())
        if previous['state'] == 'failed' and time.time() - previous['ended'] < 3600:
            return
    if command('git', 'status', '--porcelain'):
        raise RuntimeError('Codex controller checkout is dirty; leaving it untouched')
    command('git', 'fetch', 'origin', 'main')
    command('git', 'checkout', '--detach', 'origin/main')
    base = command('git', 'rev-parse', 'HEAD')
    # A red main must be reverted, never built forward by this service.
    checks = json.loads(command('gh', 'run', 'list', '--workflow', 'check.yml', '--branch', 'main',
                                 '--event', 'push', '--limit', '1', '--json', 'headSha,conclusion'))
    if not checks or checks[0] != {'headSha': base, 'conclusion': 'success'}:
        return
    auth = subprocess.run(['codex', 'login', 'status'], text=True, capture_output=True, timeout=30, check=True)
    if 'Logged in using ChatGPT' not in auth.stdout + auth.stderr:
        raise RuntimeError('Codex fleet requires ChatGPT login; run codex login locally')
    rows = json.loads(command('loop/runs.sh', 'queue', '4', timeout=300))
    failures = [json.loads(path.read_text()) for path in state.glob('*/record.json')]
    rows = [row for row in rows if sum(1 for run in failures if run['base'] == base
            and run['slug'] == row['slug'] and run['state'] == 'failed') < 2][:1]
    if not rows:
        return
    claimed = json.loads(command('loop/runs.sh', 'claim', data=json.dumps(rows)))
    if not claimed:
        return
    row = claimed[0]
    started = int(time.time())
    attempt = state / f'{started}-{row["slug"]}'
    attempt.mkdir(mode=0o700)
    record = dict(row, base=base, started=started, deadline=started + 7200,
                  state='running', events=str(attempt / 'events.jsonl'), model='gpt-6-astra')
    save(active, record)
    try:
        publish(record)
        origin = command('git', 'remote', 'get-url', 'origin')
        checkout = attempt / 'checkout'
        command('git', 'clone', '--shared', str(Path.cwd()), str(checkout), timeout=120)
        command('git', 'remote', 'set-url', 'origin', origin, cwd=checkout)
        command('git', 'checkout', '-b', 'build/' + row['slug'], base, cwd=checkout)
        prompt = Path('.agents/builder.md').read_text() + '\n## Task\n\n' + (
            f'Build Work row {row["ids"]} of {row["file"]} on the checked-out branch build/{row["slug"]}. '
            f'Its remote Claim is yours. Your Author-Agent id is codex-{started}. '
            'The user authorized this build and routine engineering decisions; proceed without another confirmation. '
            'Use the Builder recipe and existing independent CI/review/merge gates. '
            'Do not merge loop machinery or approve your own PR. Use agent-browser for browser work. '
            'Finish with a ready PR or a draft describing the concrete unresolved question. '
            'The run has a two-hour deadline.\n')
        (attempt / 'prompt.md').write_text(prompt)
        environment = {key: value for key, value in os.environ.items()
                       if key not in ('OPENAI_API_KEY', 'CODEX_API_KEY')}
        with (attempt / 'events.jsonl').open('w') as output, (attempt / 'stderr.log').open('w') as errors:
            # timeout kills the entire group even if this controller is interrupted.
            process = subprocess.Popen([
                'gtimeout', '--kill-after=20s', '7200s', 'codex', 'exec', '--ignore-user-config',
                '--model', record['model'], '--sandbox', 'workspace-write',
                '-c', 'sandbox_workspace_write.network_access=true', '-c', 'approval_policy="never"',
                '--json', '-o', str(attempt / 'result.md'), prompt,
            ], cwd=checkout, env=environment, stdout=output, stderr=errors, start_new_session=True)
            record['pid'] = process.pid
            save(active, record)
            code = process.wait()
    except BaseException:
        # Keep the active record when a child survives; the next tick must recover it first.
        if 'pid' not in record:
            finish(record, state, interrupted_exit)
        raise
    if not finish(record, state, code):
        raise RuntimeError('Codex did not deliver a ready PR; see ' + str(attempt))


interrupted_exit = 130
if __name__ == '__main__':
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    state = Path(sys.argv[1]).resolve()
    state.mkdir(parents=True, exist_ok=True, mode=0o700)
    with (state / 'lock').open('w') as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            sys.exit(0)
        tick(state)
