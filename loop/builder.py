#!/usr/bin/env python3
"""L23, L24, L46, L79, L81, L82: what GitHub Actions leaves to roundup around Builder runs: which Issues to build, how a run
ended, when a PR gets a fix run, how a review run's answer reaches the PR and when a PR waits on the user's `macOS:`
line. Exit 4: GitHub could not be read or written."""
import json
import os
import re
import secrets
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import orders  # noqa: E402

BOT = 'github-actions[bot]'
NEEDS_USER = orders.NEEDS_USER
ATTEMPTS = 3
STRIKES = 3
DAY = 24 * 3600
# A draft PR untouched this long has no live run: build and fix jobs time out after two hours.
IDLE = 3 * 3600
# A review run whose model did no work (a usage limit) leaves `review` pending; pick reruns it this many times in all.
RERUNS = 6
FAILED = '`{cause}` failed on this head. `gh pr checks {pr}` lists the failed run; `gh run view <id> --log-failed` shows why.'
CAUSES = {
    'check': FAILED,
    'rules': FAILED,
    'review': 'The review run rejected this head; its findings are the last verdict below.',
    'conflict': 'The PR conflicts with `main`: merge `origin/main` and keep the behaviour of both sides.',
    'unfinished': 'The last run left this PR a draft without a `Stopped:` line: finish the work.',
}
MACOS = ('This PR changes `crates/desktop/`, which Percy cannot render: run `just app` on a Mac, then add a '
         '`macOS: <what you saw>` line to the PR body.')


def call(*args):
    return subprocess.run(['gh', *args], check=True, text=True, stdout=subprocess.PIPE, timeout=60).stdout


def created(item):
    return datetime.fromisoformat(item.replace('Z', '+00:00')).timestamp()


def marked(number, marker, since=0):
    """This bot's comments on an Issue or PR that start with `marker`, made at or after `since`, oldest first."""
    pages = orders.gh('api', f'repos/{{owner}}/{{repo}}/issues/{number}/comments?per_page=100', '--paginate', '--slurp')
    return [c for page in pages for c in page
            if c['user']['login'] == BOT and c['body'].startswith(marker) and created(c['created_at']) >= since]


def comment(number, body):
    call('api', f'repos/{{owner}}/{{repo}}/issues/{number}/comments', '-f', f'body={body}')


def labels(number):
    return {label['name'] for label in orders.gh('api', f'repos/{{owner}}/{{repo}}/issues/{number}')['labels']}


def flag(number, why):
    """Ask the user once: label the Issue or PR `flag:needs-user` and say why, unless it already waits on them."""
    if NEEDS_USER in labels(number):
        return
    call('api', f'repos/{{owner}}/{{repo}}/issues/{number}/labels', '-f', f'labels[]={NEEDS_USER}')
    comment(number, f'<!-- needs-user -->\n{why}')


def dispatch_fix(pr, head, cause):
    call('workflow', 'run', 'fix.yml', '-f', f'pr={pr}', '-f', f'head={head}', '-f', f'cause={cause}')


def prompt(task, output):
    """Append the step output `prompt`: `.agents/builder.md`, a `## Task` heading, then the task."""
    delimiter = 'PROMPT_' + secrets.token_hex(8)
    role = Path('.agents/builder.md').read_text()
    with open(output, 'a') as out:
        out.write(f'prompt<<{delimiter}\n{role}\n## Task\n\n{task.rstrip()}\n{delimiter}\n')


def worked(path):
    """A run worked when its model used a tool; a usage limit or a setup failure leaves no tool use."""
    if not path or not Path(path).is_file():
        return False
    return any(entry.get('type') == 'assistant' and
               any(part.get('type') == 'tool_use' for part in entry.get('message', {}).get('content', []))
               for entry in json.loads(Path(path).read_text()))


