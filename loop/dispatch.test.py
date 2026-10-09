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
RULES = {**CHECK, 'name': 'rules', 'id': 2}
TASK = dict(pr=1, role='review', cause='review', head=HEAD, base=BASE, branch='build/U1')


class L88(unittest.TestCase):
    def test_l88_successful_repair_requests_review_without_a_new_commit(self):
        verdict = dict(current=True, verdict='reject', head=HEAD, at='2026-10-09T00:00:00Z', body='scope')
        request = d.review_request(HEAD, verdict)
        record = dict(run=9, key=['fix', 'reject', HEAD, ''], repair_verdict=request['verdict'])
        jobs = dict(jobs=[dict(steps=[dict(name='Run anthropics/claude-code-action@v1', conclusion='success')])])
        with patch.dict(os.environ, GITHUB_RUN_ID='9'), patch.object(d, 'claim_record', return_value=('old', record)), patch.object(d, 'api', side_effect=[jobs, PR]), patch.object(d, 'run', return_value=json.dumps(verdict)), patch.object(d, 'write_claim', return_value=True) as write:
            d.repaired(1)
            self.assertEqual(write.call_args.args[2]['review_after'], request)
        for change in ('failed', 'draft', 'new-verdict', 'lost-owner'):
            with self.subTest(change=change):
                changed = copy.deepcopy(jobs)
                if change == 'failed':
                    changed['jobs'][0]['steps'][0]['conclusion'] = 'failure'
                event = {**verdict, 'body': 'new finding'} if change == 'new-verdict' else verdict
                with patch.dict(os.environ, GITHUB_RUN_ID='10' if change == 'lost-owner' else '9'), patch.object(d, 'claim_record', return_value=('old', record)), patch.object(d, 'api', side_effect=[changed, {**PR, 'draft': change == 'draft'}]), patch.object(d, 'run', return_value=json.dumps(event)), patch.object(d, 'write_claim') as write:
                    if change == 'lost-owner':
                        with self.assertRaises(ValueError): d.repaired(1)
                    else:
                        d.repaired(1)
                    write.assert_not_called()

    def test_l88_review_request_survives_claim_but_not_new_head_or_reject(self):
        verdict = dict(current=True, verdict='reject', head=HEAD, at='2026-10-09T00:00:00Z', body='scope')
        request = d.review_request(HEAD, verdict)
        record = dict(run=9, key=['fix', 'reject', HEAD, ''], phase='issued', attempt=1, review_after=request)
        def read(path, payload=None):
            if path == 'pulls/1': return PR
            raise AssertionError(path)
        def command(*args, **kwargs):
            if args[2] == 'verdicts': return json.dumps(verdict)
            if args[2] == 'touches': return 'code'
            return 'crates/messages/src/tests/b_hop_tests.rs'
        with patch.object(d, 'api', side_effect=read), patch.object(d, 'run', side_effect=command), patch.object(d, 'gh', return_value=[dict(check_runs=[CHECK, RULES])]), patch.object(d, 'claim_record', return_value=('old', record)):
            task = d.due(1, busy=set())
            self.assertEqual(task['role'], 'review')
            self.assertEqual(task['review_after'], request)
            record['key'] = d.key(task)
            self.assertEqual(d.due(1, busy=set())['role'], 'review')
            with patch.object(d, 'gh', return_value=[dict(check_runs=[{**CHECK, 'status': 'in_progress'}, RULES])]):
                self.assertIsNone(d.due(1, busy=set()))
            verdict['body'] = 'new reject'
            self.assertEqual(d.due(1, busy=set())['role'], 'fix')
            verdict['body'] = 'scope'
            record['review_after'] = {**request, 'head': BASE}
            self.assertEqual(d.due(1, busy=set())['role'], 'fix')

    def test_l78_loop_changes_require_review_even_after_historical_rejects(self):
        self.assertEqual(d.decision(PR, [CHECK, RULES], [dict(current=False, verdict='reject')] * 2, ['loop']), ('review', 'review'))
        self.assertIsNone(d.decision(PR, [CHECK], [], ['loop']))

    def test_l24_proposed_instructions_do_not_control_the_reviewer(self):
        with tempfile.TemporaryDirectory() as path:
            root = Path(path)
            def git(*args):
                return subprocess.run(['git', '-C', path, *args], text=True, capture_output=True, check=True).stdout.strip()
            git('init')
            git('config', 'user.name', 'test')
            git('config', 'user.email', 'test@test')
            trusted = ['AGENTS.md', 'CLAUDE.md', '.claude/rule.md', '.agents/builder.md', 'loop/rules.sh']
            for name in trusted:
                target = root / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text('trusted')
            git('add', '.')
            git('commit', '-m', 'main')
            base = git('rev-parse', 'HEAD')
            for name in trusted + ['apps/AGENTS.md', '.claude/added.md']:
                target = root / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text('proposed')
            (root / 'product.txt').write_text('proposed product')
            git('add', '.')
            git('commit', '-m', 'PR')
            head = git('rev-parse', 'HEAD')
            subprocess.run(['bash', str(ROOT / 'loop/review-checkout.sh'), base], cwd=path, check=True)
            self.assertEqual(git('rev-parse', 'HEAD'), head)
            self.assertEqual((root / 'product.txt').read_text(), 'proposed product')
            for name in trusted:
                self.assertEqual((root / name).read_text(), 'trusted')
            self.assertFalse((root / 'apps/AGENTS.md').exists())
            self.assertFalse((root / '.claude/added.md').exists())

    def test_l90_planning_clean_merges_require_exact_head_review(self):
        inherited = dict(current=True, verdict='approve', head=BASE)
        self.assertEqual(d.decision(PR, [CHECK, RULES], [inherited], ['loop', 'planning']), ('review', 'review'))
        self.assertIsNone(d.decision(PR, [CHECK, RULES], [{**inherited, 'head': HEAD}], ['loop', 'planning']))

    def test_l90_plan_repair_never_runs_the_generic_write_capable_builder(self):
        task = {**TASK, 'role': 'fix', 'cause': 'reject', 'planning': True}
        with patch.object(d, 'due', return_value=task), patch.object(d, 'acquire') as acquire, patch.object(d, 'run') as run:
            d.start(1, 'fix', HEAD)
            acquire.assert_not_called()
            run.assert_called_once_with('gh', 'workflow', 'run', 'feedback.yml', '-f', 'pr=1', '-f', 'head=' + HEAD)

    def test_l88_review_requires_successful_current_head_check(self):
        self.assertEqual(d.decision(PR, [CHECK, RULES], [], ['code']), ('review', 'review'))
        for change in [dict(head_sha=BASE), dict(status='in_progress'), dict(conclusion='skipped'), dict(app=dict(slug='other'))]:
            self.assertIsNone(d.decision(PR, [{**CHECK, **change}], [], ['code']))
        self.assertIsNone(d.decision(PR, [CHECK, RULES], [dict(current=True, verdict='approve')], ['code']))
        self.assertIsNone(d.decision(PR, [CHECK, RULES], [], ['docs']))

    def test_l88_rules_failures_are_repaired_before_review(self):
        self.assertIsNone(d.decision(PR, [CHECK], [], ['code']))
        self.assertEqual(d.decision(PR, [CHECK, {**RULES, 'conclusion': 'failure'}], [], ['code']), ('fix', 'rules'))
        self.assertIsNone(d.decision(PR, [CHECK, RULES], [], ['code', 'ui']))
        percy = {**CHECK, 'name': 'percy', 'id': 3}
        self.assertEqual(d.decision(PR, [CHECK, RULES, percy], [], ['code', 'ui']), ('review', 'review'))

    def test_l2_builder_hook_stamps_commits_and_preserves_existing_attribution(self):
        with tempfile.TemporaryDirectory() as path:
            def git(*args):
                return subprocess.run(['git', '-C', path, *args], text=True, capture_output=True, check=True).stdout.strip()
            git('init')
            git('config', 'user.name', 'test')
            git('config', 'user.email', 'test@test')
            subprocess.run(['bash', str(ROOT / 'loop/author.sh'), 'builder-test'], cwd=path, env={**os.environ, 'RUNNER_TEMP': path}, check=True)
            git('commit', '--allow-empty', '-m', 'change')
            self.assertEqual(git('log', '-1', '--format=%(trailers:key=Author-Agent,valueonly)'), 'builder-test')
            git('commit', '--amend', '--allow-empty', '--no-edit')
            self.assertEqual(git('log', '-1', '--format=%(trailers:key=Author-Agent,valueonly)'), 'builder-test')
            git('commit', '--allow-empty', '-m', 'change', '-m', 'Author-Agent: another-author')
            self.assertEqual(git('log', '-1', '--format=%(trailers:key=Author-Agent,valueonly)'), 'another-author')

    def test_l88_conflicts_and_red_checks_get_repair(self):
        self.assertEqual(d.decision({**PR, 'mergeable': False}, [], [], ['code']), ('fix', 'conflict'))
        self.assertEqual(d.decision(PR, [{**CHECK, 'conclusion': 'failure'}], [], ['code']), ('fix', 'check'))
        self.assertEqual(d.decision(PR, [], [dict(current=True, verdict='reject')], ['code']), ('fix', 'reject'))
        self.assertIsNone(d.decision(PR, [], [dict(current=False, verdict='reject')], ['code']))

    def test_l88_latest_check_wins_and_skipped_is_not_a_result(self):
        self.assertIsNone(d.decision(PR, [CHECK, RULES, {**CHECK, 'id': 3, 'status': 'in_progress'}], [], ['code']))
        self.assertEqual(d.decision(PR, [CHECK, RULES, {**CHECK, 'id': 3, 'conclusion': 'skipped'}], [], ['code']), ('review', 'review'))

    def test_l88_explicit_questions_and_second_reject_keep_gates(self):
        self.assertIsNone(d.decision({**PR, 'draft': True, 'body': 'Stopped: choose semantics'}, [CHECK], [], ['code']))
        self.assertEqual(d.decision({**PR, 'draft': True, 'body': 'Stopped: the run ended without marking it ready, url'}, [], [], ['code']), ('fix', 'cutoff'))
        self.assertEqual(('fix', 'conflict'), d.decision({**PR, 'mergeable': False}, [], [dict(current=True, verdict='reject')] * 2, ['code']))
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

    def test_l82_two_repairs_escalate_without_abandoning_changed_or_unchanged_heads(self):
        repair = {**TASK, 'role': 'fix', 'cause': 'check'}
        record = dict(run=9, key=d.key(repair), attempt=2, phase='issued', fixes=1)
        owner = dict(status='completed', conclusion='success', updated_at='2026-10-09T00:00:00Z')
        with patch.object(d, 'api', return_value=owner), patch.object(d, 'started', return_value=True):
            self.assertEqual(d.fixes(record), 2)
            self.assertTrue(d.available(repair, record))
            self.assertTrue(d.available({**repair, 'head': BASE}, record))
            reviewed = {**record, 'key': d.key(TASK), 'fixes': 2}
            self.assertTrue(d.available(repair, reviewed))

    def test_l82_rolling_budget_survives_new_heads_and_expires_without_user_reset(self):
        task = {**TASK, 'role': 'fix', 'cause': 'check'}
        history = [dict(run=n, head=HEAD, cause='check', at=100 + n) for n in range(6)]
        self.assertEqual(d.repair_wait(task, history, 1000), 86500)
        self.assertEqual(d.repair_wait({**task, 'head': BASE}, history, 1000), 86500)
        self.assertEqual(d.repair_wait(task, history, 90000), 90000)
        self.assertEqual(d.repair_wait(task, history[:2], 200), 200)
        self.assertEqual(d.repair_wait(task, history[:3], 200), 1002)
        self.assertEqual(d.repair_wait({**task, 'head': BASE}, history[:3], 200), 200)

    def test_l82_cancelled_ci_is_not_a_code_repair(self):
        cancelled = {**CHECK, 'name': 'rust', 'id': 3, 'conclusion': 'cancelled'}
        failed = {**CHECK, 'conclusion': 'failure'}
        self.assertEqual(d.decision(PR, [failed, cancelled], [], ['code']), ('ci', 'cancelled-ci'))
        web = {**cancelled, 'name': 'web', 'id': 4, 'conclusion': 'failure'}
        self.assertEqual(d.decision(PR, [failed, cancelled, web], [], ['code']), ('fix', 'check'))
        self.assertIsNone(d.decision(PR, [failed, {**cancelled, 'status': 'in_progress'}], [], ['code']))
        self.assertEqual(d.decision(PR, [failed, cancelled, {**cancelled, 'id': 5, 'conclusion': 'failure'}], [], ['code']), ('fix', 'check'))

    def test_l82_reviews_and_ci_recovery_preserve_repair_history(self):
        history = [dict(run=8, head=HEAD, cause='check', at=100)]
        owner = dict(status='completed', conclusion='success', updated_at='2026-10-09T00:00:00Z')
        record = dict(run=9, key=d.key(TASK), attempt=1, phase='issued', fixes=1, repairs=history)
        with patch.object(d, 'started', return_value=True):
            self.assertEqual(d.repair_history(record, owner), history)
            repaired = {**record, 'key': ['fix', 'check', HEAD, '']}
            self.assertEqual(len(d.repair_history(repaired, owner)), 2)
        with patch.object(d, 'started', return_value=False):
            self.assertEqual(d.repair_history(repaired, owner), history)
        with patch.object(d, 'api', return_value=owner):
            self.assertFalse(d.available({**TASK, 'role': 'ci'}, {**record, 'ci_head': HEAD, 'ci_attempts': 2}))

    def test_l82_repeated_setup_failure_waits_even_after_head_changes(self):
        record = dict(run=9, key=d.key(TASK), attempt=1, phase='claimed')
        owner = dict(status='completed', conclusion='failure', updated_at='2026-10-09T00:00:00Z')
        with patch.object(d, 'api', return_value=owner), patch.object(d.time, 'time', return_value=1791504001):
            self.assertFalse(d.available({**TASK, 'head': BASE}, record))

    def test_l82_second_ci_reservation_runs_before_escalating(self):
        task = {**TASK, 'role': 'ci', 'cause': 'cancelled-ci', 'ci_run': 12}
        record = dict(run=9, key=d.key(task), attempt=1, phase='issued', fixes=0, repairs=[], ci_head=HEAD, ci_attempts=1)
        owner = dict(status='completed', conclusion='success', updated_at='2026-10-09T00:00:00Z')
        check = {**CHECK, 'conclusion': 'failure', 'details_url': 'https://github.com/o/r/actions/runs/12/job/1'}
        cancelled = {**CHECK, 'name': 'rust', 'id': 3, 'conclusion': 'cancelled'}
        commands = []
        def api(path, payload=None):
            if path == 'pulls/1':
                return PR
            if path.startswith('actions/runs/'):
                return owner
            raise AssertionError(path)
        def gh(*args, **kwargs):
            if args[:2] == ('pr', 'list'):
                return [dict(number=1)]
            return [dict(check_runs=[check, cancelled])]
        def run(*args, **kwargs):
            commands.append(args)
            if args[0:3] == ('bash', 'loop/runs.sh', 'queue'):
                return '[]'
            if args[0:3] == ('bash', 'loop/rules.sh', 'touches'):
                return 'code'
            return ''
        def write(number, old, replacement):
            record.clear(); record.update(replacement)
            return True
        with patch.object(d, 'api', side_effect=api), patch.object(d, 'gh', side_effect=gh), patch.object(d, 'run', side_effect=run), patch.object(d, 'claim_record', side_effect=lambda n: ('old', dict(record))), patch.object(d, 'write_claim', side_effect=write), patch.object(d, 'active_builds', return_value=set()), patch.object(d, 'local_branch', return_value=None), patch.dict(os.environ, GITHUB_RUN_ID='42'):
            d.reconcile()
            self.assertIn(('gh', 'run', 'rerun', '12'), commands)
            self.assertEqual(record['ci_attempts'], 2)
            self.assertEqual(record['phase'], 'issued')
            self.assertEqual(d.due(1)['cause'], 'ci')
            self.assertEqual(record['fixes'], 0)

    def test_l82_diagnostic_prompt_includes_prior_evidence_and_changed_approach(self):
        task = {**TASK, 'role': 'fix', 'cause': 'check'}
        record = dict(fixes=2, repairs=[dict(run=8), dict(run=9)])
        with tempfile.NamedTemporaryFile() as output, patch.dict(os.environ, GITHUB_OUTPUT=output.name, GITHUB_RUN_ID='42'), patch.object(d, 'due', return_value=task), patch.object(d, 'acquire', return_value=True), patch.object(d, 'issued'), patch.object(d, 'claim_record', return_value=('old', record)), patch.object(d, 'run') as run, patch.object(d.trail, 'pr_section', return_value='\n## Trail of #7') as section:
            d.start(1, 'fix', HEAD)
            prompt = run.call_args.kwargs['data']
            section.assert_called_once_with(1)
            self.assertIn('earlier repair runs 8, 9', prompt)
            self.assertIn('changed approach', prompt)
            self.assertIn('## Trail of #7', prompt)
            self.assertNotIn('outcome.json', prompt)

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
        with patch.object(d, 'active_builds', return_value=set()), patch.object(d, 'local_branch', return_value=None), patch.object(d, 'gh', return_value=[dict(number=1)]), patch.object(d, 'due', return_value=TASK), patch.object(d, 'claim_record', return_value=(None, None)), patch.object(d, 'available', return_value=True), patch.object(d, 'run', side_effect=['', '', '[]']) as run:
            d.reconcile()
            self.assertEqual(run.call_args_list[1].args, ('gh', 'workflow', 'run', 'review.yml', '-f', 'pr=1', '-f', 'head=' + HEAD))
            self.assertEqual(run.call_count, 3)

    def test_l88_claimed_pr_is_not_redispatched(self):
        with patch.object(d, 'active_builds', return_value=set()), patch.object(d, 'local_branch', return_value=None), patch.object(d, 'gh', return_value=[dict(number=1)]), patch.object(d, 'due', return_value=TASK), patch.object(d, 'claim_record', return_value=('old', {})), patch.object(d, 'available', return_value=False), patch.object(d, 'run', return_value='[]') as run:
            d.reconcile()
            self.assertEqual([c.args for c in run.call_args_list], [('bash', 'loop/runs.sh', 'recover'), ('bash', 'loop/runs.sh', 'queue', '4')])

    def test_l88_failed_owner_recovery_cannot_dispatch_new_work(self):
        with patch.object(d, 'run', side_effect=subprocess.CalledProcessError(4, ['recover'])), patch.object(d, 'gh') as gh:
            with self.assertRaises(subprocess.CalledProcessError):
                d.reconcile()
            gh.assert_not_called()


class PublicationPermissions(unittest.TestCase):
    def test_l23_writing_roles_request_workflow_changes_and_read_only_ci_evidence(self):
        for name in ('build', 'build-row', 'retro'):
            source = (ROOT / f'.github/workflows/{name}.yml').read_text()
            action = source.split('uses: anthropics/claude-code-action@v1', 1)[1]
            inputs = action.split('      - ', 1)[0]
            block = inputs.split('additional_permissions: |\n', 1)[1]
            granted = []
            for line in block.splitlines():
                if not line.startswith('            '):
                    break
                granted.append(line.strip())
            self.assertEqual(set(granted), {'actions: read', 'checks: read', 'workflows: write'}, name)
            self.assertNotIn('github_token:', inputs, name)

    def test_l24_reviewer_keeps_the_read_only_workflow_token(self):
        source = (ROOT / '.github/workflows/review.yml').read_text()
        action = source.split('uses: anthropics/claude-code-action@v1', 1)[1].split('      - ', 1)[0]
        self.assertIn('github_token: ${{ github.token }}', action)
        self.assertNotIn('additional_permissions:', action)
        job = source.split('  review:', 1)[1].split('    steps:', 1)[0]
        self.assertNotIn(': write', job)


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
