#!/usr/bin/env bash
# L87: exercise the local Builder at its external boundaries; no account or network required.
set -euo pipefail
cd "$(dirname "$0")/.."
python3 -B - <<'PY'
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('fleet', 'loop/codex.py')
fleet = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fleet)

class Codex(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.state = Path(self.temp.name)
        self.row = dict(issue=42, ids='U59', file='scenarios/ui.md', slug='U59')
        self.calls = []
        self.ready = True
        self.code = 0
        self.enabled = True
        self.rows = [self.row]
        self.claimed = True
        self.dirty = True
        self.base = 'a' * 40
        self.patches = [patch.object(fleet, 'command', side_effect=self.command),
                        patch.object(fleet.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, '', 'Logged in using ChatGPT')),
                        patch.object(fleet.subprocess, 'Popen', side_effect=self.launch)]
        for p in self.patches: p.start()
        self.addCleanup(self.temp.cleanup)
        self.addCleanup(patch.stopall)

    def command(self, *args, **kw):
        self.calls.append((args, kw))
        if args[:3] == ('gh', 'variable', 'get'): return 'true' if self.enabled else 'false'
        if args[:3] == ('git', 'status', '--porcelain'): return ' M edited' if kw.get('cwd') and self.dirty else ''
        if args[:3] == ('git', 'rev-parse', 'HEAD'): return self.base
        if args[:3] == ('gh', 'run', 'list'): return json.dumps([dict(headSha=self.base, conclusion='success')])
        if args[:2] == ('loop/runs.sh', 'queue'): return json.dumps(self.rows)
        if args[:2] == ('loop/runs.sh', 'claim'): return json.dumps([self.row] if self.claimed else [])
        if args[:4] == ('git', 'remote', 'get-url', 'origin'): return 'https://github.com/example/repo'
        if args[:2] == ('git', 'ls-remote'): return self.base + '\trefs/heads/build/U59'
        if args[:2] == ('git', 'clone'):
            checkout = Path(args[-1]); checkout.mkdir()
            (checkout/'scenarios').mkdir()
            (checkout/self.row['file']).write_text('# Scenario\n')
        if args[:3] == ('gh', 'pr', 'create'): return 'https://example/pr/1'
        if args[:3] == ('gh', 'pr', 'list'): return json.dumps([dict(number=1, isDraft=not self.ready, url='https://example/pr/1')])
        return ''

    def launch(self, args, **kw):
        self.argv, self.env = args, kw['env']
        Path(args[args.index('-o')+1]).write_text(json.dumps(dict(status='ready' if self.ready else 'stopped',title='implement the row',body='Changes implemented.' if self.ready else 'Stopped: unresolved product question.')))
        kw['stdout'].write(json.dumps(dict(type='turn.completed', usage=dict(input_tokens=100, cached_input_tokens=60, output_tokens=20))) + '\n')
        kw['stdout'].flush()
        class Result:
            pid = 123456789
            def wait(inner): return self.code
        return Result()

    def test_L87_ready_pr_and_plan_login_are_observed(self):
        with patch.dict(os.environ, OPENAI_API_KEY='do-not-use', CODEX_API_KEY='do-not-use', GH_TOKEN='do-not-use'):
            fleet.tick(self.state)
        result = json.loads((self.state/'last.json').read_text())
        self.assertEqual(result['state'], 'ready')
        self.assertTrue(any(a == ('loop/runs.sh','queue','5','codex') for a,k in self.calls))
        self.assertEqual(result['usage'], dict(input=40, cache_read=60, output=20, turns=1))
        self.assertEqual(self.env['NEXTEST_TEST_THREADS'], '1')
        self.assertNotIn('CODEX_API_KEY', self.env)
        self.assertNotIn('OPENAI_API_KEY', self.env)
        self.assertNotIn('GH_TOKEN', self.env)
        self.assertIn('--ignore-rules', self.argv)
        self.assertNotIn(str(self.state/'checkout/.git'), self.argv)
        self.assertTrue(any(a[:3] == ('gh','pr','create') for a,k in self.calls))
        self.assertEqual(self.argv[:4], ['gtimeout', '--kill-after=20s', '7200s', 'codex'])
        self.assertFalse((self.state/'active.json').exists())
        self.assertTrue(any(a[:2] == ('loop/runs.sh','unclaim') for a,k in self.calls))

    def test_L87_zero_exit_with_draft_is_failure_and_cools_down(self):
        self.ready = False
        with self.assertRaisesRegex(RuntimeError, 'did not deliver'): fleet.tick(self.state)
        self.assertTrue(any(a == ('gh','workflow','run','merge-ready.yml','-f','pr=1') for a,k in self.calls))
        self.assertNotIn('Stopped: the run ended without marking it ready,', next(self.state.glob('*/pr.md')).read_text())
        before = len(self.calls)
        fleet.tick(self.state)
        self.assertFalse(any(a[:2] == ('loop/runs.sh','claim') for a,k in self.calls[before:]))

    def test_L87_question_before_any_edits_updates_the_issue_without_changing_specs(self):
        self.ready, self.dirty = False, False
        with self.assertRaisesRegex(RuntimeError, 'did not deliver'): fleet.tick(self.state)
        scenario = next(self.state.glob('*/checkout/scenarios/ui.md')).read_text()
        self.assertEqual(scenario, '# Scenario\n')
        self.assertTrue(any(a[:4] == ('gh','issue','comment','42') for a,k in self.calls))
        self.assertTrue(any(a[:4] == ('gh','issue','edit','42') for a,k in self.calls))
        self.assertFalse(any(a[:3] == ('gh','pr','create') for a,k in self.calls))

    def test_L87_failed_engine_with_ready_pr_is_failure(self):
        self.code = 124
        with self.assertRaises(RuntimeError): fleet.tick(self.state)
        self.assertEqual(json.loads((self.state/'last.json').read_text())['exit'],124)

    def test_L87_disabled_full_queue_or_lost_claim_never_launches(self):
        for mode in ('disabled','full','lost'):
            self.enabled, self.rows, self.claimed = mode != 'disabled', [] if mode == 'full' else [self.row], mode != 'lost'
            with patch.object(fleet.subprocess,'Popen') as launch:
                fleet.tick(self.state)
                launch.assert_not_called()

    def test_L87_api_auth_is_rejected_before_claim(self):
        with patch.object(fleet.subprocess,'run',return_value=subprocess.CompletedProcess([],0,'API key','')):
            with self.assertRaisesRegex(RuntimeError,'ChatGPT login'): fleet.tick(self.state)
        self.assertFalse(any(a[:2] == ('loop/runs.sh','claim') for a,k in self.calls))

    def test_L87_two_failures_on_same_base_skip_row(self):
        for i in range(2):
            attempt=self.state/str(i); attempt.mkdir()
            fleet.save(attempt/'record.json',dict(base=self.base,slug='U59',state='failed'))
        fleet.tick(self.state)
        self.assertFalse(any(a[:2] == ('loop/runs.sh','claim') for a,k in self.calls))

    def test_L87_live_orphan_holds_the_slot(self):
        fleet.save(self.state/'active.json',dict(pid=os.getpid(),birth='today',deadline=10**12))
        with patch.object(fleet.subprocess,'run',return_value=subprocess.CompletedProcess([],0,'today','')):
            fleet.tick(self.state)
        self.assertEqual(self.calls,[])

    def test_L87_network_failure_cannot_be_idle(self):
        with patch.object(fleet,'command',side_effect=subprocess.CalledProcessError(1,['gh'])):
            with self.assertRaises(subprocess.CalledProcessError): fleet.tick(self.state)

    def test_L87_interrupted_output_keeps_completed_usage(self):
        events = self.state/'events'
        events.write_text(json.dumps(dict(type='turn.completed',usage=dict(input_tokens=7,output_tokens=3)))+'\n{"type":')
        self.assertEqual(fleet.usage(events),dict(input=7,cache_read=0,output=3,turns=1))

    def test_L87_dead_orphan_is_finished_before_new_work(self):
        attempt=self.state/'old'; attempt.mkdir()
        record=dict(self.row,base=self.base,events=str(attempt/'events'),pid=123456789)
        fleet.save(self.state/'active.json',record)
        with patch.object(fleet.subprocess,'run',return_value=subprocess.CompletedProcess([],1,'','')): fleet.tick(self.state)
        self.assertEqual(json.loads((self.state/'last.json').read_text())['exit'],130)
        self.assertFalse(any(a[:2] == ('loop/runs.sh','queue') for a,k in self.calls))
        self.assertTrue(any(a == ('gh','workflow','run','reconcile.yml') for a,k in self.calls))

    def test_L87_timeout_delivers_partial_work_to_claude_fix(self):
        self.code=124
        with self.assertRaises(RuntimeError): fleet.tick(self.state)
        body = next(self.state.glob('*/pr.md')).read_text()
        self.assertIn('Stopped: the run ended without marking it ready,', body)
        publish = next(i for i,(a,k) in enumerate(self.calls) if a[:3] == ('gh','variable','set') and '"failed"' in a[-1])
        handoff = next(i for i,(a,k) in enumerate(self.calls) if a == ('gh','workflow','run','reconcile.yml'))
        self.assertLess(publish, handoff)

    def test_L87_reused_pid_does_not_hold_the_slot(self):
        attempt=self.state/'old'; attempt.mkdir()
        record=dict(self.row,base=self.base,events=str(attempt/'events'),pid=1,birth='old',deadline=10**12)
        fleet.save(self.state/'active.json',record)
        with patch.object(fleet.subprocess,'run',return_value=subprocess.CompletedProcess([],0,'new','')):
            fleet.tick(self.state)
        self.assertFalse((self.state/'active.json').exists())

    def test_L87_public_status_excludes_local_paths(self):
        fleet.publish(dict(self.row,started=1,deadline=2,state='running',events='/private/log'))
        payload=json.loads(self.calls[0][0][-1])
        self.assertNotIn('events',payload)

unittest.main()
PY