def busy():
    """Issues whose `build #<issue> / build` job (`build #<issue>` before build-issue.yml) has not completed in any
    unfinished build.yml run; its `after` job holds no slot."""
    issues = set()
    for status in ('requested', 'pending', 'waiting', 'queued', 'in_progress'):
        runs = orders.gh('api', f'repos/{{owner}}/{{repo}}/actions/workflows/build.yml/runs?status={status}&per_page=100')
        for run in runs['workflow_runs']:
            for job in orders.gh('api', f'repos/{{owner}}/{{repo}}/actions/runs/{run["id"]}/jobs?per_page=100')['jobs']:
                match = re.fullmatch(r'build #(\d+)(?: / build)?', job['name'])
                if match and job['status'] != 'completed':
                    issues.add(int(match[1]))
    return issues


def choose(rows, taken, most):
    """Ready rows no build job holds, in priority order, filling the slots `most` leaves after `taken`."""
    free = [row for row in rows if row['state'] == 'ready' and row['issue'] not in taken]
    return [dict(issue=row['issue'], slug=row['slug']) for row in free[:max(0, most - len(taken))]]


def failing(pr):
    """The first required check, by name, that failed on the PR's head."""
    for check in pr.get('statusCheckRollup') or []:
        name = check.get('name') or check.get('context')
        if name in ('check', 'rules', 'review') and (check.get('conclusion') or check.get('state')) == 'FAILURE':
            return name
    return None


def stopped(pr):
    """Whether the PR body holds a `Stopped:` line: the Builder ended on a question only the user can answer."""
    return bool(re.search(r'^Stopped:', pr['body'] or '', re.M))


def repairs(prs, now):
    """(pr, head, cause) for each open Builder PR that needs a fix run: a conflict, a failed required check, or a draft
    without a `Stopped:` line that no run has touched for IDLE seconds."""
    due = []
    for pr in prs:
        if not pr['headRefName'].startswith('build/') or pr['isCrossRepository']:
            continue
        if pr['isDraft']:
            if now - created(pr['updatedAt']) >= IDLE and not stopped(pr):
                due.append((pr['number'], pr['headRefOid'], 'unfinished'))
        elif pr['mergeable'] == 'CONFLICTING':
            due.append((pr['number'], pr['headRefOid'], 'conflict'))
        elif cause := failing(pr):
            due.append((pr['number'], pr['headRefOid'], cause))
    return due


def unreviewed(prs):
    """(pr, run id) for each ready PR whose `review` status is pending: its review run did no model work."""
    return [(pr['number'], check['targetUrl'].rstrip('/').rsplit('/', 1)[1])
            for pr in prs if not pr['isDraft']
            for check in pr.get('statusCheckRollup') or []
            if check.get('context') == 'review' and check.get('state') == 'PENDING']


def rerun_review(pr, run):
    """Rerun an ended review run that did no model work; after RERUNS attempts, ask the user instead."""
    state = orders.gh('run', 'view', run, '--json', 'status,attempt,url')
    if state['status'] != 'completed':
        return
    if state['attempt'] >= RERUNS:
        flag(pr, f"The review run {state['url']} did no model work in {RERUNS} attempts: a usage limit or a setup "
                 'failure. Rerun it from that page once it can work.')
        return
    call('run', 'rerun', run)


def pick(most):
    """Start the fix runs open Builder PRs need, rerun review runs that did no model work, then print the build matrix
    as JSON."""
    prs = orders.gh('pr', 'list', '--state', 'open', '--limit', '200', '--json',
                    'number,headRefName,headRefOid,isDraft,isCrossRepository,mergeable,body,updatedAt,statusCheckRollup')
    for pr, head, cause in repairs(prs, time.time()):
        dispatch_fix(pr, head, cause)
    for pr, run in unreviewed(prs):
        rerun_review(pr, run)
    print(json.dumps(choose(orders.read_orders(), busy(), most)))


def ended(pr):
    """After a run that worked: a draft with a `Stopped:` line waits on the user; another draft gets a fix run."""
    if not pr['isDraft']:
        return
    if stopped(pr):
        flag(pr['number'], 'The Builder stopped on a question only you can answer: see the `Stopped:` line in the PR body.')
    else:
        dispatch_fix(pr['number'], pr['headRefOid'], 'unfinished')


