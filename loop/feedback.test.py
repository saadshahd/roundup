#!/usr/bin/env python3
"""L90 runtime observers at the feedback controller's GitHub seam."""
import sys
sys.dont_write_bytecode = True
import unittest
import copy
import base64
import json
import os
import tempfile
import subprocess
from pathlib import Path
from unittest.mock import patch
import feedback as f


def source(**changes):
    return dict(dict(number=355, title='Keep feedback moving', body='Observe the requested result.',
                     state='open', labels=[], user=dict(login='owner', type='User'),
                     editor='owner', html_url='https://github.com/o/r/issues/355'), **changes)


def answer(issue=0):
    return dict(goals=[dict(statement='Feedback reaches observed results', observer='Run the complete feedback cycle')],
                deliveries=[dict(issue=issue, title='Implement the observer', body='## Outcome\nObserve results\n## Scope\nloop/\n## Acceptance\nScenarios: L90\nSpecification: [scenarios/loop-rules.md](https://github.com/o/r/blob/main/scenarios/loop-rules.md)\nObserver: Execute a feedback cycle\n## Preserve\nL88\nKey: observe-feedback\nPriority: 0\nMode: implement\nProvider: any\n', blocked_by=[])],
                evidence=[], rules=[], question='')


def verification(proved):
    result = answer()
    if proved:
        result['deliveries'] = []
    result['evidence'] = [dict(goal=1, command='python3 loop/feedback.test.py', output='Observed cycle',
                               url='https://github.com/o/r/actions/runs/1', proved=proved,
                               remaining='' if proved else 'Source feedback remains unproved')]
    return result


