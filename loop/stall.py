#!/usr/bin/env python3
"""L94, L79: the loop notices its own stall. Work waits while no PR merged to `main` in three hours: one work
Issue (Key `stall-<yyyymmdd-hh>`) lists it, hourly comments repeat it, six hours ask the user, and the first run after
a merge closes it. Exit 4: GitHub could not be read or written."""
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import builder  # noqa: E402
import orders  # noqa: E402

STALL = 3 * 3600
ASK = 6 * 3600
WORKFLOWS = ('build', 'fix', 'review')
SPEC = 'https://github.com/saadshahd/roundup/blob/main/scenarios/loop-rules.md'


def stamp(seconds):
    return datetime.fromtimestamp(seconds, timezone.utc).strftime('%Y-%m-%d %H:%M UTC')


def paged(path):
    return [item for page in orders.gh('api', path, '--paginate', '--slurp') for item in page]


def stall_key(issue):
    return next((line.removeprefix('Key: ').strip() for line in (issue['body'] or '').splitlines()
                 if line.startswith('Key: stall-')), None)


def last_merge():
    """(number, url, merged-at seconds) of the latest PR merged to `main`, or None."""
    merged = orders.gh('pr', 'list', '--state', 'merged', '--base', 'main', '--limit', '100', '--json',
                       'number,url,mergedAt')
    merged = [pr for pr in merged if pr['mergedAt']]
    if not merged:
        return None
    pr = max(merged, key=lambda pr: pr['mergedAt'])
    return pr['number'], pr['url'], builder.created(pr['mergedAt'])


def waiting(issues, prs):
    """Lines for each open work Issue (L34) not waiting on the user (a stall Issue is the alarm, not work) and each
    open same-repository `build/` PR not waiting on the user."""
    found = [f"Issue #{i['number']} {i['title']}" for i in issues
             if builder.NEEDS_USER not in {l['name'] for l in i['labels']} and not stall_key(i)]
    found += [f"PR #{p['number']} {p['headRefName']}" for p in prs
              if p['headRefName'].startswith('build/') and not p['isCrossRepository']
              and builder.NEEDS_USER not in {l['name'] for l in p['labels']}]
    return found


def cause(run):
    """Why a run did no model work: its Claude step failed (a usage limit or setup failure) or never ran."""
    jobs = orders.gh('api', f"repos/{{owner}}/{{repo}}/actions/runs/{run['id']}/jobs?per_page=100")['jobs']
    steps = [s for job in jobs for s in job.get('steps', []) if 'claude' in s['name'].lower()]
    if any(s['conclusion'] == 'failure' for s in steps):
        return 'usage limit or setup failure: the model did no work'
    if run['conclusion'] == 'success' and not any(s['conclusion'] == 'success' for s in steps):
        return 'no model work: eligibility checks skipped the run'
    return ''


def runs_since(since):
    runs = orders.gh('api', 'repos/{owner}/{repo}/actions/runs?created=>=' + datetime.fromtimestamp(since, timezone.utc)
                     .strftime('%Y-%m-%dT%H:%M:%SZ') + '&per_page=100')['workflow_runs']
    lines = []
    for run in sorted(runs, key=lambda r: r['created_at']):
        name = run['path'].rsplit('/', 1)[-1].removesuffix('.yml')
        if name not in WORKFLOWS:
            continue
        result = run['conclusion'] or run['status']
        why = cause(run) if run['status'] == 'completed' else ''
        lines.append(f"- {name} {run['html_url']}: {result}" + (f' ({why})' if why else ''))
    return lines


def report(merge, now, issues_waiting, runs):
    last = f'#{merge[0]} {merge[1]} at {stamp(merge[2])}' if merge else 'none'
    age = f'{int((now - merge[2]) // 3600)} hours' if merge else 'unknown'
    return '\n'.join([f'Nothing merged to `main` for {age}.', '', f'Last merge: {last}', '', 'Waiting:',
                      *[f'- {line}' for line in issues_waiting], '', 'Runs since the last merge:',
                      *(runs or ['- none'])])


def open_body(key, text):
    return '\n'.join(['The loop stalled: work waits and nothing merges.', '', text, '', 'Scenarios: L94',
                      f'Specification: [scenarios/loop-rules.md]({SPEC})', f'Key: {key}', 'Priority: 0',
                      'Mode: implement', ''])


def close(issue, merge):
    builder.call('issue', 'close', str(issue['number']), '--reason', 'completed', '--comment',
                 f'PR #{merge[0]} {merge[1]} merged at {stamp(merge[2])}: the loop is moving again.')
    if builder.NEEDS_USER in {l['name'] for l in issue['labels']}:
        builder.call('api', '-X', 'DELETE', f"repos/{{owner}}/{{repo}}/issues/{issue['number']}/labels/{builder.NEEDS_USER}")


def stall(now):
    issues = [i for i in paged('repos/{owner}/{repo}/issues?state=open&per_page=100')
              if 'pull_request' not in i and orders.work_issue(i)]
    open_stalls = [i for i in issues if stall_key(i)]
    merge = last_merge()
    for issue in open_stalls:
        if merge and merge[2] > builder.created(issue['created_at']):
            close(issue, merge)
    open_stalls = [i for i in open_stalls if not (merge and merge[2] > builder.created(i['created_at']))]
    prs = orders.gh('pr', 'list', '--state', 'open', '--limit', '200', '--json',
                    'number,headRefName,isCrossRepository,labels')
    found = waiting(issues, prs)
    since = merge[2] if merge else now - ASK
    if not found or (merge and now - merge[2] < STALL):
        return
    text = report(merge, now, found, runs_since(since))
    if open_stalls:
        number = open_stalls[0]['number']
        builder.comment(number, text)
    else:
        key = 'stall-' + datetime.fromtimestamp(now, timezone.utc).strftime('%Y%m%d-%H')
        out = builder.call('issue', 'create', '--title', f'Loop stall {key}', '--label', 'loop:work', '--body',
                           open_body(key, text))
        number = int(out.strip().rsplit('/', 1)[-1])
    if not merge or now - merge[2] >= ASK:
        failing = [line for line in text.splitlines() if line.startswith('- ') and
                   any(word in line for word in (': failure', ': cancelled', 'usage limit', 'no model work'))]
        builder.flag(number, '\n'.join([f'The loop has stalled for {int((now - merge[2]) // 3600) if merge else 6}+ hours: '
                                        'the build runs meant to repair it are not working.', '', 'Failing runs:',
                                        *(failing or ['- none recorded'])]))


def main(args):
    if args:
        print('usage: stall.py', file=sys.stderr)
        return 2
    stall(time.time())
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main(sys.argv[1:]))
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(f'stall: {error}', file=sys.stderr)
        sys.exit(4)
