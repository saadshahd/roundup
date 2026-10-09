#!/usr/bin/env python3
"""L90: reviewed proposals publish native Issues; source Issues retain human feedback."""
import argparse
import base64
import functools
import hashlib
import json
import os
import re
import subprocess
import sys
import dispatch as d
import orders
from outcome import describe, redact

NAMESPACE = 'loop-feedback'
# PR changes and finished runs touch no feedback Issue; each feedback run dispatches its own sweep (L88).
UNSWEPT_EVENTS = ('workflow_run', 'pull_request_target')


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()


def intake(issue, trusted, previous):
    labels = {label['name'] for label in issue['labels']}
    # A partially published child is still owned by its reviewed parent proposal.
    if previous is None and re.search(r'(?m)^<!-- feedback-delivery [1-9][0-9]* [A-Za-z0-9-]+ -->$', issue['body'] or ''):
        return None
    revision = digest([issue['title'].strip(), (issue['body'] or '').strip()])
    # #401 explicitly preserves the existing report-only #351, including before labelling.
    if issue['number'] == 351 or 'loop:status' in labels or 'loop:report-only' in labels:
        return None
    if issue['state'] != 'open' and (not previous or not trusted or
            revision == previous['revision']):
        return None
    if re.search(r'(?im)^\s*(?:Instruction:\s*)?loop:report-only\s*$', issue['body'] or ''):
        return None
    if 'loop:work' in labels and 'ready-for-agent' in labels and previous is None:
        return None
    # Progress comments and scheduling labels never enter the human revision hash.
    if not trusted and not ('loop:work' in labels and previous is None):
        return previous
    snapshot = {name: issue[name] for name in ('number', 'title', 'body', 'html_url')}
    snapshot['kind'] = previous['kind'] if previous and previous['revision'] == revision else ('work' if 'loop:work' in labels else 'feedback')
    snapshot['revision'] = revision
    return snapshot


def require(condition, message):
    if not condition:
        raise ValueError(message)


def parse_text(value, name, maximum=60000):
    require(isinstance(value, str) and 0 < len(value.strip()) <= maximum, f'invalid {name}')
    return value


def parse_answer(answer, phase, previous=None):
    """Model output is data: only this closed set of operations reaches publication."""
    require(isinstance(answer, dict) and set(answer) == {'goals', 'deliveries', 'evidence', 'rules', 'question'}, 'invalid answer fields')
    goals = answer['goals']
    require(isinstance(goals, list) and 1 <= len(goals) <= 12, 'one to twelve observable goals required')
    for goal in goals:
        require(isinstance(goal, dict) and set(goal) == {'statement', 'observer'}, 'invalid goal')
        parse_text(goal['statement'], 'goal')
        parse_text(goal['observer'], 'observer')
    if previous:
        require(goals == previous['answer']['goals'], 'verification must retain every original goal')
    question = answer['question']
    require(isinstance(question, str), 'invalid question')
    if question:
        parse_text(question, 'question')
        require('Consequence:' in question and 'Recommendation:' in question, 'question needs consequence and recommendation')
    deliveries = answer['deliveries']
    require(isinstance(deliveries, list) and len(deliveries) <= 4, 'a batch holds at most four deliveries')
    keys = set()
    owned = set()
    for delivery in deliveries:
        require(isinstance(delivery, dict) and set(delivery) == {'issue', 'title', 'body', 'blocked_by'}, 'invalid delivery fields')
        number = delivery['issue']
        require(type(number) is int and number >= 0, 'invalid owned Issue')
        require(not number or number not in owned, 'duplicate owned Issue')
        owned.add(number)
        parse_text(delivery['title'], 'delivery title', 256)
        body = parse_text(delivery['body'], 'work order')
        if not number:
            for section in ('Outcome', 'Scope', 'Acceptance', 'Preserve'):
                require(re.search(r'^## ' + section + r'\s*$', body, re.M), f'work order needs {section}')
            require(re.search(r'^Observer: \S', body, re.M), 'new work order needs an acceptance observer')
        # Owned Issues retain their accepted scenario observers and original text.
        row = orders.order(dict(number=number, html_url='', body=body))
        require(row['slug'] not in keys, 'duplicate Key in batch')
        keys.add(row['slug'])
        require(isinstance(delivery['blocked_by'], list) and all(isinstance(dep, str) and re.fullmatch(r'(?:#[1-9][0-9]*|[A-Za-z0-9]+(?:-[A-Za-z0-9]+)*)', dep) for dep in delivery['blocked_by']), 'invalid native dependencies')
    evidence = answer['evidence']
    require(isinstance(evidence, list), 'invalid evidence')
    seen = set()
    for item in evidence:
        require(isinstance(item, dict) and set(item) == {'goal', 'command', 'output', 'url', 'proved', 'remaining'}, 'invalid evidence fields')
        require(type(item['goal']) is int and 1 <= item['goal'] <= len(goals) and item['goal'] not in seen, 'invalid or duplicate evidence goal')
        seen.add(item['goal'])
        require(type(item['proved']) is bool and isinstance(item['remaining'], str), 'invalid evidence result')
        for name in ('command', 'output', 'url'):
            parse_text(item[name], 'evidence ' + name)
        require(item['url'].startswith('https://'), 'evidence needs a retained run or capture URL')
        require(not item['proved'] or not item['remaining'], 'proved evidence still has an unproved remainder')
        require(item['proved'] or item['remaining'].strip(), 'failed evidence must name what remains unproved')
    if phase == 'verify' and not question:
        require(seen == set(range(1, len(goals) + 1)), 'verification must observe every goal')
    complete = is_complete(phase, answer)
    require(not (complete and deliveries), 'a completed goal cannot schedule more work')
    require(phase == 'plan' or bool(deliveries) or complete or question, 'unproved goals need a bounded successor batch')
    require(not question or not deliveries, 'a product question cannot authorize work')
    require(isinstance(answer['rules'], list) and len(answer['rules']) <= 4, 'invalid rule changes')
    paths = set()
    require(phase != 'verify' or not answer['rules'], 'verification proposes rule work as a specification delivery')
    for rule in answer['rules']:
        require(isinstance(rule, dict) and set(rule) == {'path', 'content', 'cases'}, 'invalid rule proposal')
        require(re.fullmatch(r'scenarios/[A-Za-z0-9_-]+\.md', rule['path']) and rule['path'] not in paths, 'rule changes belong in scenario files')
        paths.add(rule['path'])
        parse_text(rule['content'], 'rule content')
        parse_text(rule['cases'], 'examples and counterexamples')
    return answer


