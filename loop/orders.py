#!/usr/bin/env python3
"""L34: GitHub Issues are the work queue; scenario files describe acceptance."""
import json
import re
import subprocess
import sys
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor


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


def order(issue):
    body = issue['body'] or ''
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
    provider = field(body, 'Provider', 'any')
    if mode not in ('implement', 'specify') or provider not in ('any', 'claude', 'codex'):
        raise ValueError('invalid Mode or Provider')
    row = dict(mode=mode, provider=provider, issue=issue['number'], url=issue['html_url'], ids=field(body, 'Scenarios'),
                file=match[1], slug=key, priority=priority)
    if row['mode'] == 'implement' and not Path(row['file']).is_file():
        raise ValueError(f"missing specification {row['file']}")
    headings = {name for path in Path('scenarios').glob('*.md')
                for name in re.findall(r'^\*\*([A-Z][0-9]+)[ .]', path.read_text(), re.M)}
    names = []
    for first, last in re.findall(r'\b([A-Z][0-9]+)(?:\s*(?:–|-|to)\s*[A-Z]?([0-9]+))?\b', row['ids']):
        names.extend(first[0] + str(n) for n in range(int(first[1:]), int(last or first[1:]) + 1))
    if not names or (row['mode'] == 'implement' and any(name not in headings for name in names)):
        raise ValueError('acceptance scenarios are not specified on this checkout')
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
                elif 'ready-for-agent' not in {label['name'] for label in issue['labels']}:
                    row.update(state='unspecified', reason='scope or dependencies need engineering triage')
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
    pages = gh('api', 'repos/{owner}/{repo}/issues?state=all&labels=loop%3Awork&per_page=100', '--paginate', '--slurp')
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


def task(number):
    issue = gh('api', f'repos/{{owner}}/{{repo}}/issues/{number}')
    if issue['state'] != 'open' or 'ready-for-agent' not in {label['name'] for label in issue['labels']}:
        raise ValueError(f'Issue #{number} is no longer authorized for execution')
    row = order(issue)
    rows = read_orders()
    current = next((row for row in rows if row['issue'] == number), None)
    if current is None or current['state'] not in ('ready', 'in-flight'):
        raise ValueError(f'Issue #{number} is no longer eligible: {current}')
    print(f"Work order: {issue['html_url']}\n\n{issue['body']}\n\n"
          f"Mode: {row['mode']}. Use this Issue as the canonical work order. Link the PR with Refs #{number}. "
          f"Use Closes #{number} only when its full acceptance is demonstrated; a specification-only PR does not close it. "
          "Put progress and unresolved engineering questions on the Issue, not in scenario Work tables.")


def main():
    if sys.argv[1:] == ['lint']:
        stale = [str(path) for path in Path('scenarios').glob('*.md')
                 if re.search(r'^## Work$', path.read_text(), re.M)]
        if stale:
            raise ValueError('work orders belong in GitHub Issues, not tables: ' + ', '.join(stale))
        return
    if len(sys.argv) == 3 and sys.argv[1] == 'task' and sys.argv[2].isdigit():
        task(int(sys.argv[2]))
        return
    if sys.argv[1:] not in (['ready'], ['ready', '--json']):
        raise ValueError('usage: orders.py ready [--json] | task <issue>')
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