class GitHub:
    """External GitHub protocol fake: native Issues, refs, PRs and workflow observations."""
    def __init__(self):
        self.issues = {355: {**source(), 'id': 355, 'state_reason': None}}
        self.refs = {'refs/heads/main': 'a' * 40}
        self.commits = {'a' * 40: dict(tree=dict(sha='b' * 40))}
        self.trees = {'b' * 40: {}}
        self.prs = {}
        self.comments = {}
        self.blocked = {}
        self.children = {}
        self.events = {}
        self.runs = {}
        self.writes = []
        self.counter = 1000
        self.editor = 'owner'
        self.permission = 'write'
        self.main_changes = []
        self.competition = False
        self.lost_issue_response = False
        self.pull_error = None
        self.lost_pull_response = False

    def sha(self):
        self.counter += 1
        return format(self.counter, '040x')

    def api(self, path, payload=None):
        if payload is not None:
            self.writes.append((path, copy.deepcopy(payload)))
        if path.startswith('git/matching-refs/'):
            prefix = 'refs/' + path.removeprefix('git/matching-refs/')
            return [dict(ref=ref, object=dict(sha=sha)) for ref, sha in self.refs.items() if ref.startswith(prefix)]
        if path == 'git/ref/heads/main':
            return dict(object=dict(sha=self.refs['refs/heads/main']))
        if path == 'git/commits':
            sha = self.sha()
            self.commits[sha] = {**payload, 'tree': dict(sha=payload['tree'])}
            return dict(sha=sha)
        if path.startswith('git/commits/'):
            return self.commits[path.rsplit('/', 1)[1]]
        if path == 'git/trees':
            sha = self.sha()
            self.trees[sha] = {**self.trees[payload['base_tree']], **{item['path']: item['content'] for item in payload['tree']}}
            return dict(sha=sha)
        if path == 'git/refs':
            if payload['ref'] in self.refs or self.competition:
                raise subprocess.CalledProcessError(1, ['gh'])
            self.refs[payload['ref']] = payload['sha']
            return payload
        if path.startswith('git/refs/'):
            ref = path.removeprefix('git/')
            if self.refs[ref] not in self.commits[payload['sha']]['parents']:
                raise subprocess.CalledProcessError(1, ['gh'])
            self.refs[ref] = payload['sha']
            for pr in self.prs.values():
                if ref == 'refs/heads/' + pr['head']['ref']:
                    pr['head']['sha'] = payload['sha']
            return payload
        if path.startswith('collaborators/'):
            return dict(permission=self.permission)
        if path.startswith('actions/runs/'):
            number = int(path.split('/')[2])
            if path.endswith('/jobs?per_page=100'):
                return dict(jobs=[dict(steps=[dict(name='Run anthropics/claude-code-action@v1', status='completed', conclusion='success')])])
            return self.runs.get(number, dict(status='completed', conclusion='success'))
        if path.startswith('commits/') and '/check-runs' in path:
            return dict(check_runs=[])
        if path.startswith('compare/'):
            return dict(status='ahead', files=self.main_changes)
        if path.startswith('issues?'):
            items = list(self.issues.values())
            if 'labels=' in path:
                items = [i for i in items if any(l['name'] == 'loop:work' for l in i['labels'])]
            if 'state=open' in path:
                items = [i for i in items if i['state'] == 'open']
            return items
        if path == 'issues':
            number = max(self.issues) + 1
            issue = dict(number=number, id=number, state='open', state_reason=None, html_url=f'https://github.com/o/r/issues/{number}', user=dict(login='github-actions[bot]', type='Bot'),
                         title=payload['title'], body=payload['body'], labels=[dict(name=l) for l in payload['labels']])
            self.issues[number] = issue
            if self.lost_issue_response:
                self.lost_issue_response = False
                raise subprocess.TimeoutExpired('gh', 120)
            return copy.deepcopy(issue)
        if path.startswith('issues/'):
            parts = path.split('/')
            number = int(parts[1])
            suffix = '/'.join(parts[2:]).split('?')[0]
            if not suffix:
                if payload:
                    self.issues[number].update(payload)
                return copy.deepcopy(self.issues[number])
            if suffix == 'parent':
                for parent, children in self.children.items():
                    if number in children:
                        return self.issues[parent]
                raise subprocess.CalledProcessError(1, ['gh'], stderr='gh: Not Found (HTTP 404)')
            if suffix == 'comments':
                if payload:
                    self.comments.setdefault(number, []).append(dict(body=payload['body'], user=dict(login='github-actions[bot]')))
                    return {}
                return self.comments.get(number, [])
            if suffix == 'dependencies/blocked_by':
                if payload:
                    self.blocked.setdefault(number, []).append(payload['issue_id'])
                    return {}
                return [copy.deepcopy(self.issues[n]) for n in self.blocked.get(number, [])]
            if suffix == 'sub_issues':
                if payload:
                    self.children.setdefault(number, []).append(payload['sub_issue_id'])
                    return {}
                return [self.issues[n] for n in self.children.get(number, [])]
            if suffix == 'labels':
                self.issues[number]['labels'] += [dict(name=l) for l in payload['labels']]
                return self.issues[number]['labels']
        if path == 'pulls':
            if self.pull_error:
                raise self.pull_error
            number = max(self.prs, default=0) + 1
            sha = self.refs['refs/heads/' + payload['head']]
            self.prs[number] = dict(number=number, title=payload['title'], body=payload['body'], state='open', merged=False, mergeable=True, draft=False,
                                    head=dict(sha=sha, ref=payload['head'], repo=dict(full_name='o/r')),
                                    base=dict(ref='main', sha=self.refs['refs/heads/main'], repo=dict(full_name='o/r')))
            if self.lost_pull_response:
                self.lost_pull_response = False
                raise subprocess.TimeoutExpired('gh', 120)
            return self.prs[number]
        if path.startswith('pulls/'):
            parts = path.split('/')
            pr = self.prs[int(parts[1])]
            if len(parts) == 2:
                if payload:
                    pr.update(payload)
                return copy.deepcopy(pr)
            if parts[2].startswith('files'):
                tree = self.trees[self.commits[pr['head']['sha']]['tree']['sha']]
                base = self.trees[self.commits[pr['base']['sha']]['tree']['sha']]
                return [dict(filename=name) for name, value in tree.items() if base.get(name) != value]
            if parts[2].startswith('commits'):
                return [dict(commit=dict(message=self.commits[pr['head']['sha']]['message']))]
        raise AssertionError((path, payload))

    def gh(self, *args, data=None):
        if args[:2] == ('api', 'graphql'):
            number = int(next(arg.removeprefix('number=') for arg in args if arg.startswith('number=')))
            issue = self.issues[number]
            node = dict(number=number, title=issue['title'], body=issue['body'], url=issue['html_url'], state=issue['state'].upper(),
                        labels=dict(totalCount=len(issue['labels']), nodes=issue['labels']), editor=dict(login=self.editor), author=dict(login=issue['user']['login']))
            return dict(data=dict(repository=dict(issue=node)))
        if args[:2] == ('run', 'list'):
            return []
        if args[:2] == ('pr', 'list'):
            branch = args[args.index('--head')+1] if '--head' in args else None
            return [dict(number=p['number'], headRefOid=p['head']['sha'], headRefName=p['head']['ref'], body=p['body'], isCrossRepository=False)
                    for p in self.prs.values() if (not branch or p['head']['ref'] == branch) and ('all' in args or p['state'] == 'open')]
        path = next(arg.split('repos/{owner}/{repo}/', 1)[1] for arg in args if arg.startswith('repos/{owner}/{repo}/'))
        if path.startswith('contents/'):
            ref = next(arg.removeprefix('ref=') for arg in args if arg.startswith('ref='))
            content = self.trees[self.commits[ref]['tree']['sha']][path.removeprefix('contents/')]
            return dict(encoding='base64', content=base64.b64encode(content.encode()).decode())
        result = self.api(path, json.loads(data) if data else None)
        return [result] if '--slurp' in args else result

    def run(self, *args, data=None, **kwargs):
        if args[:2] == (sys.executable, 'loop/feedback.py') and args[2] in ('publish', 'resume'):
            try:
                number = int(args[3])
                if args[2] == 'resume':
                    old, record = f.read_claim(number)
                    result = f.resume_publication(number, old, record)
                    return str(result) if result else ''
                f.publish(number)
            except (ValueError, subprocess.CalledProcessError) as error:
                raise subprocess.CalledProcessError(1, args, stderr=str(error)) from error
            return ''
        if args[:2] == ('git', 'rev-parse'):
            return 'b' * 40 if args[2].endswith('tree}') else self.refs['refs/heads/main']
        if args[:3] == ('bash', 'loop/retro.sh', 'spent'):
            return '0'
        if args[:3] == ('bash', 'loop/runs.sh', 'builders'):
            return ''
        if args[:3] == ('gh', 'pr', 'diff'):
            return '\n'.join(item['filename'] for item in self.api(f'pulls/{args[3]}/files?per_page=100'))
        if args[:3] == ('bash', 'loop/rules.sh', 'touches'):
            return subprocess.run(args, input=data, capture_output=True, text=True, check=True).stdout
        if args[:3] == ('bash', 'loop/rules.sh', 'publication-ready'):
            pr = self.prs[int(args[3])]
            exact = [e for e in self.events.get(pr['number'], []) if e['head'] == pr['head']['sha']]
            if not exact or exact[-1]['verdict'] != 'approve':
                raise subprocess.CalledProcessError(1, ['publication-ready'])
            return 'ready ' + pr['head']['sha']
        if args[:3] == ('bash', 'loop/rules.sh', 'verdicts'):
            return '\n'.join(json.dumps(e) for e in self.events.get(int(args[3]), []))
        if args[:3] == ('bash', 'loop/runs.sh', 'prompt'):
            return subprocess.run(args, input=data, capture_output=True, text=True, check=True).stdout
        if args[:3] in (('gh', 'workflow', 'run'), ('gh', 'pr', 'merge'), ('gh', 'pr', 'ready')):
            if args[:3] == ('gh', 'pr', 'ready'):
                self.prs[int(args[3])]['draft'] = '--undo' in args
            self.writes.append((args, None))
            return ''
        raise AssertionError(args)

    def merge(self, number, approve=True):
        pr = self.prs[number]
        pr.update(state='closed', merged=True)
        if approve:
            self.events[number] = [dict(head=pr['head']['sha'], verdict='approve')]



