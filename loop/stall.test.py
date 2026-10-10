#!/usr/bin/env python3
import importlib.util
import os
import time
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('stall', ROOT / 'loop/stall.py')
stall = importlib.util.module_from_spec(spec)
spec.loader.exec_module(stall)
builder, orders = stall.builder, stall.orders

NOW = 1_791_000_000.0


def stamp(seconds_ago):
    return time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime(NOW - seconds_ago))


HOUR = 3600
READY = dict(number=7, title='Build U1', html_url='https://x/7', state='open', body='', created_at=stamp(HOUR * 20),
             labels=[], user=dict(login='saadshahd'))
BUILD_PR = dict(number=9, headRefName='build/U1', isCrossRepository=False, labels=[])


class GitHub:
    """Answers stall.py's reads from fixed data and records its writes."""

    def __init__(self, merged_ago=None, issues=(), prs=(), runs=(), jobs=None):
        self.merged_ago, self.issues, self.prs, self.runs = merged_ago, list(issues), list(prs), list(runs)
        self.jobs = jobs or {}
        self.writes = []

    def read(self, *args):
        if args[:2] == ('pr', 'list'):
            if 'merged' in args:
                return [] if self.merged_ago is None else [dict(number=5, url='https://x/pull/5', mergedAt=stamp(self.merged_ago))]
            return self.prs
        path = args[1]
        if '/jobs' in path:
            return dict(jobs=self.jobs.get(int(path.split('/runs/')[1].split('/')[0]), []))
        if '/actions/runs' in path:
            return dict(workflow_runs=self.runs)
        if '/issues?' in path:
            return [self.issues]
        return dict(labels=[l for i in self.issues for l in i['labels']] if self.issues else [])

    def write(self, *args):
        self.writes.append(args)
        if args[:2] == ('issue', 'create'):
            number = 100 + len([w for w in self.writes if w[:2] == ('issue', 'create')])
            return f'https://github.com/o/r/issues/{number}\n'
        return ''

    def __enter__(self):
        self.patches = [patch.object(orders, 'gh', side_effect=self.read), patch.object(builder, 'call', side_effect=self.write),
                        patch.object(stall.builder.orders, 'gh', side_effect=self.read)]
        for p in self.patches:
            p.start()
        return self

    def __exit__(self, *_):
        for p in self.patches:
            p.stop()

    def of(self, *head):
        return [w for w in self.writes if w[:len(head)] == head]

    def comments(self):
        return [w[3].removeprefix('body=') for w in self.of('api') if w[1].endswith('/comments')]


def stalled(**changes):
    return GitHub(**dict(dict(merged_ago=HOUR * 4, issues=[READY]), **changes))


def stall_issue(ago=HOUR, labels=()):
    return dict(number=50, title='Loop stall', html_url='https://x/50', state='open', created_at=stamp(ago),
                body='Key: stall-20261010-03\n', labels=[dict(name=n) for n in ('loop:work', *labels)],
                user=dict(login='github-actions[bot]'))


def run(id, workflow, conclusion, ago=HOUR):
    return dict(id=id, path=f'.github/workflows/{workflow}.yml', html_url=f'https://run/{id}', conclusion=conclusion,
                status='completed', created_at=stamp(ago))