def is_complete(phase, answer):
    return phase == 'verify' and len(answer['evidence']) == len(answer['goals']) and all(item['proved'] for item in answer['evidence']) and not answer['question']


def listing(path):
    return [item for page in d.gh('api', 'repos/{owner}/{repo}/' + path, '--paginate', '--slurp') for item in page]


def read_source(number, previous=None, closed=False):
    # One GraphQL snapshot keeps the text and its editor from different revisions from mixing.
    owner, repo = os.environ['GITHUB_REPOSITORY'].split('/')
    query = 'query($owner:String!,$repo:String!,$number:Int!){repository(owner:$owner,name:$repo){issue(number:$number){number title body url state labels(first:100){totalCount nodes{name}} editor{login} author{login}}}}'
    result = d.gh('api', 'graphql', '-f', 'query=' + query, '-f', 'owner=' + owner, '-f', 'repo=' + repo, '-F', f'number={number}')
    require(not result.get('errors'), 'cannot read human feedback editor')
    node = result['data']['repository']['issue']
    require(node and node['labels']['totalCount'] <= 100, 'feedback Issue snapshot is incomplete')
    issue = dict(number=node['number'], title=node['title'], body=node['body'], html_url=node['url'],
                 state=node['state'].lower(), labels=node['labels']['nodes'])
    actor = node['editor'] or node['author']
    login = actor['login'] if actor else ''
    trusted = bool(login) and not login.endswith('[bot]') and permission(login) in ('admin', 'maintain', 'write')
    return intake({**issue, 'state': 'open'} if closed else issue, trusted, previous)


@functools.cache
def permission(login):
    # One process is one sweep or one run; a collaborator's role does not change within it.
    return d.api(f'collaborators/{login}/permission')['permission']


def read_claim(number):
    return d.claim_record(number, NAMESPACE)


def save_claim(number, old, record):
    require(d.write_claim(number, old, record, NAMESPACE), f'feedback Claim changed for #{number}')


def assert_current(record):
    source = read_source(record['source']['number'], record['source'])
    require(source and source['revision'] == record['source']['revision'], 'human feedback changed; proposal is stale')


def children_state(children):
    states = []
    for number in sorted(children):
        issue = d.api(f'issues/{number}')
        states.append([number, issue['state'], issue.get('state_reason')])
    return states


def next_task(source, record):
    if not source:
        return None
    revision = source['revision']
    if not record or record['source']['revision'] != revision:
        return dict(role='plan', cause=revision, head=revision, base='')
    if record['phase'] == 'replan':
        return dict(role=record['key'][0], cause=revision, head=digest([revision, record['recovery']]), base='')
    if record['phase'] in ('question', 'complete'):
        return None
    if record['phase'] == 'published':
        states = children_state(record['children'])
        if states == record['baseline'] and any(state[1] == 'open' for state in states):
            return None
        return dict(role='verify', cause=revision, head=digest([record['pr'], states, record.get('refresh')]), base='')
    if record['phase'] == 'proposed':
        return None
    role, cause, head, _ = record['key']
    return dict(role=role, cause=cause, head=head, base='')


def under_spend_cap():
    return int(d.run('bash', 'loop/retro.sh', 'spent', timeout=600)) < int(os.environ['LOOP_DAILY_TOKENS'])


def start(number):
    old, record = read_claim(number)
    source = read_source(number, record['source'] if record else None)
    task = next_task(source, record)
    if not task or not d.available(task, record):
        return
    if record and record['key'] == d.key(task) and read_proposal_head(record):
        # Reconciliation publishes the retained answer; a queued job must not spend another model attempt.
        return
    require(under_spend_cap(), 'spend cap reached')
    # Immutable Claim commits retain every accepted human revision, including earlier bodies.
    attempt = d.consumed(record) + 1 if record and record['key'] == d.key(task) else 1
    previous = (record.get('plan') or record.get('previous')) if record and task['role'] == 'verify' else None
    current = dict(key=d.key(task), run=int(os.environ['GITHUB_RUN_ID']), attempt=attempt, phase='claimed', fixes=0,
                   source=source, main=(record['main'] if record and record['key'] == d.key(task) else d.api('git/ref/heads/main')['object']['sha']), previous=previous)
    if record and record.get('recovery'):
        current['recovery'] = record['recovery']
    save_claim(number, old, current)
    assert_current(current)
    old, latest = read_claim(number)
    require(latest == current, 'feedback owner changed')
    current['phase'] = 'issued'
    save_claim(number, old, current)
    context = dict(source=source, phase=task['role'], main=current['main'], previous=previous,
                   run=f"https://github.com/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{current['run']}")
    if current.get('recovery'):
        context['recovery'] = current['recovery']
    d.run('bash', 'loop/runs.sh', 'prompt', 'feedback', data=json.dumps(context))
    with open(os.environ['GITHUB_OUTPUT'], 'a') as out:
        out.write(f"issue={number}\nmain={current['main']}\n")


