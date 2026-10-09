#!/usr/bin/env python3
"""L92: the Trail, each work Issue's record of its runs."""
import json
import os
import re
import subprocess
import sys
import time
from datetime import datetime

# A workflow or a person with write access (as in L46); everyone else's marker counts for nothing.
TRUSTED = {'OWNER', 'MEMBER', 'COLLABORATOR'}
BOTS = {'github-actions[bot]', 'claude[bot]'}
MARK = re.compile(r'\A<!-- trail (\d+) (\d+) -->\n')
LINKED = re.compile(r'^(?:Refs|Closes|Fixes|Resolves) #(\d+)\b', re.M | re.I)
DIAGNOSTIC = ("Read the Issue's Trail and the earlier runs' job logs first. "
              "Distinguish failed tests from cancelled CI, setup/authentication failure, stale work and publication failure. "
              "Post the observed cause and a changed approach as your `Changed:` entry before editing; "
              "do not repeat an unsuccessful approach or invent unavailable evidence. "
              "Reproduce the failure, make one bounded correction, and verify the failing observer. If the blocker is shared loop machinery, "
              "create or reuse one linked engineering work Issue with reproduction and acceptance, label loop:work and ready-for-agent only when specified, "
              "and keep this work linked; ordinary engineering failure is not a product question. "
              "Conclude with evidence of what changed and what remains.")


def gh(*args, data=None):
    out = subprocess.run(['gh', *args], input=data, text=True, capture_output=True, check=True, timeout=60).stdout
    return json.loads(out) if out.strip() else None


def epoch(text):
    return datetime.fromisoformat(text.replace('Z', '+00:00')).timestamp()


def comments(issue):
    """Every comment of the Issue, oldest first; malformed data raises, never reads as an empty Trail."""
    pages = gh('api', f'repos/{{owner}}/{{repo}}/issues/{issue}/comments?per_page=100', '--paginate', '--slurp')
    found = []
    for item in (c for page in pages for c in page):
        if not (isinstance(item.get('body'), str) and isinstance(item.get('id'), int) and isinstance(item.get('user'), dict)):
            raise ValueError('invalid comment data')
        login = item['user'].get('login')
        found.append(dict(id=item['id'], at=epoch(item['created_at']), url=item['html_url'], login=login, body=item['body'],
                          writer=item.get('author_association') in TRUSTED))
    return sorted(found, key=lambda c: (c['at'], c['id']))


def entries(found):
    """Trail entries: the marker counts from a bot of the loop or a writer, from no one else."""
    result = []
    for item in found:
        mark = MARK.match(item['body'])
        if not mark or not (item['login'] in BOTS or item['writer']):
            continue
        header = item['body'][mark.end():].split('\n\n', 1)[0]
        fields = dict(line.split(': ', 1) for line in header.splitlines() if ': ' in line)
        result.append(dict(item, run=mark[1], attempt=mark[2], fields=fields, kind=fields.get('Entry', '')))
    return result


def plan(found, now, edit_at=lambda: 0):
    """Whether the next build run is diagnostic and until when it waits (L92)."""
    ends = [e for e in entries(found) if e['kind'] == 'end']
    result = dict(diagnostic=False, until=None, url=None, streak=0)
    if not ends:
        return result
    last = ends[-1]
    def same(entry):
        return tuple(entry['fields'].get(name) for name in ('Cause', 'Delivery', 'Head')) == \
            tuple(last['fields'].get(name) for name in ('Cause', 'Delivery', 'Head'))
    streak = 0
    for entry in reversed(ends):
        if not same(entry):
            break
        streak += 1
    result.update(url=last['url'], streak=streak, diagnostic=streak >= 2)
    until = None
    if streak >= 3:
        until = last['at'] + min(7200, 900 * 2 ** (streak - 3))
        # A writer speaking after the last run lifts this wait; a bot never does.
        if any(c['writer'] and c['at'] > last['at'] for c in found) or edit_at() > last['at']:
            until = None
    builds = sorted(e['at'] for e in ends if e['fields'].get('Cause') == 'build' and e['at'] > now - 86400)
    if len(builds) >= 6:
        until = max(until or 0, builds[-6] + 86400)
    result['until'] = until if until is not None and until > now else None
    return result


def writer_edit(issue):
    """When a writer last edited the Issue's body, else 0. A bot's edit counts for nothing."""
    name = gh('api', 'repos/{owner}/{repo}')['full_name']
    owner, repo = name.split('/')
    query = 'query($o:String!,$r:String!,$n:Int!){repository(owner:$o,name:$r){issue(number:$n){userContentEdits(first:30){nodes{editedAt editor{login __typename}}}}}}'
    data = gh('api', 'graphql', '-f', f'query={query}', '-f', f'o={owner}', '-f', f'r={repo}', '-F', f'n={issue}')
    latest = 0
    for edit in data['data']['repository']['issue']['userContentEdits']['nodes'] or []:
        editor = edit.get('editor')
        if not editor or editor['__typename'] != 'User':
            continue
        permission = gh('api', f"repos/{{owner}}/{{repo}}/collaborators/{editor['login']}/permission")['permission']
        if permission in ('admin', 'maintain', 'write'):
            latest = max(latest, epoch(edit['editedAt']))
    return latest


def plan_for(issue, now=None):
    found = comments(issue)
    return plan(found, now or time.time(), lambda: writer_edit(issue)), found