class Feedback(unittest.TestCase):
    def test_l90_untrusted_author(self):
        self.assertIsNone(f.intake(source(), False, None))

    def test_l90_report_only(self):
        for item in (source(number=351), source(labels=[dict(name='loop:report-only')]), source(body='loop:report-only')):
            self.assertIsNone(f.intake(item, True, None))
        self.assertIsNotNone(f.intake(source(body='No implementation was performed.'), True, None))

    def test_l90_partial_publication_child_keeps_its_parent_owner(self):
        child = source(number=356, labels=[dict(name='loop:work')],
                       body=answer()['deliveries'][0]['body'] + '\n<!-- feedback-delivery 355 observe-feedback -->')
        self.assertIsNone(f.intake(child, False, None))
        self.assertIsNone(f.intake(child, True, None))

    def test_l90_bot_update(self):
        original = f.intake(source(), True, None)
        self.assertEqual(f.intake(source(body='Bot progress'), False, original), original)
        self.assertEqual(f.intake(source(labels=[dict(name='flag:needs-user')]), True, original), original)
        self.assertEqual(f.intake(source(labels=[dict(name='loop:work')]), True, original)['kind'], 'feedback')

    def test_l90_changed_human_feedback(self):
        original = f.intake(source(), True, None)
        changed = f.intake(source(body='Observe a different result.'), True, original)
        self.assertNotEqual(original['revision'], changed['revision'])
        self.assertEqual(original['body'], 'Observe the requested result.')
        self.assertIsNone(f.intake(source(labels=[dict(name='loop:work'), dict(name='ready-for-agent')]), True, None))
        self.assertIsNotNone(f.intake(source(labels=[dict(name='loop:work')]), True, None))

    def test_l90_failed_evidence_successor(self):
        previous = dict(answer=answer())
        self.assertEqual(f.parse_answer(verification(False), 'verify', previous)['deliveries'], answer()['deliveries'])
        missing = verification(False)
        missing['deliveries'] = []
        with self.assertRaisesRegex(ValueError, 'successor'):
            f.parse_answer(missing, 'verify', previous)
        narrowed = verification(True)
        narrowed['goals'][0]['statement'] = 'CI is green'
        with self.assertRaisesRegex(ValueError, 'original goal'):
            f.parse_answer(narrowed, 'verify', previous)

    def test_l90_independent_goal_verification(self):
        self.assertEqual(f.parse_answer(verification(True), 'verify', dict(answer=answer())), verification(True))
        for change in (dict(evidence=[]), dict(evidence=[dict(goal=1, proved=True)])):
            with self.assertRaises(ValueError):
                f.parse_answer({**verification(True), **change}, 'verify', dict(answer=answer()))

    def test_l90_closed_child_starts_verification_not_completion(self):
        snapshot = f.intake(source(), True, None)
        record = dict(source=snapshot, phase='published', pr=2, children=[3], baseline=[[3, 'open', None]])
        with patch.object(f.d, 'api', return_value=dict(state='closed', state_reason='completed')):
            self.assertEqual(f.next_task(snapshot, record)['role'], 'verify')
        with patch.object(f.d, 'api', return_value=dict(state='open', state_reason=None)):
            self.assertIsNone(f.next_task(snapshot, record))

    def test_l90_output_cannot_execute_or_authorize_arbitrary_operations(self):
        for change in (dict(close=True), dict(deliveries=answer()['deliveries'] * 5),
                       dict(rules=[dict(path='.github/workflows/check.yml', content='forged gate', cases='case')])):
            with self.assertRaises(ValueError):
                f.parse_answer({**answer(), **change}, 'plan')

    def test_l90_bad_input_stops_before_network(self):
        for args in ([], ['start', '0'], ['repair', '1', 'stale'], ['publish', 'not-an-issue']):
            result = subprocess.run([sys.executable, 'loop/feedback.py', *args], capture_output=True, text=True)
            self.assertEqual(result.returncode, 2)

    def test_l90_github_failure_reports_redacted_diagnostics(self):
        with tempfile.TemporaryDirectory() as directory:
            gh = Path(directory) / 'gh'
            gh.write_text('#!/bin/sh\nprintf "PR creation disabled (HTTP 403), credential %s\\n" "$GH_TOKEN" >&2\nexit 1\n')
            gh.chmod(0o755)
            result = subprocess.run([sys.executable, 'loop/feedback.py', 'publish', '355'],
                                    env={**os.environ, 'PATH': directory + os.pathsep + os.environ['PATH'], 'GH_TOKEN': 'private-test-credential'},
                                    text=True, capture_output=True)
        self.assertEqual(result.returncode, 4)
        self.assertIn('PR creation disabled (HTTP 403)', result.stderr)
        self.assertIn('[redacted]', result.stderr)
        self.assertNotIn('private-test-credential', result.stderr)

    def test_l90_collaborator_feedback(self):
        item = source()
        self.assertEqual(f.intake(item, True, None)['body'], item['body'])
        self.assertIsNone(f.intake(source(labels=[dict(name='loop:status')]), True, None))