def read_file(path, ref):
    file = d.gh('api', 'repos/{owner}/{repo}/contents/' + path, '-f', 'ref=' + ref, '--method', 'GET')
    require(file.get('encoding') == 'base64', 'proposal exceeds GitHub content limit')
    return base64.b64decode(file['content']).decode()


def proposal_path(record):
    return f"loop/plans/{record['source']['number']}/{record['source']['revision'][:16]}-{record['key'][0]}-{record['key'][2][:16]}.json"


def proposal_branch(record):
    return f"build/feedback-{record['source']['number']}-{record['source']['revision'][:12]}-{record['key'][0]}-{record['key'][2][:12]}"


def read_proposal_head(record):
    branch = proposal_branch(record)
    refs = d.api('git/matching-refs/heads/' + branch)
    return next((ref['object']['sha'] for ref in refs if ref['ref'] == 'refs/heads/' + branch), None)


def comment(number, marker, body):
    existing = listing(f'issues/{number}/comments?per_page=100')
    if any(c['user']['login'] == 'github-actions[bot]' and c['body'].startswith(marker) for c in existing):
        return
    # A lost write is inspected on the next reconciliation, never immediately repeated.
    d.api(f'issues/{number}/comments', dict(body=marker + '\n' + body))


def commit_proposal(record, answer, main, parents):
    proposal = dict(source=record['source'], phase=record['key'][0], trigger=record['key'][2], main=main,
                    previous=record['previous'], answer=answer)
    files = {proposal_path(record): json.dumps(proposal, indent=2) + '\n'}
    files.update({rule['path']: rule['content'] for rule in answer['rules']})
    base_tree = d.api(f'git/commits/{main}')['tree']['sha']
    tree = d.api('git/trees', dict(base_tree=base_tree, tree=[dict(path=name, mode='100644', type='blob', content=content) for name, content in files.items()]))
    return d.api('git/commits', dict(tree=tree['sha'], parents=list(dict.fromkeys(parents)),
                                   message=f"L90: {proposal['phase']} feedback #{record['source']['number']}\n\nAuthor-Agent: feedback-{os.environ['GITHUB_RUN_ID']}"))['sha']


def claim_for_pr(number):
    for ref in d.api('git/matching-refs/heads/' + NAMESPACE + '/'):
        record = json.loads(d.api('git/commits/' + ref['object']['sha'])['message'])
        if record.get('pr') == number:
            return ref['object']['sha'], record
    raise ValueError('planning PR has no feedback Claim')


def repair(number, head):
    task = d.due(number)
    if not task or task['role'] != 'fix' or not task.get('planning') or (head and task['head'] != head):
        return
    if not d.acquire(task):
        return
    fresh = d.due(number)
    if not fresh or d.key(fresh) != d.key(task):
        return
    _, record = claim_for_pr(number)
    assert_current(record)
    main = d.api('git/ref/heads/main')['object']['sha']
    context = dict(source=record['source'], phase=record['key'][0], main=main, previous=record['previous'],
                   repair=dict(pr=number, head=task['head'], cause=task['cause'], candidate=read_file(record['path'], task['head'])),
                   run=f"https://github.com/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{os.environ['GITHUB_RUN_ID']}")
    d.run('bash', 'loop/runs.sh', 'prompt', 'feedback', data=json.dumps(context))
    with open(os.environ['GITHUB_OUTPUT'], 'a') as out:
        out.write(f"issue={record['source']['number']}\nmain={main}\npr={number}\nhead={task['head']}\nbranch={task['branch']}\n")

    d.run('gh', 'pr', 'ready', str(number), '--undo')
    d.issued(number)


def repair_propose(number, head, main, answer):
    _, owner = d.claim_record(number)
    require(owner and owner['run'] == int(os.environ['GITHUB_RUN_ID']) and owner['key'][0] == 'fix' and owner['key'][2] == head,
            'not the planning repair owner')
    require(re.fullmatch('[0-9a-f]{40}', main), 'invalid observed main')
    pr = d.api(f'pulls/{number}')
    require(pr['state'] == 'open' and pr['head']['sha'] == head, 'stale planning repair')
    old, record = claim_for_pr(number)
    assert_current(record)
    answer = parse_answer(answer, record['key'][0], record['previous'])
    if answer['question']:
        d.run('gh', 'pr', 'ready', str(number), '--undo')
        d.gh('api', '--method', 'PATCH', f'repos/{{owner}}/{{repo}}/pulls/{number}', '--input', '-',
             data=json.dumps(dict(body=pr['body'] + '\nStopped: ' + answer['question'])))
        comment(record['source']['number'], f'<!-- feedback-question {number} -->', 'Stopped: ' + answer['question'])
        save_claim(record['source']['number'], old, {**record, 'phase': 'question'})
        return
    sha = commit_proposal(record, answer, main, [head, main])
    assert_current(record)
    # Updating the controller context first makes the old proposal fail closed during publication.
    save_claim(record['source']['number'], old, {**record, 'main': main})
    d.gh('api', '--method', 'PATCH', 'repos/{owner}/{repo}/git/refs/heads/' + pr['head']['ref'], '--input', '-',
         data=json.dumps(dict(sha=sha, force=False)))
    # A lost ref response is recovered by the shared Claim/repair controller; never force an old head.
    if pr['draft']:
        d.run('gh', 'pr', 'ready', str(number))
    ensure_checks(d.api(f'pulls/{number}'))