def built(issue, slug, ran, run_url):
    """After a build run: hand its PR on, or settle the Issue it left. A run that made no tool call (`ran` false)
    records nothing. With no open PR, an Issue a person closed, one closed after its `build/<slug>` PR merged, or one a
    dependency, PR or `flag:needs-user` holds, records nothing; one a bot closed otherwise reopens; one still ready is
    struck, and STRIKES in a day wait on the user."""
    if not ran:
        print('the run made no tool call; nothing to record')
        return
    prs = orders.gh('pr', 'list', '--head', f'build/{slug}', '--state', 'open', '--json', 'number,headRefOid,isDraft,body')
    if prs:
        ended(prs[0])
        return
    current = orders.gh('api', f'repos/{{owner}}/{{repo}}/issues/{issue}')
    if current['state'] == 'closed':
        if slug.startswith('stall-') or (current.get('closed_by') or {}).get('type') != 'Bot' or \
                orders.gh('pr', 'list', '--head', f'build/{slug}', '--state', 'merged', '--json', 'number'):
            print(f'#{issue} closed by a person or after its build/{slug} PR merged; nothing to record')
            return
        call('issue', 'reopen', str(issue))
    elif NEEDS_USER in labels(issue):
        print(f'#{issue} waits on the user; nothing to record')
        return
    elif not orders.queued(issue):
        print(f'#{issue} is no longer ready; nothing to record')
        return
    strike(issue, f'The build run {run_url} ended without a PR.')


def strike(issue, why):
    """Record one failed try on the Issue; STRIKES in a day take it off the queue and wait on the user."""
    comment(issue, f'<!-- strike -->\n{why}')
    if len(marked(issue, '<!-- strike -->', time.time() - DAY)) >= STRIKES:
        flag(issue, f'{STRIKES} tries in 24 hours (build runs that ended without a PR, or PRs that used their fix runs) failed, so the queue stops building this Issue until '
                    f'you remove `{NEEDS_USER}`. The strike comments link each run.')


def view(pr):
    return orders.gh('pr', 'view', str(pr), '--json', 'number,url,state,headRefOid,headRefName,isDraft,isCrossRepository,body')


def started(attempt):
    """When the fix run an attempt comment links began, or None when its run is not readable."""
    run = re.search(r'/runs/(\d+)', attempt)
    began = (orders.gh('api', f'repos/{{owner}}/{{repo}}/actions/runs/{run[1]}') if run else {}).get('run_started_at')
    return created(began) if began else None


def fix_task(pr, head, cause):
    """The fix run's task, or None when the PR moved past `head`, is not a Builder PR, or used its ATTEMPTS."""
    current = view(pr)
    if current['state'] != 'OPEN' or current['headRefOid'] != head or not current['headRefName'].startswith('build/'):
        return None
    attempts = marked(pr, '<!-- fix-attempt -->')
    if len(attempts) >= ATTEMPTS:
        exhaust(current, attempts, cause)
        return None
    findings = [c['body'] for c in marked(pr, 'VERDICT:')][-1:] if cause == 'review' else []
    return '\n\n'.join([f"Fix PR #{pr} on branch `{current['headRefName']}`, head {head}. " +
                        CAUSES[cause].format(cause=cause, pr=pr), *findings,
                        'Push to that branch. End as `.agents/builder.md` says: a ready PR with auto-merge armed, or a '
                        'draft with a `Stopped:` line.'])