class Lifecycle(unittest.TestCase):
    def setUp(self):
        self.hub = GitHub()
        output = tempfile.NamedTemporaryFile()
        self.addCleanup(output.close)
        for replacement in (patch.object(f.d, 'api', side_effect=self.hub.api),
                            patch.object(f.d, 'gh', side_effect=self.hub.gh),
                            patch.object(f.d, 'run', side_effect=self.hub.run),
                            patch.object(f.orders, 'gh', side_effect=self.hub.gh),
                            patch.object(f.orders.trail, 'gh', side_effect=self.hub.gh),
                            patch.dict(os.environ, GITHUB_REPOSITORY='o/r', GITHUB_RUN_ID='42', GITHUB_OUTPUT=output.name, LOOP_DAILY_TOKENS='1000')):
            replacement.start()
            self.addCleanup(replacement.stop)

    def plan(self, result=None):
        f.start(355)
        f.propose(355, result or answer())
        return f.read_claim(355)[1]['pr']

    def published(self, result=None):
        pr = self.plan(result)
        self.hub.merge(pr)
        f.publish(355)
        return f.read_claim(355)[1]

    def unpublished(self):
        self.hub.pull_error = subprocess.CalledProcessError(1, ['gh'], stderr='PR creation is disabled (HTTP 403)')
        with self.assertRaises(subprocess.CalledProcessError):
            self.plan()
        self.hub.pull_error = None
        return f.read_claim(355)[1]

    def test_l90_reviewed_plan_publication(self):
        pr = self.plan()
        self.assertEqual(list(self.hub.issues), [355])
        self.hub.merge(pr, approve=False)
        with self.assertRaises(subprocess.CalledProcessError):
            f.publish(355)
        self.assertEqual(list(self.hub.issues), [355])
        self.hub.merge(pr)
        f.publish(355)
        f.publish(355)
        record = f.read_claim(355)[1]
        self.assertEqual(record['children'], [356])
        self.assertEqual(self.hub.children[355], [356])
        self.assertIn(dict(name='ready-for-agent'), self.hub.issues[356]['labels'])
        self.assertEqual(self.hub.issues[355]['body'], source()['body'])
        self.assertEqual(self.hub.issues[355]['state'], 'open')

    def test_l90_unreviewed_merge_gets_a_fresh_plan_not_delivery(self):
        pr = self.plan()
        old = f.read_claim(355)[1]
        self.hub.merge(pr, approve=False)
        self.hub.runs[42] = dict(status='completed', conclusion='success')
        f.reconcile()
        record = f.read_claim(355)[1]
        self.assertEqual(record['phase'], 'replan')
        self.assertEqual(record['recovery']['pr'], pr)
        self.assertEqual(self.hub.children, {})
        os.environ['GITHUB_RUN_ID'] = '43'
        f.start(355)
        current = f.read_claim(355)[1]
        self.assertEqual(current['phase'], 'issued')
        self.assertNotEqual(f.proposal_path(current), f.proposal_path(old))
        f.propose(355, answer())
        replacement = f.read_claim(355)[1]['pr']
        self.assertNotEqual(pr, replacement)
        self.assertEqual(self.hub.children, {})
        self.hub.merge(replacement)
        f.publish(355)
        self.assertEqual(f.read_claim(355)[1]['children'], [356])

    def test_l90_unreviewed_verification_retains_original_goals(self):
        record = self.published()
        self.hub.issues[record['children'][0]].update(state='closed', state_reason='completed')
        os.environ['GITHUB_RUN_ID'] = '43'
        f.start(355)
        f.propose(355, verification(True))
        self.hub.merge(f.read_claim(355)[1]['pr'], approve=False)
        self.hub.runs[43] = dict(status='completed', conclusion='success')
        f.reconcile()
        os.environ['GITHUB_RUN_ID'] = '44'
        f.start(355)
        current = f.read_claim(355)[1]
        self.assertEqual(current['key'][0], 'verify')
        self.assertEqual(current['previous']['answer']['goals'], record['plan']['answer']['goals'])
        weaker = verification(True)
        weaker['goals'][0]['statement'] = 'A weaker goal'
        with self.assertRaisesRegex(ValueError, 'retain every original goal'):
            f.propose(355, weaker)

    def test_l93_publication_timeout_does_not_starve_later_feedback(self):
        pr = self.plan()
        self.hub.merge(pr)
        self.hub.issues[356] = source(number=356)
        run = self.hub.run
        def timeout(*args, **kwargs):
            if args[:3] == (sys.executable, 'loop/feedback.py', 'publish'):
                raise subprocess.TimeoutExpired(args, 180)
            return run(*args, **kwargs)
        with patch.object(f.d, 'run', side_effect=timeout), self.assertRaisesRegex(ValueError, 'unknown'):
            f.reconcile()
        self.assertTrue(any('issue=356' in call for call, _ in self.hub.writes))

    def test_l93_resumed_publication_failure_does_not_starve_later_feedback(self):
        self.unpublished()
        self.hub.runs[42] = dict(status='completed', conclusion='failure')
        self.hub.pull_error = subprocess.CalledProcessError(1, ['gh'], stderr='denied')
        self.hub.issues[356] = source(number=356)
        with self.assertRaisesRegex(ValueError, 'resumed publication failed'):
            f.reconcile()
        self.assertTrue(any('issue=356' in call for call, _ in self.hub.writes))

    def test_l90_blocked_publication_does_not_starve_later_feedback(self):
        pr = self.plan(answer(issue=999))
        self.hub.merge(pr)
        self.hub.issues[356] = source(number=356, title='Another authorized request')
        with self.assertRaisesRegex(ValueError, 'publication blocked'):
            f.reconcile()
        self.assertTrue(any(call[:4] == ('gh', 'workflow', 'run', 'feedback.yml')
                            and 'issue=356' in call for call, _ in self.hub.writes))
        self.assertEqual(f.read_claim(355)[1]['phase'], 'proposed')
        self.assertEqual(self.hub.children, {})

    def test_l90_duplicate_and_competing_claims(self):
        f.start(355)
        self.hub.runs[42] = dict(status='in_progress')
        before = len(self.hub.writes)
        f.start(355)
        self.assertEqual(len(self.hub.writes), before)
        old, record = f.read_claim(355)
        first = {**record, 'phase': 'issued', 'run': 43}
        second = {**record, 'phase': 'issued', 'run': 44}
        f.save_claim(355, old, first)
        with self.assertRaisesRegex(ValueError, 'Claim changed'):
            f.save_claim(355, old, second)
        self.assertEqual(f.read_claim(355)[1]['run'], 43)

    def test_l90_ended_owner_has_only_two_attempts(self):
        f.start(355)
        os.environ['GITHUB_RUN_ID'] = '43'
        f.start(355)
        self.assertEqual(f.read_claim(355)[1]['attempt'], 2)
        os.environ['GITHUB_RUN_ID'] = '44'
        before = len(self.hub.writes)
        f.start(355)
        self.assertEqual(len(self.hub.writes), before)

    def test_l90_reuse_owner(self):
        result = answer(349)
        delivery = result['deliveries'][0]
        self.hub.issues[349] = {**source(number=349, title=delivery['title'], body=delivery['body']), 'id': 349,
                                'labels': [dict(name='loop:work'), dict(name='ready-for-agent')]}
        record = self.published(result)
        self.assertEqual(record['children'], [349])
        self.assertEqual(sorted(self.hub.issues), [349, 355])
        self.assertEqual(self.hub.issues[349]['body'], delivery['body'])

    def test_l90_unready_work_is_authorized_after_its_plan_without_duplication(self):
        result = answer(355)
        delivery = result['deliveries'][0]
        self.hub.issues[355].update(title=delivery['title'], body=delivery['body'], labels=[dict(name='loop:work')])
        record = self.published(result)
        self.assertEqual(record['source']['kind'], 'work')
        self.assertEqual(record['children'], [355])
        self.assertEqual(list(self.hub.issues), [355])
        self.assertIn(dict(name='ready-for-agent'), self.hub.issues[355]['labels'])

    def test_l90_migrated_owner_reuses_its_existing_acceptance_observer(self):
        result = answer(349)
        delivery = result['deliveries'][0]
        delivery['body'] = delivery['body'].replace('Observer: Execute a feedback cycle\n', '')
        self.hub.issues[349] = {**source(number=349, title=delivery['title'], body=delivery['body']), 'id': 349,
                                'labels': [dict(name='loop:work'), dict(name='ready-for-agent')]}
        record = self.published(result)
        self.assertEqual(record['children'], [349])
        self.assertEqual(self.hub.issues[349]['body'], delivery['body'])

    def test_l90_reused_child_keeps_its_existing_parent(self):
        result = answer(349)
        delivery = result['deliveries'][0]
        self.hub.issues[349] = {**source(number=349, title=delivery['title'], body=delivery['body']), 'id': 349,
                                'labels': [dict(name='loop:work'), dict(name='ready-for-agent')]}
        self.hub.issues[394] = source(number=394)
        self.hub.children[394] = [349]
        self.published(result)
        self.assertEqual(self.hub.children[394], [349])
        self.assertEqual(self.hub.blocked[355], [349])
        self.assertNotIn(355, self.hub.children)

    def test_l90_live_owner_cannot_be_rewritten(self):
        result = answer(349)
        delivery = result['deliveries'][0]
        self.hub.issues[349] = {**source(number=349, title='Existing owner', body=delivery['body']), 'id': 349,
                                'labels': [dict(name='loop:work'), dict(name='ready-for-agent')]}
        pr = self.plan(result)
        self.hub.merge(pr)
        with self.assertRaisesRegex(ValueError, 'reused unchanged'):
            f.publish(355)
        self.assertEqual(self.hub.issues[349]['title'], 'Existing owner')

    def test_l90_cancelled_dependency(self):
        self.hub.issues[399] = {**source(number=399), 'id': 399, 'state': 'closed', 'state_reason': 'not_planned'}
        result = answer()
        result['deliveries'][0]['blocked_by'] = ['#399']
        record = self.published(result)
        child = record['children'][0]
        self.assertEqual(self.hub.blocked[child], [399])
        rows = f.orders.read_orders()
        self.assertEqual(rows[0]['state'], 'waiting')

    def test_l90_independent_goal_verification(self):
        record = self.published()
        self.hub.issues[record['children'][0]].update(state='closed', state_reason='completed')
        os.environ['GITHUB_RUN_ID'] = '43'
        f.start(355)
        self.assertEqual(f.read_claim(355)[1]['key'][0], 'verify')
        f.propose(355, verification(True))
        pr = f.read_claim(355)[1]['pr']
        self.assertEqual(self.hub.issues[355]['state'], 'open')
        self.hub.merge(pr)
        f.publish(355)
        self.assertEqual(self.hub.issues[355]['state'], 'closed')
        self.assertEqual(f.read_claim(355)[1]['phase'], 'complete')

    def test_l90_unreviewed_or_stale_closure(self):
        record = self.published()
        self.hub.issues[record['children'][0]].update(state='closed', state_reason='completed')
        os.environ['GITHUB_RUN_ID'] = '43'
        f.start(355)
        f.propose(355, verification(True))
        pr = f.read_claim(355)[1]['pr']
        self.hub.merge(pr, approve=False)
        with self.assertRaises(subprocess.CalledProcessError):
            f.publish(355)
        self.hub.events[pr] = [dict(head='c' * 40, verdict='approve')]
        with self.assertRaises(subprocess.CalledProcessError):
            f.publish(355)
        self.hub.merge(pr)
        self.hub.main_changes = [dict(filename='apps/desktop/src/App.tsx')]
        with self.assertRaisesRegex(ValueError, 'main behavior changed'):
            f.publish(355)
        self.hub.main_changes = []
        self.hub.issues[355]['body'] = 'Also observe the real application.'
        with self.assertRaisesRegex(ValueError, 'stale'):
            f.publish(355)
        self.assertEqual(self.hub.issues[355]['state'], 'open')

    def test_l90_existing_behavior_needs_verification_not_duplicate_work(self):
        result = answer()
        result['deliveries'] = []
        record = self.published(result)
        self.assertEqual(record['children'], [])
        self.assertEqual(list(self.hub.issues), [355])
        self.assertEqual(f.next_task(record['source'], record)['role'], 'verify')
        self.assertEqual(self.hub.issues[355]['state'], 'open')

    def test_l90_failed_evidence_successor(self):
        record = self.published()
        self.hub.issues[record['children'][0]].update(state='closed', state_reason='completed')
        os.environ['GITHUB_RUN_ID'] = '43'
        f.start(355)
        result = verification(False)
        result['deliveries'][0]['body'] = result['deliveries'][0]['body'].replace('Key: observe-feedback', 'Key: remaining-feedback')
        f.propose(355, result)
        pr = f.read_claim(355)[1]['pr']
        self.hub.merge(pr)
        f.publish(355)
        self.assertEqual(f.read_claim(355)[1]['children'], [357])
        self.assertEqual(self.hub.issues[355]['state'], 'open')
        self.assertEqual(f.read_claim(355)[1]['plan']['evidence'][0]['remaining'], 'Source feedback remains unproved')

    def test_l90_failed_evidence_cannot_recycle_only_closed_owners(self):
        record = self.published()
        child = record['children'][0]
        self.hub.issues[child].update(state='closed', state_reason='completed')
        os.environ['GITHUB_RUN_ID'] = '43'
        f.start(355)
        result = verification(False)
        result['deliveries'][0]['issue'] = child
        f.propose(355, result)
        pr = f.read_claim(355)[1]['pr']
        with self.assertRaisesRegex(ValueError, 'open successor'):
            f.gate(pr)
        self.assertEqual(self.hub.issues[355]['state'], 'open')

    def test_l90_changed_main_schedules_fresh_observation(self):
        record = self.published()
        self.hub.issues[record['children'][0]].update(state='closed', state_reason='completed')
        os.environ['GITHUB_RUN_ID'] = '43'
        f.start(355)
        f.propose(355, verification(True))
        old, record = f.read_claim(355)
        self.hub.main_changes = [dict(filename='crates/rupd/src/main.rs')]
        self.assertTrue(f.refresh_verification(355, old, record))
        refreshed = f.read_claim(355)[1]
        self.assertEqual(f.next_task(refreshed['source'], refreshed)['role'], 'verify')
        self.assertEqual(self.hub.prs[record['pr']]['state'], 'closed')
        self.assertEqual(self.hub.issues[355]['state'], 'open')

    def test_l90_verification_retry_keeps_original_goals(self):
        record = self.published()
        self.hub.issues[record['children'][0]].update(state='closed', state_reason='completed')
        os.environ['GITHUB_RUN_ID'] = '43'
        f.start(355)
        os.environ['GITHUB_RUN_ID'] = '44'
        f.start(355)
        self.assertEqual(f.read_claim(355)[1]['previous']['answer']['goals'], answer()['goals'])

    def test_l90_bot_authored_unready_work_is_owned_triage(self):
        item = source(labels=[dict(name='loop:work')], user=dict(login='github-actions[bot]', type='Bot'))
        self.hub.issues[355] = item
        self.hub.editor = 'github-actions[bot]'
        f.start(355)
        self.assertEqual(f.read_claim(355)[1]['key'][0], 'plan')

    def test_l90_repair_uses_shared_claim_and_trusted_publication(self):
        number = self.plan()
        pr = self.hub.prs[number]
        head = pr['head']['sha']
        task = dict(pr=number, role='fix', cause='reject', head=head, base='a' * 40, branch=pr['head']['ref'], planning=True)
        os.environ['GITHUB_RUN_ID'] = '43'
        # Acquiring the real shared per-PR Claim does not publish model output.
        self.hub.events[number] = [dict(head=head, current=True, verdict='reject')]
        f.repair(number, head)
        self.assertEqual(f.d.claim_record(number)[1]['key'], f.d.key(task))
        result = answer()
        result['deliveries'][0]['title'] = 'Address the independent reject'
        f.repair_propose(number, head, 'a' * 40, result)
        updated = self.hub.prs[number]['head']['sha']
        self.assertNotEqual(updated, head)
        proposal, _ = f.read_proposal(self.hub.prs[number])
        self.assertEqual(proposal['answer']['deliveries'][0]['title'], 'Address the independent reject')
        self.assertEqual(list(self.hub.issues), [355])
        self.assertEqual(f.d.fixes(f.d.claim_record(number)[1]), 1)
        with self.assertRaisesRegex(ValueError, 'stale planning repair'):
            f.repair_propose(number, head, 'a' * 40, result)

    def test_l90_reviewed_closure_keywords_cannot_bypass_verification(self):
        number = self.plan()
        self.hub.prs[number]['body'] += '\nCloses #355'
        with self.assertRaisesRegex(ValueError, 'auto-close'):
            f.gate(number)

    def test_l90_missed_check_dispatch_is_recovered_without_an_agent(self):
        number = self.plan()
        before = len(self.hub.writes)
        f.publish(355)
        dispatches = [write[0] for write in self.hub.writes[before:] if isinstance(write[0], tuple)]
        self.assertIn(('gh', 'workflow', 'run', 'check.yml', '--ref', self.hub.prs[number]['head']['ref']), dispatches)
        self.assertEqual(f.read_claim(355)[1]['attempt'], 1)

    def test_l90_publication_failure_resumes_without_another_planner(self):
        self.unpublished()
        branch = next(ref for ref in self.hub.refs if ref.startswith('refs/heads/build/'))
        head = self.hub.refs[branch]
        os.environ['GITHUB_RUN_ID'] = '43'
        f.reconcile()
        record = f.read_claim(355)[1]
        self.assertEqual(record['phase'], 'proposed')
        self.assertEqual(self.hub.prs[record['pr']]['head']['sha'], head)
        self.assertEqual(record['attempt'], 1)
        self.assertEqual(record['run'], 43)
        self.assertIn('Author-Agent: feedback-42', self.hub.commits[head]['message'])
        self.assertNotIn((('gh', 'workflow', 'run', 'feedback.yml', '-f', 'issue=355'), None), self.hub.writes)
        self.assertEqual(list(self.hub.issues), [355])

    def test_l90_publication_recovery_waits_for_the_live_owner(self):
        self.hub.lost_pull_response = True
        with self.assertRaises(subprocess.TimeoutExpired):
            self.plan()
        self.hub.runs[42] = dict(status='in_progress')
        os.environ['GITHUB_RUN_ID'] = '43'
        before = copy.deepcopy(self.hub.writes)
        f.reconcile()
        self.assertEqual(self.hub.writes, before)

    def test_l90_queued_start_keeps_a_saved_proposal_for_publication(self):
        self.unpublished()
        os.environ['GITHUB_RUN_ID'] = '43'
        before = copy.deepcopy(self.hub.writes)
        f.start(355)
        self.assertEqual(self.hub.writes, before)

    def test_l90_lost_pull_response_reuses_the_existing_pr(self):
        self.hub.lost_pull_response = True
        with self.assertRaises(subprocess.TimeoutExpired):
            self.plan()
        os.environ['GITHUB_RUN_ID'] = '43'
        f.reconcile()
        self.assertEqual(list(self.hub.prs), [1])
        self.assertEqual(f.read_claim(355)[1]['pr'], 1)
        self.assertEqual(list(self.hub.issues), [355])

    def test_l90_interrupted_publication_recovery_keeps_the_proposal(self):
        self.unpublished()
        self.hub.pull_error = subprocess.CalledProcessError(1, ['gh'])
        os.environ['GITHUB_RUN_ID'] = '43'
        with self.assertRaisesRegex(ValueError, 'resumed publication failed'):
            f.reconcile()
        self.assertEqual(f.read_claim(355)[1]['phase'], 'publishing')
        self.hub.pull_error = None
        os.environ['GITHUB_RUN_ID'] = '44'
        f.reconcile()
        record = f.read_claim(355)[1]
        self.assertEqual(record['phase'], 'proposed')
        self.assertEqual(record['attempt'], 1)
        self.assertEqual(list(self.hub.prs), [1])

    def test_l90_changed_feedback_does_not_publish_the_saved_proposal(self):
        self.unpublished()
        self.hub.issues[355]['body'] = 'Observe a different result.'
        os.environ['GITHUB_RUN_ID'] = '43'
        f.reconcile()
        self.assertEqual(self.hub.prs, {})
        self.assertIn((('gh', 'workflow', 'run', 'feedback.yml', '-f', 'issue=355'), None), self.hub.writes)

    def test_l90_saved_proposal_cannot_replace_controller_context(self):
        record = self.unpublished()
        head = self.hub.refs['refs/heads/' + f.proposal_branch(record)]
        files = self.hub.trees[self.hub.commits[head]['tree']['sha']]
        proposal = json.loads(files[f.proposal_path(record)])
        proposal['source']['body'] = 'Forged source feedback'
        files[f.proposal_path(record)] = json.dumps(proposal)
        os.environ['GITHUB_RUN_ID'] = '43'
        with self.assertRaisesRegex(ValueError, 'controller-owned context'):
            f.reconcile()
        self.assertEqual(self.hub.prs, {})

    def test_l88_excluded_unclaimed_issues_need_no_ownership_reads(self):
        for labels, author in ((['loop:work', 'ready-for-agent'], 'User'), ([], 'Bot')):
            self.hub.issues[355].update(labels=[dict(name=name) for name in labels], user=dict(login='author', type=author))
            with patch.object(f, 'read_claim') as read:
                f.reconcile()
                read.assert_not_called()
            self.assertEqual(self.hub.writes, [])

    def test_l90_intake_dispatches_without_a_user_label(self):
        f.reconcile()
        self.assertIn((('gh', 'workflow', 'run', 'feedback.yml', '-f', 'issue=355'), None), self.hub.writes)

    def test_l90_spend_timeout_leaves_no_claim_or_dispatch(self):
        original = self.hub.run
        def timeout(*args, **kwargs):
            if args[:3] == ('bash', 'loop/retro.sh', 'spent'):
                self.assertEqual(kwargs['timeout'], 600)
                raise subprocess.TimeoutExpired(args, kwargs['timeout'])
            return original(*args, **kwargs)
        with patch.object(f.d, 'run', side_effect=timeout):
            for operation in (lambda: f.start(355), f.reconcile):
                with self.assertRaises(subprocess.TimeoutExpired):
                    operation()
                self.assertEqual(self.hub.writes, [])

    def test_l90_spend_cap_does_not_create_a_dispatch_chain(self):
        os.environ['LOOP_DAILY_TOKENS'] = '0'
        f.reconcile()
        self.assertEqual(self.hub.writes, [])

    def test_l90_untrusted_editor_cannot_start_a_plan(self):
        self.hub.permission = 'read'
        f.reconcile()
        self.assertEqual(self.hub.writes, [])

    def test_l90_changed_feedback_after_completion_reopens_the_source(self):
        record = self.published()
        self.hub.issues[record['children'][0]].update(state='closed', state_reason='completed')
        os.environ['GITHUB_RUN_ID'] = '43'
        f.start(355)
        f.propose(355, verification(True))
        self.hub.merge(f.read_claim(355)[1]['pr'])
        f.publish(355)
        self.hub.issues[355]['body'] = 'Also observe interruption.'
        f.reconcile()
        self.assertEqual(self.hub.issues[355]['state'], 'open')
        self.assertEqual(self.hub.writes[-1][0], ('gh', 'workflow', 'run', 'feedback.yml', '-f', 'issue=355'))

    def test_l90_changed_human_feedback_keeps_prior_source(self):
        f.start(355)
        old, record = f.read_claim(355)
        self.hub.issues[355]['body'] = 'Observe more than CI.'
        os.environ['GITHUB_RUN_ID'] = '43'
        f.start(355)
        current = f.read_claim(355)[1]
        self.assertNotEqual(current['source']['revision'], record['source']['revision'])
        self.assertEqual(json.loads(self.hub.commits[old]['message'])['source']['body'], source()['body'])

    def test_l90_lost_issue_response_reuses_the_created_child(self):
        pr = self.plan()
        self.hub.merge(pr)
        self.hub.lost_issue_response = True
        with self.assertRaises(subprocess.TimeoutExpired):
            f.publish(355)
        self.assertNotIn(dict(name='ready-for-agent'), self.hub.issues[356]['labels'])
        f.start(356)
        self.assertIsNone(f.read_claim(356)[1])
        f.publish(355)
        self.assertIn('<!-- feedback-delivery 355 observe-feedback -->', self.hub.issues[356]['body'])
        self.assertEqual(sorted(self.hub.issues), [355, 356])
        self.assertEqual(f.read_claim(355)[1]['children'], [356])


if __name__ == '__main__':
    unittest.main()