def propose(number, answer):
    old, record = read_claim(number)
    require(record and record['run'] == int(os.environ['GITHUB_RUN_ID']) and record['phase'] == 'issued', 'not the feedback owner')
    assert_current(record)
    phase = record['key'][0]
    answer = parse_answer(answer, phase, record['previous'])
    if answer['question']:
        comment(number, f"<!-- feedback-question {record['source']['revision']} -->",
                'Stopped: ' + answer['question'] + '\nEdit the source Issue with the decision to resume planning.')
        save_claim(number, old, {**record, 'phase': 'question'})
        return
    branch = proposal_branch(record)
    if not read_proposal_head(record):
        sha = commit_proposal(record, answer, record['main'], [record['main']])
        d.api('git/refs', dict(ref='refs/heads/' + branch, sha=sha))
    publish_proposal(number, old, record)


def publish_proposal(number, old, record):
    branch = proposal_branch(record)
    assert_current(record)
    prs = d.gh('pr', 'list', '--head', branch, '--state', 'all', '--json', 'number,headRefOid', '--limit', '100')
    if prs:
        require(len(prs) == 1, 'multiple planning PRs for one Claim')
        pr = prs[0]['number']
    else:
        assert_current(record)
        pr = d.api('pulls', dict(title=f'L90: {record["key"][0]} feedback #{number}', head=branch, base='main',
                                body=f'Scenarios: L90\nRefs #{number}\n\nSource revision: {record["source"]["revision"]}\n\nIndependent review authorizes publication of the proposed goal and native work Issues.'))['number']
    save_claim(number, old, {**record, 'phase': 'proposed', 'pr': pr, 'path': proposal_path(record)})
    ensure_checks(d.api(f'pulls/{pr}'))
    comment(number, f'<!-- feedback-proposal {pr} -->', f'Proposed {record["key"][0]}: #{pr}. Human source revision `{record["source"]["revision"]}` is retained in the proposal and Claim history.')
    return pr


def resume_publication(number, old, record):
    if record['phase'] not in ('issued', 'publishing') or d.api(f"actions/runs/{record['run']}")['status'] != 'completed':
        return None
    head = read_proposal_head(record)
    if not head:
        return None
    parse_proposal(json.loads(read_file(proposal_path(record), head)), record)
    # The shared atomic Claim excludes a queued planner while the controller resumes writes.
    current = {**record, 'phase': 'publishing', 'run': int(os.environ['GITHUB_RUN_ID'])}
    save_claim(number, old, current)
    owned, latest = read_claim(number)
    require(latest == current, 'feedback publication owner changed')
    return publish_proposal(number, owned, current)


def ensure_checks(pr):
    # A GITHUB_TOKEN write starts no pull_request run, or one GitHub holds for a person's approval. The controller
    # dispatches its own and deletes the held ones, so no planning PR asks for a click. Missed dispatches are recoverable.
    for workflow in ('check.yml', 'loop.yml'):
        runs = d.gh('run', 'list', '--workflow', workflow, '--branch', pr['head']['ref'], '--limit', '100', '--json', 'databaseId,headSha,status,conclusion')
        held = [run for run in runs if run['conclusion'] == 'action_required']
        if not any(run['headSha'] == pr['head']['sha'] for run in runs if run not in held):
            args = ('-f', f'pr={pr["number"]}') if workflow == 'loop.yml' else ()
            d.run('gh', 'workflow', 'run', workflow, '--ref', pr['head']['ref'], *args)
        for run in held:
            d.run('gh', 'run', 'delete', str(run['databaseId']))
    if not pr.get('auto_merge'):
        d.run('gh', 'pr', 'merge', str(pr['number']), '--auto', '--merge')


def parse_proposal(proposal, record):
    require(set(proposal) == {'source', 'phase', 'trigger', 'main', 'previous', 'answer'}, 'invalid proposal envelope')
    require(proposal['phase'] in ('plan', 'verify'), 'invalid proposal phase')
    require(re.fullmatch(r'[0-9a-f]{40}', proposal['main']), 'invalid observed main')
    answer = parse_answer(proposal['answer'], proposal['phase'], proposal['previous'])
    require(not answer['question'], 'product questions stay on the source Issue')
    require(proposal['source'] == record['source'] and proposal['main'] == record['main'] and
            proposal['phase'] == record['key'][0] and proposal['trigger'] == record['key'][2] and
            proposal['previous'] == record['previous'], 'proposal changed controller-owned context')
    return proposal