def exhaust(current, attempts, cause):
    """A Builder PR out of fix runs: tell its work Issue what failed, close the PR unmerged, delete its branch and strike
    the Issue, once per PR. The closed PR keeps its diff at `refs/pull/<pr>/head`. A PR naming no Issue asks the user."""
    pr, branch = current['number'], current['headRefName']
    match = re.search(r'^(?:Refs|Closes|Fixes) #(\d+)', current['body'] or '', re.M)
    if not match:
        flag(pr, f'{ATTEMPTS} fix runs did not get this PR to merge, and its body names no Issue to record that on.')
        return
    issue = int(match[1])
    if marked(issue, f'<!-- fix-exhausted #{pr} -->'):
        return
    commits = orders.gh('pr', 'view', str(pr), '--json', 'commits')['commits']
    verdicts = marked(pr, 'VERDICT:')

    def reason(cause, before):
        found = [c['body'] for c in verdicts if created(c['created_at']) <= before][-1:] or ['']
        return f"review rejected: {found[0].split(chr(10) * 2, 1)[-1].strip()}" if cause == 'review' else f'{cause} failed'

    lines, since = [], None
    for attempt in attempts:
        ended_at = created(attempt['created_at'])
        cause_ran = (re.search(r'answered `(\w+)`', attempt['body']) or [None, '?'])[1]
        if since is None:
            since = started(attempt['body']) or ended_at
        subjects = [c['messageHeadline'] for c in commits if since < created(c['committedDate']) <= ended_at]
        lines.append(f"- `{cause_ran}` ({reason(cause_ran, ended_at)}): " + ('; '.join(subjects) or 'no commit'))
        since = ended_at
    comment(issue, f'<!-- fix-exhausted #{pr} -->\nPR #{pr} ({current["url"]}) used its {ATTEMPTS} fix runs and was closed '
                   f'unmerged; its diff stays at that link, head `{current["headRefOid"]}`. Each line is a cause a fix '
                   f'run answered, its failing check or review finding, and the commits it pushed; the last is the cause '
                   f'that found no run left.\n\n'
                   + '\n'.join(lines) + f'\n- `{cause}` ({reason(cause, time.time())}): no fix run left\n\nTake another approach.')
    strike(issue, f'PR #{pr} used its {ATTEMPTS} fix runs: {current["url"]}')
    call('pr', 'close', str(pr), '--delete-branch')


def fixed(pr, cause, ran, run_url):
    """After a fix run that worked (`ran`): count the attempt on the PR, then hand the PR on."""
    if not ran:
        print('the run made no tool call; it counts as no attempt')
        return
    comment(pr, f'<!-- fix-attempt -->\nFix run {run_url} answered `{cause}`.')
    ended(view(pr))


def review_task(pr, head):
    """The review run's task, or None when the PR is not open at `head` or is a draft."""
    current = view(pr)
    if current['state'] != 'OPEN' or current['headRefOid'] != head or current['isDraft']:
        return None
    lines = [f'Review PR #{pr}, head {head}.', *re.findall(r'^(?:Scenarios|Moves|macOS):.*$', current['body'] or '', re.M)]
    if any(path.startswith('apps/desktop/src/') for path in call('pr', 'diff', str(pr), '--name-only').split()):
        percy = subprocess.run(['loop/percy.sh', 'build', head], text=True, stdout=subprocess.PIPE)
        if percy.returncode == 0:
            lines.append(f'It touches apps/desktop/src: run percy-review on Percy build {percy.stdout.strip()}, made for '
                         'this head; with a build id it needs no .percy/config.yml. Each diff must match a then-clause of '
                         'the scenarios above or a D check the Moves: line names; nothing the author wrote counts as '
                         'intent (rule 5).')
        elif percy.returncode == 1:
            lines.append('It touches apps/desktop/src, but no Percy build exists for this head: review the code alone and '
                         'say so in a NOTE.')
        else:
            raise OSError('the Percy API failed')
    earlier = [c['body'] for c in marked(pr, 'VERDICT:')]
    if earlier:
        lines += ['If an earlier approval names an ancestor, focus on the changes since that head, including conflict '
                  'resolutions and their effects. Your verdict still covers the current head; do not repeat unchanged '
                  'findings.', '\nEarlier verdicts, oldest first:\n\n' + '\n\n---\n\n'.join(earlier)]
    return '\n'.join(lines)


def status(head, state, description, run_url):
    call('api', f'repos/{{owner}}/{{repo}}/statuses/{head}', '-f', f'state={state}', '-f', 'context=review',
         '-f', f'description={description}', '-f', f'target_url={run_url}')