def section(issue, now=None):
    """The task text of a build or fix run: the last six entries and the writers' words since the last end entry."""
    result, found = plan_for(issue, now)
    items = entries(found)
    if not items:
        return ''
    last_end = max((e['at'] for e in items if e['kind'] == 'end'), default=0)
    run, attempt = os.environ.get('GITHUB_RUN_ID', ''), os.environ.get('GITHUB_RUN_ATTEMPT', '1')
    lines = ['', f'## Trail of #{issue}', '', 'Earlier runs on this Issue, oldest first:']
    for entry in items[-6:]:
        lines += ['', f"[{entry['kind'] or 'entry'} by {entry['login']}]({entry['url']})", MARK.sub('', entry['body'])]
    spoken = [c for c in found if c['writer'] and c['at'] > last_end and not MARK.match(c['body'])]
    if spoken:
        lines += ['', 'Writers commented since the last end entry:']
        lines += [f"\n{c['login']}: {c['body']}" for c in spoken]
    lines += ['', f'Before your first commit, post one comment on #{issue} whose text starts with this marker line, then `Entry: changed`, then '
              '`Changed: <what differs from the last attempt>, <the evidence for it>`:', f'<!-- trail {run} {attempt} -->']
    if result['diagnostic']:
        lines += ['', 'Diagnostic run: the last runs ended the same way with no new head. ' + DIAGNOSTIC]
    return '\n'.join(lines)


def linked_issue(body):
    found = LINKED.findall(body or '')
    return int(found[0]) if found else None


def pr_section(pr):
    issue = linked_issue(gh('pr', 'view', str(pr), '--json', 'body')['body'])
    return section(issue) if issue else ''


def render(role, cause, delivery, head, result, changed, run, attempt, url):
    header = [f'<!-- trail {run} {attempt} -->', 'Entry: end', f'Role: {role}', f'Run: {url}', f'Cause: {cause}',
              f"Delivery: {delivery['state']}", f'Head: {head}', f"Result: {result['result']}"]
    if changed:
        header.append('Changed: none')
    note = ' (truncated)' if result.get('truncated') else ''
    return '\n'.join(header) + f"\n\nModel claim{note}:\n\n{result.get('text') or '(no explanation)'}"


def branch_head(branch):
    try:
        return gh('api', f'repos/{{owner}}/{{repo}}/git/ref/heads/{branch}')['object']['sha']
    except subprocess.CalledProcessError as error:
        if '404' in (error.stderr or '') or 'Not Found' in (error.stderr or ''):
            return None
        raise


def end(issue, role, cause, branch, start, pr=None):
    """Post the end entry for this run, once; the run's trusted completion job calls it."""
    import outcome
    run, attempt = os.environ['GITHUB_RUN_ID'], os.environ.get('GITHUB_RUN_ATTEMPT', '1')
    url = f"{os.environ['GITHUB_SERVER_URL']}/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{run}"
    found = entries(comments(issue))
    mine = [e for e in found if (e['run'], e['attempt']) == (run, attempt)]
    if any(e['kind'] == 'end' for e in mine):
        return
    result = json.loads(os.environ.get('RESULT') or 'null') or dict(result='missing', text='')
    delivery = outcome.observation(branch, pr)
    head = branch_head(branch)
    head = 'none' if head in (None, start) else head
    changed = head != 'none' and not any(e['kind'] == 'changed' for e in mine)
    gh('api', f'repos/{{owner}}/{{repo}}/issues/{issue}/comments', '--input', '-',
       data=json.dumps(dict(body=render(role, cause, delivery, head, result, changed, run, attempt, url))))


def repeats():
    """Open work Issues whose Trail ends the same way twice or more with no new head (L27)."""
    pages = gh('api', 'repos/{owner}/{repo}/issues?state=open&labels=loop%3Awork&per_page=100', '--paginate', '--slurp')
    rows = []
    for issue in (i for page in pages for i in page if 'pull_request' not in i):
        result = plan(comments(issue['number']), time.time())
        if result['streak'] >= 2:
            rows.append(dict(issue=issue['number'], title=issue['title'], count=result['streak'], url=result['url']))
    return rows


def main(argv):
    command = argv[1] if len(argv) > 1 else ''
    if command == 'section' and len(argv) == 3 and argv[2].isdigit():
        print(section(int(argv[2])))
    elif command == 'pr-section' and len(argv) == 3 and argv[2].isdigit():
        print(pr_section(argv[2]))
    elif command == 'end' and len(argv) == 7 and argv[2].isdigit():
        end(int(argv[2]), argv[3], argv[4], argv[5], argv[6])
    elif command == 'end-pr' and len(argv) == 7 and argv[2].isdigit():
        issue = linked_issue(gh('pr', 'view', argv[2], '--json', 'body')['body'])
        if issue:
            end(issue, argv[3], argv[4], argv[5], argv[6], argv[2])
    elif command == 'repeats':
        print(json.dumps(repeats()))
    else:
        sys.exit('usage: trail.py section <issue> | pr-section <pr> | end <issue> <role> <cause> <branch> <start> | end-pr <pr> <role> <cause> <branch> <start> | repeats')


if __name__ == '__main__':
    try:
        main(sys.argv)
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError):
        print('Trail unavailable: reading or posting an entry failed.', file=sys.stderr)
        sys.exit(4)
