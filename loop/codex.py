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


def deliver(record):
    """Only the controller can commit, push or open a PR; the Builder receives no GitHub credential."""
    attempt = Path(record['events']).parent
    checkout = attempt / 'checkout'
    if not checkout.exists():
        return
    head = command('git', 'rev-parse', 'HEAD', cwd=checkout)
    if head != record['base']:
        parent = command('git', 'rev-parse', 'HEAD^', cwd=checkout)
        trailer = command('git', 'show', '-s', '--format=%(trailers:key=Author-Agent,valueonly)', 'HEAD', cwd=checkout)
        if parent != record['base'] or trailer != 'codex-' + str(record['started']):
            raise RuntimeError('Unexpected checkout history; preserve it for inspection')
    result = json.loads((attempt / 'result.json').read_text()) if (attempt / 'result.json').exists() else {}
    body = 'Scenarios: ' + record['ids'] + '\n\n' + result.get('body', 'The local Codex run ended before its final report; changes are preserved for a fix run.')
    if (record['exit'] != 0 or result.get('status') != 'ready') and result.get('status') != 'stopped':
        body += '\n\nStopped: the run ended without marking it ready, local Codex attempt ' + str(record['started']) + '; changes are preserved for repair.'
    body_path = attempt / 'pr.md'
    body_path.write_text(body)
    if head == record['base']:
        if not command('git', 'status', '--porcelain', cwd=checkout):
            if result.get('status') != 'stopped':
                return
            # A question still needs a reviewable draft, even before implementation.
            scenario = checkout / record['file']
            with scenario.open('a') as output:
                output.write('\n\n## Unresolved question\n\n' + result['body'] + '\n')
        command('git', '-c', 'core.hooksPath=/dev/null', 'add', '-A', cwd=checkout)
        command('git', '-c', 'core.hooksPath=/dev/null', 'commit', '-m', record['ids'] + ': ' + result.get('title', 'continue the interrupted build'),
                '-m', 'Author-Agent: codex-' + str(record['started']), cwd=checkout)
    origin = command('git', 'remote', 'get-url', 'origin')
    command('git', '-c', 'core.hooksPath=/dev/null', 'push', origin, 'HEAD:refs/heads/build/' + record['slug'], cwd=checkout)
    args = ['gh', 'pr', 'create', '--base', 'main', '--head', 'build/' + record['slug'],
            '--title', record['ids'] + ': ' + result.get('title', 'continue the interrupted build'), '--body-file', str(body_path)]
    if record['exit'] != 0 or result.get('status') != 'ready':
        args.append('--draft')
    command(*args)


def finish(record, state, code):
    prs = json.loads(command('gh', 'pr', 'list', '--head', 'build/' + record['slug'],
                             '--state', 'open', '--json', 'number,isDraft,url'))
    ready = len(prs) == 1 and not prs[0]['isDraft']
    if code == 0 and ready:
        command('gh', 'pr', 'merge', prs[0]['url'], '--auto', '--merge')
    record.update(state='ready' if code == 0 and ready else 'failed',
                  exit=code, ended=int(time.time()), prs=prs)
    events = Path(record['events'])
    if events.exists():
        record['usage'] = usage(events)
    save(state / 'last.json', record)
    save(Path(record['events']).parent / 'record.json', record)
    # L23 retains every PR and frees only Claims without one. Local code/logs are always kept.
    if command('git', 'ls-remote', '--heads', 'origin', 'refs/heads/build/' + record['slug']):
        command('loop/runs.sh', 'unclaim', data=json.dumps([record]))
    publish(record)
    if len(prs) == 1 and prs[0]['isDraft']:
        command('gh', 'workflow', 'run', 'merge-ready.yml', '-f', 'pr=' + str(prs[0]['number']))
    # Release public ownership before dispatch; recovery repeats this idempotent handoff.
    command('gh', 'workflow', 'run', 'reconcile.yml')
    (state / 'active.json').unlink()
    return record['state'] == 'ready'


