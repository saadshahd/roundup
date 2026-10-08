#!/usr/bin/env python3
"""L88 dispatch ownership and L89 gate parity."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
sys.dont_write_bytecode = True
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('dispatch', ROOT / 'loop/dispatch.py')
d = importlib.util.module_from_spec(spec)
spec.loader.exec_module(d)
HEAD = 'a' * 40
BASE = 'b' * 40
PR = dict(state='open', draft=False, body='', mergeable=True,
          head=dict(sha=HEAD, ref='build/U1', repo=dict(full_name='o/r')),
          base=dict(sha=BASE, ref='main', repo=dict(full_name='o/r')))
CHECK = dict(id=1, name='check', head_sha=HEAD, status='completed', conclusion='success', app=dict(slug='github-actions'))
TASK = dict(pr=1, role='review', cause='review', head=HEAD, base=BASE, branch='build/U1')


class L88(unittest.TestCase):
    def test_l88_review_requires_successful_current_head_check(self):
        self.assertEqual(d.decision(PR, [CHECK], [], ['code']), ('review', 'review'))
        for change in [dict(head_sha=BASE), dict(status='in_progress'), dict(conclusion='skipped'), dict(app=dict(slug='other'))]:
            self.assertIsNone(d.decision(PR, [{**CHECK, **change}], [], ['code']))
        self.assertIsNone(d.decision(PR, [CHECK], [dict(current=True, verdict='approve')], ['code']))
        self.assertIsNone(d.decision(PR, [CHECK], [], ['docs']))

    def test_l88_conflicts_and_red_checks_get_repair(self):
        self.assertEqual(d.decision({**PR, 'mergeable': False}, [], [], ['code']), ('fix', 'conflict'))
        self.assertEqual(d.decision(PR, [{**CHECK, 'conclusion': 'failure'}], [], ['code']), ('fix', 'check'))
        self.assertEqual(d.decision(PR, [], [dict(current=True, verdict='reject')], ['code']), ('fix', 'reject'))
        self.assertIsNone(d.decision(PR, [], [dict(current=False, verdict='reject')], ['code']))

    def test_l88_latest_check_wins_and_skipped_is_not_a_result(self):
        self.assertIsNone(d.decision(PR, [CHECK, {**CHECK, 'id': 2, 'status': 'in_progress'}], [], ['code']))
        self.assertEqual(d.decision(PR, [CHECK, {**CHECK, 'id': 2, 'conclusion': 'skipped'}], [], ['code']), ('review', 'review'))

    def test_l88_explicit_questions_and_second_reject_keep_gates(self):
        self.assertIsNone(d.decision({**PR, 'draft': True, 'body': 'Stopped: choose semantics'}, [CHECK], [], ['code']))
        self.assertEqual(d.decision({**PR, 'draft': True, 'body': 'Stopped: the run ended without marking it ready, url'}, [], [], ['code']), ('fix', 'cutoff'))
        self.assertIsNone(d.decision({**PR, 'mergeable': False}, [], [dict(current=True, verdict='reject')] * 2, ['code']))
        self.assertIsNone(d.decision({**PR, 'state': 'closed'}, [CHECK], [], ['code']))
        fork = copy.deepcopy(PR)
        fork['head']['repo']['full_name'] = 'fork/r'
        self.assertIsNone(d.decision(fork, [CHECK], [], ['code']))
        fork['head']['repo'] = None
        self.assertIsNone(d.decision(fork, [CHECK], [], ['code']))

    def test_l88_live_owner_blocks_both_review_and_repair(self):
        record = dict(run=9, key=d.key(TASK), attempt=1)
        with patch.object(d, 'api', return_value=dict(status='in_progress')):
            self.assertFalse(d.available(TASK, record))
            self.assertFalse(d.available({**TASK, 'role': 'fix', 'cause': 'conflict'}, record))
        with patch.object(d, 'api', return_value=dict(status='completed')), patch.object(d, 'consumed', side_effect=lambda r: r['attempt']):
            self.assertTrue(d.available(TASK, record))
            self.assertFalse(d.available(TASK, {**record, 'attempt': 2}))
            self.assertTrue(d.available({**TASK, 'head': BASE}, {**record, 'attempt': 2}))

    def test_l88_two_fixes_per_pr_survive_new_heads_and_intervening_reviews(self):
        repair = {**TASK, 'role': 'fix', 'cause': 'check'}
        record = dict(run=9, key=d.key(repair), attempt=1, phase='issued', fixes=1)
        with patch.object(d, 'api', return_value=dict(status='completed')), patch.object(d, 'started', return_value=True):
            self.assertEqual(d.fixes(record), 2)
            self.assertFalse(d.available({**repair, 'head': BASE}, record))
            self.assertFalse(d.available({**repair, 'cause': 'conflict', 'base': HEAD}, record))
        # A reviewer preserves the repair total, even if its own head/cause differs.
        reviewed = {**record, 'key': d.key(TASK), 'fixes': 2}
        with patch.object(d, 'api', return_value=dict(status='completed')), patch.object(d, 'started', return_value=True):
            self.assertFalse(d.available(repair, reviewed))

    def test_l88_main_movement_does_not_reset_review_retry_budget(self):
        self.assertEqual(d.key(TASK), d.key({**TASK, 'base': HEAD}))
        task = {**TASK, 'role': 'fix', 'cause': 'conflict'}
        self.assertNotEqual(d.key(task), d.key({**task, 'base': HEAD}))

    def test_l88_stale_or_duplicate_dispatch_never_invokes_model_prompt(self):
        for fresh in (None, {**TASK, 'head': BASE}, {**TASK, 'role': 'fix'}):
            with patch.object(d, 'due', return_value=fresh), patch.object(d, 'acquire') as acquire, patch.object(d, 'run') as run:
                d.start(1, 'review', HEAD)
                acquire.assert_not_called()
                run.assert_not_called()
        with patch.object(d, 'due', return_value=TASK), patch.object(d, 'acquire', return_value=False), patch.object(d, 'run') as run:
            d.start(1, 'review', HEAD)
            run.assert_not_called()

    def test_l88_changed_evidence_after_acquisition_does_no_work(self):
        with patch.object(d, 'due', side_effect=[TASK, None]), patch.object(d, 'acquire', return_value=True), patch.object(d, 'run') as run:
            d.start(1, 'review', HEAD)
            run.assert_not_called()

    def test_l88_two_failed_preflights_do_not_exhaust_model_attempts(self):
        record = dict(run=9, key=d.key(TASK), attempt=1, phase='claimed')
        for _ in range(3):
            with patch.object(d, 'api', return_value=dict(status='completed')):
                self.assertTrue(d.available(TASK, record))
                self.assertEqual(d.consumed(record), 0)
        record['phase'] = 'issued'
        with patch.object(d, 'api', return_value=dict(jobs=[dict(steps=[dict(name='Run anthropics/claude-code-action@v1', status='completed', conclusion='skipped')])])):
            self.assertEqual(d.consumed(record), 0)

    def test_l88_review_allows_the_existing_percy_wait(self):
        with tempfile.NamedTemporaryFile() as output, patch.dict(os.environ, GITHUB_OUTPUT=output.name), patch.object(d, 'due', return_value=TASK), patch.object(d, 'acquire', return_value=True), patch.object(d, 'issued'), patch.object(d, 'run', return_value='ready') as run:
            d.start(1, 'review', HEAD)
            self.assertEqual(run.call_args_list[0].kwargs['timeout'], 660)

    def test_l88_review_uses_the_real_shared_builder_prompt(self):
        original = d.run
        def execute(*args, **kwargs):
            if args[2] == 'review-due':
                return 'PR #1, head ' + HEAD + '.'
            return original(*args, **kwargs)
        with tempfile.NamedTemporaryFile() as output, patch.dict(os.environ, GITHUB_OUTPUT=output.name), patch.object(d, 'due', return_value=TASK), patch.object(d, 'acquire', return_value=True), patch.object(d, 'issued'), patch.object(d, 'run', side_effect=execute):
            d.start(1, 'review', HEAD)
            result = Path(output.name).read_text()
            self.assertIn((ROOT / '.agents/builder.md').read_text(), result)
            self.assertIn('Review PR #1, head ' + HEAD, result)
            self.assertIn('pr=1', result)
            self.assertIn('head=' + HEAD, result)
            delimiter = result.splitlines()[0].removeprefix('prompt<<')
            self.assertEqual(result.splitlines().count(delimiter), 1)

    def test_l88_initial_builder_and_local_builder_keep_ownership(self):
        for running, local in [({'U1'}, None), (set(), 'build/U1')]:
            with patch.object(d, 'api', return_value=PR), patch.object(d, 'active_builds', return_value=running), patch.object(d, 'local_branch', return_value=local), patch.object(d, 'run') as run:
                self.assertIsNone(d.due(1))
                run.assert_not_called()

    def test_l88_unknown_claim_write_is_observed_before_retry(self):
        error = subprocess.CalledProcessError(1, ['gh'])
        for observed, wanted in [('ours', True), ('theirs', False)]:
            with patch.object(d, 'claim_record', side_effect=[(None, None), (observed, {})]), patch.object(d, 'available', return_value=True), patch.object(d, 'run', return_value=BASE), patch.object(d, 'api', side_effect=[dict(sha='ours'), error]), patch.dict(os.environ, GITHUB_RUN_ID='42'):
                self.assertEqual(d.acquire(TASK), wanted)

    def test_l88_claim_update_uses_non_force_sibling_commit(self):
        record = dict(run=9, key=d.key(TASK), attempt=1)
        with patch.object(d, 'claim_record', return_value=('old', record)), patch.object(d, 'available', return_value=True), patch.object(d, 'consumed', return_value=1), patch.object(d, 'run', return_value=BASE), patch.object(d, 'api', return_value=dict(sha='new')) as api, patch.object(d, 'gh') as gh, patch.dict(os.environ, GITHUB_RUN_ID='42'):
            self.assertTrue(d.acquire(TASK))
            self.assertEqual(api.call_args.args[1]['parents'], ['old'])
            self.assertEqual(json.loads(api.call_args.args[1]['message'])['attempt'], 2)
            self.assertEqual(json.loads(gh.call_args.kwargs['data']), dict(sha='new', force=False))

    def test_l88_git_rejects_competing_sibling_claims(self):
        # Exercise the ownership primitive against a real bare remote, not a mocked lock.
        with tempfile.TemporaryDirectory() as path:
            def git(*args, data=None):
                return subprocess.run(['git', '-C', path, *args], input=data, text=True, capture_output=True, check=True).stdout.strip()
            git('init', '--bare')
            tree = git('mktree', data='')
            with patch.dict(os.environ, GIT_AUTHOR_NAME='test', GIT_AUTHOR_EMAIL='test@test', GIT_COMMITTER_NAME='test', GIT_COMMITTER_EMAIL='test@test'):
                old = git('commit-tree', tree, '-m', 'old')
                first = git('commit-tree', tree, '-p', old, '-m', 'first')
                second = git('commit-tree', tree, '-p', old, '-m', 'second')
            git('update-ref', 'refs/heads/loop-pr/1', old)
            git('push', path, first + ':refs/heads/loop-pr/1')
            with self.assertRaises(subprocess.CalledProcessError):
                git('push', path, second + ':refs/heads/loop-pr/1')
            self.assertEqual(git('rev-parse', 'refs/heads/loop-pr/1'), first)

    def test_l88_conflict_repair_is_limited_to_build_branches(self):
        other = {**PR, 'mergeable': False, 'head': {**PR['head'], 'ref': 'loop/x'}}
        self.assertIsNone(d.decision(other, [], [], ['code']))

    def test_l88_missed_event_dispatches_and_empty_queue_stops(self):
        with patch.object(d, 'active_builds', return_value=set()), patch.object(d, 'local_branch', return_value=None), patch.object(d, 'gh', return_value=[dict(number=1)]), patch.object(d, 'due', return_value=TASK), patch.object(d, 'claim_record', return_value=(None, None)), patch.object(d, 'available', return_value=True), patch.object(d, 'run', side_effect=['', '[]']) as run:
            d.reconcile()
            self.assertEqual(run.call_args_list[0].args, ('gh', 'workflow', 'run', 'review.yml', '-f', 'pr=1', '-f', 'head=' + HEAD))
            self.assertEqual(run.call_count, 2)

    def test_l88_claimed_pr_is_not_redispatched(self):
        with patch.object(d, 'active_builds', return_value=set()), patch.object(d, 'local_branch', return_value=None), patch.object(d, 'gh', return_value=[dict(number=1)]), patch.object(d, 'due', return_value=TASK), patch.object(d, 'claim_record', return_value=('old', {})), patch.object(d, 'available', return_value=False), patch.object(d, 'run', return_value='[]') as run:
            d.reconcile()
            run.assert_called_once_with('bash', 'loop/runs.sh', 'queue', '4')


class L89(unittest.TestCase):
    def test_l89_required_gate_rejects_failed_cancelled_and_skipped_lanes(self):
        text = (ROOT / '.github/workflows/check.yml').read_text()
        command = text.split('        run: test ')[1].strip()
        for rust in ('success', 'failure', 'cancelled', 'skipped'):
            for web in ('success', 'failure', 'cancelled', 'skipped'):
                result = subprocess.run(['bash', '-c', 'test ' + command], env={**os.environ, 'RUST': rust, 'WEB': web})
                self.assertEqual(result.returncode == 0, rust == web == 'success')

    def test_l89_local_and_ci_use_the_same_recipes(self):
        text = (ROOT / 'justfile').read_text()
        self.assertIn('check: check-rust check-web', text)
        workflows = (ROOT / '.github/workflows/check.yml').read_text()
        self.assertIn('run: just check-rust', workflows)
        self.assertIn('run: just check-web', workflows)
        self.assertIn('needs: [rust, web]', workflows)


if __name__ == '__main__':
    unittest.main()
