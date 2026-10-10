#!/usr/bin/env python3
"""L34: open GitHub Issues are the work queue; scenario files describe acceptance."""
import json
import re
import subprocess
import sys
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor

NEEDS_USER = 'flag:needs-user'
# The repository is public: only its collaborators and the loop's own Apps file work, or vouch for an Issue with
# `loop:work`, which only collaborators can apply.
AUTHORS = ('OWNER', 'MEMBER', 'COLLABORATOR')
BOTS = ('claude[bot]', 'github-actions[bot]')


def gh(*args):
    return json.loads(subprocess.run(['gh', *args], check=True, text=True,
                                   stdout=subprocess.PIPE, timeout=30).stdout)


def field(body, name, default=None):
    found = re.findall(r'^' + re.escape(name) + r': (.+)$', body, re.M)
    if not found and default is not None:
        return default
    if len(found) != 1:
        raise ValueError(f'needs one {name}: line')
    return found[0].strip()


def work_issue(issue):
    """Whether the Issue may run: a collaborator or a loop App filed it, or a collaborator labelled it `loop:work`."""
    return (issue.get('author_association') in AUTHORS or (issue.get('user') or {}).get('login') in BOTS or
            'loop:work' in {label['name'] for label in issue['labels']})


def order(issue):
    body = issue['body'] or ''
    if not re.search(r'^Key: ', body, re.M):
        # A request filed without queue fields (a bug report): a Builder specifies it before anyone builds it.
        return dict(mode='specify', issue=issue['number'], url=issue['html_url'], ids=issue['title'], file='',
                    slug=f'issue-{issue["number"]}', priority=50, request=True, respecify=False)
    key = field(body, 'Key')
    if not re.fullmatch(r'[A-Za-z0-9]+(?:-[A-Za-z0-9]+)*', key):
        raise ValueError('invalid Key')
    spec = field(body, 'Specification')
    # The Issue form displays the specification as a clickable repository link.
    match = re.fullmatch(r'\[(scenarios/[A-Za-z0-9_-]+\.md)\]\(https://github\.com/[^\s]+\)', spec)
    if not match:
        raise ValueError('Specification must link one scenario file')
    priority = int(field(body, 'Priority'))
    if not 0 <= priority <= 100:
        raise ValueError('Priority must be between 0 and 100')
    mode = field(body, 'Mode', 'implement')
    if mode not in ('implement', 'specify'):
        raise ValueError('invalid Mode')
    row = dict(mode=mode, issue=issue['number'], url=issue['html_url'], ids=field(body, 'Scenarios'),
               file=match[1], slug=key, priority=priority, request=False, respecify=False)
    headings = {name for path in Path('scenarios').glob('*.md')
                for name in re.findall(r'^\*\*([A-Z][0-9]+)[ .]', path.read_text(), re.M)}
    names = []
    for first, last in re.findall(r'\b([A-Z][0-9]+)(?:\s*(?:–|-|to)\s*[A-Z]?([0-9]+))?\b', row['ids']):
        names.extend(first[0] + str(n) for n in range(int(first[1:]), int(last or first[1:]) + 1))
    if row['mode'] == 'implement' and (not names or not Path(row['file']).is_file() or
                                       any(name not in headings for name in names)):
        # Its acceptance is not on this checkout: a Builder specifies it first, then the queue implements it.
        row.update(mode='specify', respecify=True)
    return row


def classify(issues, dependencies, prs):
    """Return observable states; malformed orders and unavailable dependencies never become ready."""
    rows = []
    keys = {}
    cycles = set()
    visited = set()
    def visit(number, path):
        if number in path:
            cycles.update(path[path.index(number):])
            return
        if number in visited:
            return
        for dep in dependencies.get(number, []):
            if dep['state'] != 'closed' or dep.get('state_reason') != 'completed':
                visit(dep['number'], path + [number])
        visited.add(number)
    for number in dependencies:
        visit(number, [])
    for issue in issues:
        number = issue['number']
        row = dict(issue=number, url=issue['html_url'], ids=issue['title'], file='', slug='', priority=100)
        if not work_issue(issue):
            row.update(state='unspecified', reason='filed outside the repository; a collaborator adds loop:work to vouch for it')
            rows.append(row)
            continue
        try:
            row = order(issue)
            keys.setdefault(row['slug'], []).append(number)
            if issue['state'] == 'closed':
                row.update(state='done' if issue.get('state_reason') == 'completed' else 'cancelled', reason='Issue closed')
            else:
                active = [pr['number'] for pr in prs if not pr.get('isCrossRepository', False) and
                          (pr['headRefName'] == 'build/' + row['slug'] or
                           re.search(r'^(?:Refs|Closes|Fixes|Resolves) #' + str(number) + r'\b', pr.get('body') or '', re.M | re.I))]
                blocked = [dep for dep in dependencies.get(number, [])
                           if dep['state'] != 'closed' or dep.get('state_reason') != 'completed']
                if active:
                    row.update(state='in-flight', reason=' '.join(f'#{n}' for n in active))
                elif NEEDS_USER in {label['name'] for label in issue['labels']} and not row['slug'].startswith('stall-'):
                    row.update(state='waiting', reason=f'waits on the user ({NEEDS_USER})')
                elif number in cycles:
                    row.update(state='blocked', reason='dependency cycle; engineering must repair the graph')
                elif blocked:
                    row.update(state='waiting', reason='blocked by ' + ', '.join(f'#{dep["number"]}' for dep in blocked))
                else:
                    row.update(state='ready', reason='')
        except (KeyError, TypeError, ValueError) as error:
            row.update(state='blocked', reason=f'invalid work order: {error}')
        rows.append(row)
    for row in rows:
        if row['state'] not in ('done', 'cancelled') and len(keys.get(row['slug'], [])) > 1:
            row.update(state='blocked', reason='duplicate work Key; engineering must reconcile the Issues')
    return sorted(rows, key=lambda row: (row['priority'], row['issue']))