def read_proposal(pr):
    paths = [item['filename'] for item in listing(f'pulls/{pr["number"]}/files?per_page=100')]
    plans = [path for path in paths if path.startswith('loop/plans/')]
    require(len(plans) == 1, 'planning PR needs exactly one proposal')
    proposal = json.loads(read_file(plans[0], pr['head']['sha']))
    number = proposal['source']['number']
    _, record = read_claim(number)
    require(record and record.get('pr') == pr['number'] and record['path'] == plans[0], 'proposal has no matching controller Claim')
    proposal = parse_proposal(proposal, record)
    answer = proposal['answer']
    require(set(paths) == {plans[0]} | {rule['path'] for rule in answer['rules']}, 'planning PR contains undeclared changes')
    for rule in answer['rules']:
        require(read_file(rule['path'], pr['head']['sha']) == rule['content'], 'rule content differs from proposal')
    assert_current(record)
    # GitHub closing keywords would skip goal verification at merge, before publication runs.
    messages = [pr.get('title') or '', pr.get('body') or '']
    messages += [commit['commit']['message'] for commit in listing(f'pulls/{pr["number"]}/commits?per_page=100')]
    require(not any(re.search(r'(?i)\b(?:close[sd]?|fix(?:es|ed)?|resolve[sd]?)\s+(?:[\w.-]+/[\w.-]+)?#\d+|\b(?:close[sd]?|fix(?:es|ed)?|resolve[sd]?)\s+https://github.com/', message) for message in messages), 'planning PR must not auto-close Issues')
    return proposal, record


def gate(number):
    pr = d.api(f'pulls/{number}')
    proposal, _ = read_proposal(pr)
    if proposal['phase'] == 'verify':
        require(is_observation_current(proposal['main']), 'main behavior changed since goal observation')
    if proposal['answer']['deliveries']:
        read_deliveries(proposal)
    return proposal


def is_observation_current(main):
    # Planning commits do not change the behavior observed; all other main changes invalidate it.
    current = d.api('git/ref/heads/main')['object']['sha']
    compared = d.api(f"compare/{main}...{current}")
    return compared['status'] in ('identical', 'ahead') and len(compared['files']) < 300 and all(item['filename'].startswith('loop/plans/') for item in compared['files'])


def refresh_verification(number, old, record):
    if record['key'][0] != 'verify' or is_observation_current(record['main']):
        return False
    pr = d.api(f'pulls/{record["pr"]}')
    if pr['state'] == 'open':
        d.gh('api', '--method', 'PATCH', f'repos/{{owner}}/{{repo}}/pulls/{pr["number"]}', '--input', '-', data=json.dumps(dict(state='closed')))
    comment(number, f'<!-- feedback-stale {pr["number"]} -->', f"Main behavior changed after observation in #{pr['number']}; fresh verification is required.")
    previous = record['previous']
    save_claim(number, old, {**record, 'phase': 'published', 'plan': previous, 'children': previous['children'],
                             'baseline': [], 'refresh': d.api('git/ref/heads/main')['object']['sha']})
    return True


def dependencies(number):
    return listing(f'issues/{number}/dependencies/blocked_by?per_page=100')


def prepare_deliveries(proposal, catalog, busy):
    """Reuse native owners, and refuse an update that would displace a live delivery."""
    prepared = []
    for delivery in proposal['answer']['deliveries']:
        row = orders.order(dict(number=delivery['issue'], html_url='', body=delivery['body']))
        same_key = [issue for issue in catalog if re.search(r'^Key: ' + re.escape(row['slug']) + r'\s*$', issue.get('body') or '', re.M)]
        require(len(same_key) <= 1, 'duplicate existing work Key')
        existing = next((issue for issue in catalog if issue['number'] == delivery['issue']), None) if delivery['issue'] else next(iter(same_key), None)
        require(not delivery['issue'] or existing, 'named work Issue is missing')
        active = False
        if existing:
            require(existing['number'] != proposal['source']['number'] or proposal['source']['kind'] == 'work', 'feedback cannot authorize implementation of its own parent')
            require(not same_key or same_key[0]['number'] == existing['number'], 'Key belongs to another Issue')
            labels = {label['name'] for label in existing['labels']}
            require('loop:work' in labels, 'owned delivery must be a work Issue')
            require(orders.field(existing['body'], 'Key') == row['slug'], 'an owned Key cannot change')
            active = row['slug'] in busy or 'ready-for-agent' in labels or existing['state'] == 'closed'
            if active:
                require(existing['title'] == delivery['title'] and existing['body'].split('\n<!-- feedback-delivery ')[0].rstrip() == delivery['body'].split('\n<!-- feedback-delivery ')[0].rstrip(), 'live or completed owner must be reused unchanged')
                current = {f"#{dep['number']}" for dep in dependencies(existing['number'])}
                require(current == set(delivery['blocked_by']), 'live owner dependencies must be preserved')
        prepared.append(dict(delivery=delivery, row=row, existing=existing, active=active))
    names = {item['row']['slug'] for item in prepared}
    aliases = {f"#{item['existing']['number']}": item['row']['slug'] for item in prepared if item['existing']}
    graph = {}
    for item in prepared:
        key = item['row']['slug']
        graph[key] = [aliases.get(dep, dep) for dep in item['delivery']['blocked_by']]
        require(all(dep.startswith('#') or dep in names for dep in graph[key]), 'missing batch dependency')
        require(key not in graph[key], 'self dependency')
    def visit(name, path):
        require(name not in path, 'dependency cycle in proposed batch')
        for dep in graph.get(name, []):
            if not dep.startswith('#'):
                visit(dep, path | {name})
    for name in graph:
        visit(name, set())
    return prepared