class Stall(unittest.TestCase):
    def go(self, github):
        with github:
            stall.stall(NOW)
        return github

    def test_l94_no_issue_when_no_work_waits(self):
        github = self.go(GitHub(merged_ago=HOUR * 30, issues=[], prs=[]))
        self.assertEqual(github.writes, [])

    def test_l94_no_work_leaves_an_open_stall_open(self):
        github = self.go(GitHub(merged_ago=HOUR * 30, issues=[stall_issue()]))
        self.assertEqual(github.writes, [])

    def test_l94_a_recent_merge_is_not_a_stall(self):
        self.assertEqual(self.go(GitHub(merged_ago=HOUR * 2, issues=[READY])).writes, [])

    def test_l94_a_user_flagged_pr_is_not_waiting_work(self):
        pr = dict(BUILD_PR, labels=[dict(name='flag:needs-user')])
        self.assertEqual(self.go(GitHub(merged_ago=HOUR * 4, prs=[pr])).writes, [])

    def test_l94_an_issue_waiting_on_the_user_or_filed_outside_is_not_waiting_work(self):
        flagged = dict(READY, labels=[dict(name='flag:needs-user')])
        outside = dict(READY, user=dict(login='stranger'))
        pull = dict(READY, pull_request={})
        for issue in (flagged, outside, pull):
            with self.subTest(issue=issue):
                self.assertEqual(self.go(GitHub(merged_ago=HOUR * 4, issues=[issue])).writes, [])

    def test_l94_a_cross_repository_pr_is_not_waiting_work(self):
        pr = dict(BUILD_PR, isCrossRepository=True)
        self.assertEqual(self.go(GitHub(merged_ago=HOUR * 4, prs=[pr])).writes, [])

    def test_l94_first_stall_opens_one_implement_issue(self):
        github = self.go(stalled(prs=[BUILD_PR]))
        create = github.of('issue', 'create')
        self.assertEqual(len(create), 1)
        body = create[0][create[0].index('--body') + 1]
        key = 'stall-' + time.strftime('%Y%m%d-%H', time.gmtime(NOW))
        for line in ('Scenarios: L94', f'Key: {key}', 'Priority: 0', 'Mode: implement'):
            self.assertIn('\n' + line + '\n', '\n' + body)
        self.assertIn('loop:work', create[0])
        self.assertNotIn('ready-for-agent', create[0])
        os.chdir(ROOT)
        issue = dict(number=100, title='Loop stall', html_url='https://x/100', state='open', body=body,
                     labels=[dict(name='loop:work')], user=dict(login='github-actions[bot]'))
        row = orders.classify([issue], {}, [])[0]
        self.assertEqual((row['state'], row['mode']), ('ready', 'implement'))

    def test_l94_body_lists_merge_waiting_work_and_runs(self):
        runs = [run(1, 'build', 'failure'), run(2, 'review', 'success'), run(3, 'check', 'failure'), run(4, 'fix', 'success')]
        jobs = {1: [dict(steps=[dict(name='Run claude-code-action', conclusion='failure')])],
                2: [dict(steps=[dict(name='Run claude-code-action', conclusion='skipped')])],
                4: [dict(steps=[dict(name='Run claude-code-action', conclusion='success')])]}
        github = self.go(stalled(prs=[BUILD_PR], runs=runs, jobs=jobs))
        create = github.of('issue', 'create')[0]
        body = create[create.index('--body') + 1]
        for text in ('#5 https://x/pull/5', 'Issue #7 Build U1', 'PR #9 build/U1', 'build https://run/1: failure (usage limit',
                     'review https://run/2: success (no model work', 'fix https://run/4: success\n'):
            self.assertIn(text, body)
        self.assertNotIn('run/3', body)

    def test_l94_next_run_comments_not_a_second_issue(self):
        github = self.go(stalled(issues=[READY, stall_issue()]))
        self.assertEqual(github.of('issue', 'create'), [])
        self.assertEqual(len(github.comments()), 1)
        self.assertIn('Waiting:', github.comments()[0])

    def test_l94_before_six_hours_does_not_ask(self):
        github = self.go(stalled(merged_ago=HOUR * 5))
        self.assertFalse([w for w in github.writes if any('flag:needs-user' in str(a) for a in w)])

    def test_l94_six_hours_asks_the_user_once(self):
        github = self.go(stalled(merged_ago=HOUR * 7, runs=[run(1, 'build', 'failure')]))
        asks = [b for b in github.comments() if b.startswith('<!-- needs-user -->')]
        self.assertEqual(len(asks), 1)
        self.assertIn('stalled for 7+ hours', asks[0])
        self.assertIn('https://run/1', asks[0])
        self.assertEqual(len(github.of('api', 'repos/{owner}/{repo}/issues/101/labels')), 1)

    def test_l94_six_hours_asks_nothing_more_while_flagged(self):
        flagged = stall_issue(labels=('flag:needs-user',))
        github = self.go(stalled(merged_ago=HOUR * 8, issues=[READY, flagged]))
        self.assertEqual([b for b in github.comments() if b.startswith('<!-- needs-user -->')], [])

    def test_l94_first_run_after_a_merge_closes_the_issue(self):
        flagged = stall_issue(ago=HOUR * 2, labels=('flag:needs-user',))
        github = self.go(GitHub(merged_ago=HOUR, issues=[flagged]))
        close = github.of('issue', 'close')
        self.assertEqual(len(close), 1)
        self.assertIn('completed', close[0])
        self.assertIn('PR #5', close[0][-1])
        self.assertEqual(len(github.of('api', '-X', 'DELETE')), 1)

    def test_l94_a_merge_before_the_issue_opened_never_closes_it(self):
        github = self.go(GitHub(merged_ago=HOUR * 5, issues=[stall_issue(ago=HOUR)]))
        self.assertEqual(github.of('issue', 'close'), [])

    def test_l94_a_gh_failure_is_not_read_as_no_stall(self):
        with patch.object(orders, 'gh', side_effect=OSError('gh failed')):
            with self.assertRaises(OSError):
                stall.stall(NOW)


if __name__ == '__main__':
    unittest.main()