def tick(state):
    active = state / 'active.json'
    if active.exists():
        record = json.loads(active.read_text())
        # Match both PID and creation time; PID reuse after a reboot cannot hold the slot.
        if record.get('pid'):
            observed = subprocess.run(['ps', '-p', str(record['pid']), '-o', 'lstart='],
                                      text=True, capture_output=True, timeout=10)
            if observed.returncode == 0 and not record.get('birth') and time.time() <= record['deadline']:
                return
            if observed.returncode == 0 and observed.stdout.strip() == record.get('birth'):
                if time.time() <= record['deadline']:
                    return
                os.killpg(record['pid'], 9)
        record.setdefault('exit', interrupted_exit)
        if not json.loads(command('gh', 'pr', 'list', '--head', 'build/' + record['slug'], '--state', 'all', '--json', 'number')):
            deliver(record)
        finish(record, state, record['exit'])
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
    rows = json.loads(command('loop/runs.sh', 'queue', '5', timeout=300))
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
        command('git', 'fetch', 'origin', 'main', cwd=checkout)
        command('git', 'checkout', '-b', 'build/' + row['slug'], base, cwd=checkout)
        command('git', 'config', 'credential.helper', '', cwd=checkout)
        command('git', 'config', '--add', 'credential.helper', '!gh auth git-credential', cwd=checkout)
        prompt = Path('.agents/builder.md').read_text() + '\n## Task\n\n' + (
            f'Build Work row {row["ids"]} of {row["file"]} on the checked-out branch build/{row["slug"]}. '
            f'Its remote Claim is yours. Your Author-Agent id is codex-{started}. '
            'The user authorized this build and routine engineering decisions; proceed without another confirmation. '
            'Use the Builder recipe and existing independent CI/review/merge gates. '
            'Do not merge or approve your own PR. Use agent-browser for browser work. '
            'The controller, not this sandboxed build run, owns git commits, pushes and PR creation. '
            'Implement and run pnpm install and just check; leave changes uncommitted. '
            'Return status ready only after checks pass, with a concise title and PR body. '
            'For an unresolved product question return status stopped and include Stopped: with its consequence and recommendation in the body. '
            'The run has a two-hour deadline.\n')
        (attempt / 'prompt.md').write_text(prompt)
        # Pass only non-secret runtime paths/locales. GitHub writes belong to the controller.
        environment = {key: value for key, value in os.environ.items() if key in
                       ('PATH', 'HOME', 'USER', 'LOGNAME', 'TMPDIR', 'LANG', 'LC_ALL', 'TERM',
                        'CARGO_HOME', 'RUSTUP_HOME', 'PNPM_HOME', 'CODEX_HOME')}
        environment['NEXTEST_TEST_THREADS'] = '1'
        (attempt / 'gh').mkdir()
        environment['GH_CONFIG_DIR'] = str(attempt / 'gh')
        cache = state / 'cache'
        cache.mkdir(exist_ok=True)
        environment['CARGO_HOME'] = str(cache / 'cargo')
        environment['XDG_CACHE_HOME'] = str(cache)
        environment['npm_config_store_dir'] = str(cache / 'pnpm')
        schema = {'type': 'object', 'properties': {
            'status': {'type': 'string', 'enum': ['ready', 'stopped']},
            'title': {'type': 'string'}, 'body': {'type': 'string'}},
            'required': ['status', 'title', 'body'], 'additionalProperties': False}
        save(attempt / 'schema.json', schema)
        with (attempt / 'events.jsonl').open('w') as output, (attempt / 'stderr.log').open('w') as errors:
            # timeout kills the entire group even if this controller is interrupted.
            process = subprocess.Popen([
                'gtimeout', '--kill-after=20s', '7200s', 'codex', 'exec', '--ignore-user-config', '--ignore-rules',
                '--model', record['model'], '--sandbox', 'workspace-write', '--add-dir', str(cache),
                '-c', 'sandbox_workspace_write.network_access=true', '-c', 'approval_policy="never"',
                '--json', '--output-schema', str(attempt / 'schema.json'), '-o', str(attempt / 'result.json'), prompt,
            ], cwd=checkout, env=environment, stdout=output, stderr=errors, start_new_session=True)
            record['pid'] = process.pid
            save(active, record)
            record['birth'] = command('ps', '-p', str(process.pid), '-o', 'lstart=')
            record['deadline'] = int(time.time()) + 7230
            save(active, record)
            code = process.wait()
    except BaseException:
        # Keep the active record when a child survives; the next tick must recover it first.
        if 'pid' not in record:
            finish(record, state, interrupted_exit)
        raise
    record['exit'] = code
    save(active, record)
    deliver(record)
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