def read_orders():
    pages = gh('api', 'repos/{owner}/{repo}/issues?state=all&per_page=100', '--paginate', '--slurp')
    issues = [issue for page in pages for issue in page if 'pull_request' not in issue]
    def dependencies(issue):
        if issue['state'] == 'closed' or issue.get('issue_dependencies_summary', {}).get('total_blocked_by') == 0:
            return issue['number'], []
        pages = gh('api', f'repos/{{owner}}/{{repo}}/issues/{issue["number"]}/dependencies/blocked_by?per_page=100',
                   '--paginate', '--slurp')
        return issue['number'], [dependency for page in pages for dependency in page]
    with ThreadPoolExecutor(max_workers=8) as pool:
        blocked = dict(pool.map(dependencies, issues))
    prs = gh('pr', 'list', '--state', 'open', '--limit', '1000', '--json', 'number,headRefName,body,isCrossRepository')
    return classify(issues, blocked, prs)


class NotReady(Exception):
    """The Issue is no longer ready; starting no run is the correct outcome."""


def queued(number):
    """True when the queue would build Issue `number` now: no dependency, PR or missing field holds it."""
    current = next((row for row in read_orders() if row['issue'] == number), None)
    return current is not None and current['state'] == 'ready'


def task(number):
    """Print the Builder's task for a ready Issue; raise NotReady when another run or a person took it."""
    issue = gh('api', f'repos/{{owner}}/{{repo}}/issues/{number}')
    if issue['state'] != 'open' or not work_issue(issue):
        raise NotReady(f'Issue #{number} is no longer authorized for execution')
    row = order(issue)
    if not queued(number):
        raise NotReady(f'Issue #{number} is no longer ready')
    if row['request']:
        completion = ('This Issue is a request without queue fields. Add its acceptance as scenarios, then edit this Issue '
                      'to add Key, Priority, Specification and Scenarios lines and Mode: implement, so the queue builds it '
                      'once the scenarios are on main. Never close it from this PR.')
    elif row['respecify']:
        completion = (f"This implementation Issue names scenarios ({row['ids']}) that {row['file']} on main does not hold. "
                      'Add its acceptance as scenarios, then edit this Issue so its Scenarios line names their IDs and it '
                      'keeps Mode: implement, so the queue builds it once the scenarios are on main. Never close it from this PR.')
    elif row['mode'] == 'specify':
        completion = ('Close this specification Issue only after its specification acceptance is demonstrated; implementation '
                      f'stays in its dependent Issue. Use Closes #{number} when that condition holds.')
    else:
        completion = ('Close this implementation Issue only when its full acceptance is demonstrated; a specification-only PR '
                      f'does not close it. Use Closes #{number} when that condition holds.')
    print(f"Work order: {issue['html_url']}\n\n{issue['body']}\n\n"
          f"Mode: {row['mode']}. Use this Issue as the canonical work order. Link the PR with Refs #{number}. {completion} "
          "Put progress and unresolved engineering questions on the Issue, not in scenario Work tables.")


def main():
    if sys.argv[1:] == ['lint']:
        stale = [str(path) for path in Path('scenarios').glob('*.md')
                 if re.search(r'^## Work$', path.read_text(), re.M)]
        if stale:
            raise ValueError('work orders belong in GitHub Issues, not tables: ' + ', '.join(stale))
        return
    if len(sys.argv) == 3 and sys.argv[1] == 'task' and sys.argv[2].isdigit():
        try:
            task(int(sys.argv[2]))
        except NotReady as refusal:
            print(f'work orders: {refusal}', file=sys.stderr)
            sys.exit(3)
        return
    if sys.argv[1:] not in (['ready'], ['ready', '--json']):
        raise ValueError('usage: orders.py ready [--json] | task <issue> (exit 3: not ready)')
    rows = read_orders()
    if sys.argv[-1] == '--json':
        print(json.dumps(rows))
    else:
        for row in rows:
            print(row['state'], row['ids'], row['file'], f'#{row["issue"]}', row['reason'])


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(f'work orders: {error}', file=sys.stderr)
        sys.exit(4)