def read_deliveries(proposal):
    catalog = [issue for issue in listing('issues?state=all&labels=loop%3Awork&per_page=100') if 'pull_request' not in issue]
    rows = orders.read_orders()
    busy = d.active_builds()
    busy |= {row['slug'] for row in rows if row['state'] == 'in-flight'}
    owners = d.api('git/matching-refs/heads/loop-row/')
    busy |= {owner['ref'].removeprefix('refs/heads/loop-row/') for owner in owners}
    prepared = prepare_deliveries(proposal, catalog, busy)
    if proposal['phase'] == 'verify':
        require(any(not item['existing'] or item['existing']['state'] == 'open' for item in prepared), 'unproved goals need open successor work, not only closed owners')
    sources = {int(ref['ref'].rsplit('/', 1)[1]) for ref in d.api('git/matching-refs/heads/' + NAMESPACE + '/')}
    for item in prepared:
        existing = item['existing']
        if existing and existing['number'] in sources:
            require(existing['body'] == item['delivery']['body'] and existing['title'] == item['delivery']['title'], 'preserve owned source feedback; propose separate work')
    return prepared


def parent_issue(number):
    try:
        return d.api(f'issues/{number}/parent')
    except subprocess.CalledProcessError as error:
        if 'HTTP 404' in (error.stderr or ''):
            return None
        raise


def publish_deliveries(proposal, record):
    number = proposal['source']['number']
    prepared = read_deliveries(proposal)
    by_key = {}
    for item in prepared:
        assert_current(record)
        delivery, existing = item['delivery'], item['existing']
        marker = f"<!-- feedback-delivery {number} {item['row']['slug']} -->"
        body = delivery['body']
        if not existing or marker in existing['body']:
            body = body.rstrip() + '\n' + marker + f'\nParent goal: #{number}\nPlan: #{record["pr"]}\n'
        if not existing:
            existing = d.api('issues', dict(title=delivery['title'], body=body, labels=['loop:work']))
            item['existing'] = existing
        else:
            labels = {label['name'] for label in existing['labels']}
            if existing['state'] == 'open' and 'ready-for-agent' not in labels and not item['active']:
                # The original feedback Issue is never rewritten, including a work Issue being triaged.
                require(existing['number'] != number or existing['body'] == delivery['body'], 'source work Issue needs a separate reviewed specification; preserve its text')
                if existing['body'] != body:
                    d.gh('api', '--method', 'PATCH', f'repos/{{owner}}/{{repo}}/issues/{existing["number"]}', '--input', '-', data=json.dumps(dict(title=delivery['title'], body=body)))
        by_key[item['row']['slug']] = existing
    # Create every dependency before adding any intake label. A partial publication stays unready.
    for item in prepared:
        assert_current(record)
        child = item['existing']
        known = {dep['number']: dep for dep in dependencies(child['number'])}
        declared = {int(dep[1:]) if dep.startswith('#') else by_key[dep]['number'] for dep in item['delivery']['blocked_by']}
        for removed in known.keys() - declared:
            require(not item['active'], 'cannot remove a live owner dependency')
            d.run('gh', 'api', '--method', 'DELETE', f'repos/{{owner}}/{{repo}}/issues/{child["number"]}/dependencies/blocked_by/{known[removed]["id"]}')
        for dep in item['delivery']['blocked_by']:
            target = d.api(f'issues/{int(dep[1:])}') if dep.startswith('#') else by_key[dep]
            require(target['number'] != child['number'], 'self dependency')
            if target['number'] not in known:
                d.api(f'issues/{child["number"]}/dependencies/blocked_by', dict(issue_id=target['id']))
        if child['number'] != number:
            children = listing(f'issues/{number}/sub_issues?per_page=100')
            if child['number'] not in {c['number'] for c in children}:
                parent = parent_issue(child['number'])
                if not parent:
                    d.api(f'issues/{number}/sub_issues', dict(sub_issue_id=child['id']))
                elif parent['number'] != number and child['number'] not in {dep['number'] for dep in dependencies(number)}:
                    # GitHub permits only one native parent; reuse its owner without reparenting it.
                    d.api(f'issues/{number}/dependencies/blocked_by', dict(issue_id=child['id']))
    for item in prepared:
        assert_current(record)
        child = item['existing']
        if child['state'] == 'open' and 'ready-for-agent' not in {label['name'] for label in child['labels']}:
            d.api(f'issues/{child["number"]}/labels', dict(labels=['ready-for-agent']))
    return sorted({item['existing']['number'] for item in prepared})


