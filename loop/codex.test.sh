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
        self.row = dict(ids='U59', file='scenarios/ui.md', slug='U59')
        self.calls = []
        self.ready = True
        self.code = 0
        self.enabled = True
        self.rows = [self.row]
        self.claimed = True
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
        if args[:3] == ('git', 'status', '--porcelain'): return ''
        if args[:3] == ('git', 'rev-parse', 'HEAD'): return self.base
        if args[:3] == ('gh', 'run', 'list'): return json.dumps([dict(headSha=self.base, conclusion='success')])
        if args[:2] == ('loop/runs.sh', 'queue'): return json.dumps(self.rows)
        if args[:2] == ('loop/runs.sh', 'claim'): return json.dumps([self.row] if self.claimed else [])
        if args[:4] == ('git', 'remote', 'get-url', 'origin'): return 'https://github.com/example/repo'
        if args[:2] == ('git', 'clone'): Path(args[-1]).mkdir()
        if args[:3] == ('gh', 'pr', 'list'): return json.dumps([dict(number=1, isDraft=not self.ready, url='https://example/pr/1')])
        return ''

    def launch(self, args, **kw):
        self.argv, self.env = args, kw['env']
        kw['stdout'].write(json.dumps(dict(type='turn.completed', usage=dict(input_tokens=100, cached_input_tokens=60, output_tokens=20))) + '\n')
        kw['stdout'].flush()
        class Result:
            pid = 123456789
            def wait(inner): return self.code
        return Result()

    def test_L87_ready_pr_and_plan_login_are_observed(self):
        with patch.dict(os.environ, OPENAI_API_KEY='do-not-use', CODEX_API_KEY='do-not-use'):
            fleet.tick(self.state)
        result = json.loads((self.state/'last.json').read_text())
        self.assertEqual(result['state'], 'ready')
        self.assertEqual(result['usage'], dict(input=40, cache_read=60, output=20, turns=1))
        self.assertNotIn('CODEX_API_KEY', self.env)
        self.assertNotIn('OPENAI_API_KEY', self.env)
        self.assertEqual(self.argv[:4], ['gtimeout', '--kill-after=20s', '7200s', 'codex'])
        self.assertFalse((self.state/'active.json').exists())
        self.assertTrue(any(a[:2] == ('loop/runs.sh','unclaim') for a,k in self.calls))

    def test_L87_zero_exit_with_draft_is_failure_and_cools_down(self):
        self.ready = False
        with self.assertRaisesRegex(RuntimeError, 'did not deliver'): fleet.tick(self.state)
        before = len(self.calls)
        fleet.tick(self.state)
        self.assertFalse(any(a[:2] == ('loop/runs.sh','claim') for a,k in self.calls[before:]))

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
        fleet.save(self.state/'active.json',dict(pid=os.getpid()))
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
        with patch.object(fleet.os,'kill',side_effect=ProcessLookupError): fleet.tick(self.state)
        self.assertEqual(json.loads((self.state/'last.json').read_text())['exit'],130)
        self.assertFalse(any(a[:2] == ('loop/runs.sh','queue') for a,k in self.calls))

    def test_L87_public_status_excludes_local_paths(self):
        fleet.publish(dict(self.row,started=1,deadline=2,state='running',events='/private/log'))
        payload=json.loads(self.calls[0][0][-1])
        self.assertNotIn('events',payload)

class Reservation(unittest.TestCase):
    def test_L87_workflow_reserves_exactly_one_slot_before_claim(self):
        source=Path('.github/workflows/build.yml').read_text()
        start=source.index('          [[ $MAX =~')
        end=source.index('          echo "rows=',start)
        script=source[start:end]
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); (root/'loop').mkdir()
            fake=root/'loop/runs.sh'
            fake.write_text('#!/bin/sh\necho "$1 $2" >>trace\nif [ "$1" = queue ]; then echo "[]"; else cat; fi\n')
            fake.chmod(0o755)
            for enabled,expected in [('true','queue 3'),('false','queue 4')]:
                trace=root/'trace'
                trace.write_text('')
                subprocess.run(['bash','-eo','pipefail','-c',script],cwd=root,env=dict(os.environ,MAX='4',CODEX_ENABLED=enabled),check=True)
                self.assertIn(expected,trace.read_text().splitlines())

unittest.main()
PY