def review_post(pr, head, reviewer, answer, ran, run_url):
    """Publish a review run's answer on `head`: a VERDICT comment and the `review` status; a reject on a Builder PR
    starts a fix run. Lines that would forge a verdict field are dropped. A run whose model did no work (`ran` false)
    leaves `review` pending for pick to rerun. False when a run that worked holds no verdict."""
    current = view(pr)
    if current['state'] != 'OPEN' or current['headRefOid'] != head:
        print(f'PR #{pr} moved past {head}; the newer head gets its own review')
        return True
    answer = json.loads(answer) if answer.strip() else {}
    verdict = answer.get('verdict')
    if verdict not in ('approve', 'reject') and not ran:
        status(head, 'pending', 'The review run did no model work; the next build pick reruns it', run_url)
        return True
    if verdict not in ('approve', 'reject'):
        status(head, 'error', 'The review run gave no verdict', run_url)
        flag(pr, f'The review run {run_url} gave no verdict on {head}; rerun it from that page.')
        return False
    findings = '\n'.join(line for line in str(answer.get('findings', '')).splitlines()
                         if not re.match(r'(VERDICT|Head|Reviewed-by-Agent):', line))
    comment(pr, f'VERDICT: {verdict}\nHead: {head}\n\n{findings}\n\nReviewed-by-Agent: {reviewer}')
    status(head, 'success' if verdict == 'approve' else 'failure', f'The review run answered {verdict}', run_url)
    if verdict == 'reject' and current['headRefName'].startswith('build/') and not current['isCrossRepository']:
        dispatch_fix(pr, head, 'review')
    return True


def macos_line(pr):
    """False, asking the user, when a ready PR changes `crates/desktop/` and its body has no `macOS:` line. Once the line
    is there, the label comes off if that question was the last one asked."""
    current = view(pr)
    pages = orders.gh('api', f'repos/{{owner}}/{{repo}}/pulls/{pr}/files?per_page=100', '--paginate', '--slurp')
    if current['isDraft'] or not any(f['filename'].startswith('crates/desktop/') for page in pages for f in page):
        return True
    if not re.search(r'^macOS: \S', current['body'] or '', re.M):
        flag(pr, MACOS)
        return False
    asked = marked(pr, '<!-- needs-user -->')
    if asked and asked[-1]['body'] == f'<!-- needs-user -->\n{MACOS}' and NEEDS_USER in labels(pr):
        call('api', '-X', 'DELETE', f'repos/{{owner}}/{{repo}}/issues/{pr}/labels/{NEEDS_USER}')
    return True


def task_or_nothing(task):
    """Print a task and exit 0, or exit 3 when there is none: the run has nothing to do."""
    print(task or '', end='')
    return 0 if task else 3


def main(args):
    run_url = (f"{os.environ.get('GITHUB_SERVER_URL', '')}/{os.environ.get('GITHUB_REPOSITORY', '')}"
               f"/actions/runs/{os.environ.get('GITHUB_RUN_ID', '')}")
    match args:
        case ['prompt']:
            prompt(sys.stdin.read(), os.environ['GITHUB_OUTPUT'])
        case ['worked', path]:
            return 0 if worked(path) else 1
        case ['pick', most] if most.isdigit():
            pick(int(most))
        case ['built', issue, slug, ran] if issue.isdigit() and ran in ('true', 'false'):
            built(int(issue), slug, ran == 'true', run_url)
        case ['fix-task', pr, head, cause] if pr.isdigit() and cause in CAUSES:
            return task_or_nothing(fix_task(int(pr), head, cause))
        case ['fixed', pr, cause, ran] if pr.isdigit() and ran in ('true', 'false'):
            fixed(int(pr), cause, ran == 'true', run_url)
        case ['review-task', pr, head] if pr.isdigit():
            return task_or_nothing(review_task(int(pr), head))
        case ['review-post', pr, head, reviewer, ran] if pr.isdigit() and ran in ('true', 'false'):
            return 0 if review_post(int(pr), head, reviewer, sys.stdin.read(), ran == 'true', run_url) else 1
        case ['macos-line', pr] if pr.isdigit():
            return 0 if macos_line(int(pr)) else 1
        case _:
            print('usage: builder.py prompt | worked <file> | pick <most> | built <issue> <slug> <ran> | '
                  'fix-task <pr> <head> <cause> | fixed <pr> <cause> <ran> | review-task <pr> <head> | '
                  'review-post <pr> <head> <reviewer> <ran> | macos-line <pr>', file=sys.stderr)
            return 2
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main(sys.argv[1:]))
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(f'builder: {error}', file=sys.stderr)
        sys.exit(4)