def publish(number):
    old, record = read_claim(number)
    if not record or record['phase'] != 'proposed':
        return
    assert_current(record)
    pr = d.api(f'pulls/{record["pr"]}')
    if not pr.get('merged'):
        require(pr['state'] == 'open', f'planning PR #{pr["number"]} closed without delivery; engineering repair needed')
        ensure_checks(pr)
        return
    require(pr['base']['ref'] == 'main' and pr['head']['repo']['full_name'] == os.environ['GITHUB_REPOSITORY'], 'planning PR must merge into this repository main')
    # A verdict comment updates the PR, so an unchanged refused PR is not gated again.
    waiting = record.get('waiting') or {}
    if (waiting.get('pr'), waiting.get('updated')) != (pr['number'], pr['updated_at']):
        waiting = {}
        # Reuse all L46 checks and independent authorship, with merged instead of open state.
        try:
            d.run('bash', 'loop/rules.sh', 'publication-ready', str(pr['number']), timeout=300)
        except subprocess.CalledProcessError as error:
            if error.returncode != 1:
                raise
            reason = redact(' '.join((error.stderr or 'publication-ready refused').split()), os.environ)[:300]
            waiting = dict(pr=pr['number'], updated=pr['updated_at'], reason=reason)
            save_claim(number, old, {**record, 'waiting': waiting})
    if waiting:
        print(f'feedback #{number}: merged plan #{pr["number"]} waits: {waiting["reason"]}')
        return
    proposal = gate(pr['number'])
    answer = proposal['answer']
    complete = is_complete(proposal['phase'], answer)
    if complete:
        assert_current(record)
        comment(number, f'<!-- feedback-verified {pr["number"]} -->', f"Every original goal has independently reviewed evidence in #{pr['number']}, observed on `{proposal['main']}` for human revision `{proposal['source']['revision']}`.")
        assert_current(record)
        d.gh('api', '--method', 'PATCH', f'repos/{{owner}}/{{repo}}/issues/{number}', '--input', '-', data=json.dumps(dict(state='closed', state_reason='completed')))
        fresh = read_source(number, record['source'], closed=True)
        if not fresh or fresh['revision'] != record['source']['revision']:
            d.gh('api', '--method', 'PATCH', f'repos/{{owner}}/{{repo}}/issues/{number}', '--input', '-', data=json.dumps(dict(state='open')))
            raise ValueError('feedback changed during closure; source reopened')
        save_claim(number, old, {**record, 'phase': 'complete'})
    else:
        children = publish_deliveries(proposal, record)
        message = (f"Reviewed plan #{pr['number']} published: " + ', '.join(f'#{child}' for child in children) + '. Child delivery will trigger fresh observation of the original goal.'
                   if children else f"Reviewed goal in #{pr['number']} is ready for independent observation.")
        comment(number, f'<!-- feedback-published {pr["number"]} -->', message)
        # Prior proposals remain linked rather than recursively copied into every successor.
        plan = dict(pr=pr['number'], path=record['path'], children=children,
                    answer=dict(goals=answer['goals']), evidence=answer['evidence'])
        save_claim(number, old, {**record, 'phase': 'published', 'children': children,
                                 'baseline': children_state(children), 'plan': plan})
    d.run('gh', 'workflow', 'run', 'reconcile.yml')


def recover_unreviewed_plan(number, old, record):
    """A bypassed merge never authorizes delivery; a fresh proposal must earn review."""
    pr = d.api(f'pulls/{record["pr"]}')
    if not pr.get('merged'):
        return False
    verdicts = [json.loads(line) for line in d.run('bash', 'loop/rules.sh', 'verdicts', str(pr['number'])).splitlines()]
    exact = [verdict for verdict in verdicts if verdict['head'] == pr['head']['sha']]
    if exact and exact[-1]['verdict'] == 'approve':
        return False
    assert_current(record)
    recovery = dict(pr=pr['number'], head=pr['head']['sha'], reason='Merged plan lacks independent exact-head approval.')
    save_claim(number, old, {**record, 'phase': 'replan', 'recovery': recovery})
    print(f'Feedback #{number}: PR #{pr["number"]} merged without approval; withheld delivery and scheduled a fresh reviewed plan.')
    return True


def publication(command, number):
    """Translate the isolated publisher's process result, including an unknown timeout."""
    try:
        return 'ok', d.run(sys.executable, 'loop/feedback.py', command, str(number), timeout=180)
    except subprocess.CalledProcessError as error:
        return 'failed', redact(error.stderr or str(error), os.environ)[:4000]
    except subprocess.TimeoutExpired:
        return 'unknown', 'Publication timed out; writes may have applied. The next sweep inspects retained state before retrying.'


def reconcile():
    # One sweep is serialized by reconcile.yml. Native Issues, never proposal files, own the queue.
    event = os.environ.get('GITHUB_EVENT_NAME', '')
    if event in UNSWEPT_EVENTS:
        print(f'feedback: {event} changes no feedback Issue; the next Issue event, main push, dispatch or five-minute sweep reconciles', file=sys.stderr)
        return
    issues = [item for item in listing('issues?state=open&per_page=100') if 'pull_request' not in item]
    refs = d.api('git/matching-refs/heads/' + NAMESPACE + '/')
    claimed = {int(ref['ref'].rsplit('/', 1)[1]) for ref in refs}
    known = {issue['number'] for issue in issues}
    for ref in refs:
        number = int(ref['ref'].rsplit('/', 1)[1])
        if number not in known:
            issues.append(d.api(f'issues/{number}'))
    runs = d.gh('run', 'list', '--workflow', 'feedback.yml', '--limit', '100', '--json', 'status')
    queued = sum(run['status'] != 'completed' for run in runs)
    within_budget = None
    failures = []
    for issue in issues:
        number = issue['number']
        labels = {label['name'] for label in issue['labels']}
        if labels & {'loop:status', 'loop:report-only'}:
            continue
        if number not in claimed and ('loop:work' in labels and 'ready-for-agent' in labels or issue['user']['type'] == 'Bot' and 'loop:work' not in labels):
            continue
        try:
            within_budget, queued = sweep(issue, number in claimed, within_budget, queued)
        except (ValueError, subprocess.CalledProcessError, KeyError, TypeError) as error:
            # One source's failure leaves the others swept; timeouts and OS errors stop the sweep.
            print(f'feedback #{number}: {describe(error)}', file=sys.stderr)
            failures.append(error)
    if failures:
        raise failures[0]
    print(f'Feedback sweep complete; {queued} runs running or queued.')


def sweep(issue, claimed, within_budget, queued):
    """One source Issue's reconciliation; returns the sweep's spend decision and queued planner count."""
    number = issue['number']
    old, record = read_claim(number) if claimed else (None, None)
    source = read_source(number, record['source'] if record else None)
    if not source:
        return within_budget, queued
    if issue['state'] == 'closed':
        d.gh('api', '--method', 'PATCH', f'repos/{{owner}}/{{repo}}/issues/{number}', '--input', '-', data=json.dumps(dict(state='open')))
    if record and record['source']['revision'] == source['revision'] and record['phase'] in ('issued', 'publishing'):
        status, detail = publication('resume', number)
        require(status == 'ok', f'Feedback #{number}: resumed publication {status}: {detail}')
        if detail:
            print(f'Feedback #{number}: resumed proposal PR #{detail}.')
            return within_budget, queued
    if record and record['source']['revision'] == source['revision'] and record['phase'] == 'proposed':
        if recover_unreviewed_plan(number, old, record):
            old, record = read_claim(number)
        elif not refresh_verification(number, old, record):
            status, detail = publication('publish', number)
            require(status == 'ok', f'Feedback #{number}, PR #{record["pr"]}: publication blocked ({status}): {detail}')
            print(detail or f'Feedback #{number}: publication checked for PR #{record["pr"]}.')
            return within_budget, queued
        _, record = read_claim(number)
    task = next_task(source, record)
    if task and d.available(task, record) and queued < 4:
        if within_budget is None:
            within_budget = under_spend_cap()
        if not within_budget:
            print('feedback: spend cap reached', file=sys.stderr)
            return within_budget, queued
        if record and record['phase'] == 'proposed' and record['source']['revision'] != source['revision']:
            pr = d.api(f'pulls/{record["pr"]}')
            if pr['state'] == 'open':
                d.gh('api', '--method', 'PATCH', f'repos/{{owner}}/{{repo}}/pulls/{pr["number"]}', '--input', '-', data=json.dumps(dict(state='closed')))
        queued += 1
        d.run('gh', 'workflow', 'run', 'feedback.yml', '-f', f'issue={number}')
        print(f'Feedback #{number}: queued {task["role"]}; next: feedback run.')
    elif task:
        print(f'feedback #{number}: claimed or retry limit reached', file=sys.stderr)
    return within_budget, queued


def activity():
    rows = []
    for ref in d.api('git/matching-refs/heads/' + NAMESPACE + '/'):
        record = json.loads(d.api('git/commits/' + ref['object']['sha'])['message'])
        if record['phase'] == 'proposed' and record.get('waiting', {}).get('pr') == record['pr']:
            rows.append(f"[Feedback #{record['source']['number']}]({record['source']['html_url']}): merged plan #{record['pr']} waits: {record['waiting']['reason']}")
        if record['phase'] not in ('claimed', 'issued'):
            continue
        owner = d.api(f"actions/runs/{record['run']}")
        if owner['status'] == 'completed' and d.consumed(record) >= 2:
            number = record['source']['number']
            rows.append(f"[Feedback #{number}]({record['source']['html_url']}): needs investigation after two attempts; [last run]({owner['html_url']})")
    return rows


def parse_number(value):
    if not re.fullmatch('[1-9][0-9]*', value):
        raise argparse.ArgumentTypeError('expected a positive Issue or PR number')
    return int(value)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    for name in ('reconcile', 'start', 'repair', 'repair-propose', 'propose', 'publish', 'resume', 'gate'):
        command = commands.add_parser(name)
        if name != 'reconcile':
            command.add_argument('number', type=parse_number)
        if name == 'repair':
            command.add_argument('head', nargs='?', default='')
        if name == 'repair-propose':
            command.add_argument('head')
            command.add_argument('main')
    args = parser.parse_args()
    for name in ('head', 'main'):
        value = getattr(args, name, '')
        if value and not re.fullmatch('[0-9a-f]{40}', value):
            parser.error(f'{name} must be a full commit SHA')
    try:
        if args.command == 'reconcile':
            reconcile()
        elif args.command == 'start':
            start(args.number)
        elif args.command == 'repair':
            repair(args.number, args.head)
        elif args.command == 'repair-propose':
            repair_propose(args.number, args.head, args.main, json.load(sys.stdin))
        elif args.command == 'propose':
            propose(args.number, json.load(sys.stdin))
        elif args.command == 'resume':
            old, record = read_claim(args.number)
            result = resume_publication(args.number, old, record) if record else None
            if result:
                print(result)
        elif args.command == 'publish':
            publish(args.number)
        elif args.command == 'gate':
            gate(args.number)
    except ValueError as error:
        print(f'feedback: {error}', file=sys.stderr)
        sys.exit(1)
    except (subprocess.SubprocessError, KeyError, TypeError, OSError) as error:
        print(f'feedback: {describe(error)}', file=sys.stderr)
        sys.exit(4)
