#!/usr/bin/env python3
"""L29 retained explanation and L81 delivery observation regressions."""
import contextlib
import io
import json
import os
import re
import runpy
import sys
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import outcome


class OutcomeTests(unittest.TestCase):
    def test_l93_summary_uses_results_without_api_reads_or_prompt_output(self):
        workflow = (Path(__file__).resolve().parent.parent / '.github/workflows/summary.yml').read_text()
        block = workflow.split('        run: |\n', 1)[1]
        code = '\n'.join(line[10:] for line in block.splitlines())
        with tempfile.TemporaryDirectory() as directory:
            summary = Path(directory) / 'summary.md'
            with patch.dict(os.environ, GITHUB_STEP_SUMMARY=str(summary),
                            RESULTS=json.dumps(dict(due=dict(result='success', outputs=dict(pr='42', prompt='PRIVATE')),
                                                    review=dict(result='skipped'))),
                            PURPOSE='Review', NEXT='Repair', GITHUB_SERVER_URL='https://github.com',
                            GITHUB_REPOSITORY='o/r', GITHUB_RUN_ID='1', GITHUB_RUN_ATTEMPT='1', GITHUB_EVENT_NAME='workflow_dispatch'):
                exec(compile(code, 'summary-workflow', 'exec'), {})
            text = summary.read_text()
            self.assertIn('No agent work ran', text)
            self.assertIn('/pull/42', text)
            self.assertNotIn('PRIVATE', text)
            self.assertNotIn('uses:', workflow)
            self.assertIn('permissions: {}', workflow)

    def test_l93_command_report_keeps_failure_and_redacts_summary(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'summary.md'
            with patch.dict(os.environ, GITHUB_STEP_SUMMARY=str(path), GH_TOKEN='private-value'), contextlib.redirect_stdout(io.StringIO()) as output:
                code = outcome.report([sys.executable, '-c', 'import sys; print("blocked private-value"); sys.exit(7)'])
            self.assertEqual(code, 7)
            self.assertIn('blocked [redacted]', path.read_text())
            self.assertNotIn('private-value', path.read_text() + output.getvalue())

    def test_l93_every_workflow_explains_all_its_jobs_even_on_failure(self):
        root = Path(__file__).resolve().parent.parent / '.github/workflows'
        for workflow in root.glob('*.yml'):
            if workflow.name == 'summary.yml':
                continue
            jobs = workflow.read_text().split('\njobs:\n', 1)[1]
            names = re.findall(r'^  ([\w-]+):\s*$', jobs, re.M)
            summary = jobs.split('\n  summary:\n', 1)[1]
            self.assertIn('if: always()', summary, workflow.name)
            self.assertIn('uses: ./.github/workflows/summary.yml', summary, workflow.name)
            needs = re.search(r'needs: \[([^]]+)\]', summary).group(1).split(', ')
            self.assertEqual(set(needs), set(names) - {'summary'}, workflow.name)


    def test_f7_retention_excludes_auth_and_redacts_partial_captures(self):
        workflow = (Path(__file__).resolve().parent.parent / '.github/workflows/f7.yml').read_text()
        block = workflow.split('        shell: python\n        run: |\n', 1)[1].split('      - uses:', 1)[0]
        code = '\n'.join(line[10:] for line in block.splitlines())
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            private = root / 'f7/roundup-f7.test'
            (private / 'out').mkdir(parents=True)
            (private / 'auth-status.json').write_text('private authentication')
            (private / 'out/i_base.txt').write_text('oauth-value github-value ghp_example')
            (private / 'out/i_base.version.txt').write_text('2.1.295')
            (root / 'f7/run.log').write_text('oauth-value')
            with patch.dict(os.environ, RUNNER_TEMP=directory, GITHUB_RUN_ID='9', CLAUDE_CODE_OAUTH_TOKEN='oauth-value', GH_TOKEN='github-value'), contextlib.redirect_stdout(io.StringIO()):
                exec(compile(code, 'f7-retention', 'exec'), {})
            output = root / 'f7-evidence'
            self.assertEqual({p.name for p in output.iterdir()}, {'i_base.txt', 'i_base.version.txt', 'run.log', 'manifest.json'})
            self.assertEqual((output / 'i_base.txt').read_text(), '[redacted] [redacted] [redacted]')
            manifest = json.loads((output / 'manifest.json').read_text())
            self.assertEqual(len(manifest['missing']), 6)
            self.assertEqual(manifest['state'], 'captured-not-judged')

    def test_l29_retains_final_explanation_without_tool_output(self):
        report = outcome.model_outcome([
            {'type': 'user', 'content': 'private tool output'},
            {'type': 'result', 'subtype': 'success', 'result': 'Cannot access the PR.'}], {})
        self.assertEqual(report['text'], 'Cannot access the PR.')
        self.assertEqual(report['kind'], 'model-claim')
        self.assertNotIn('private', json.dumps(report))

    def test_l29_missing_output_is_not_success(self):
        self.assertEqual(outcome.model_outcome([], {})['result'], 'missing')
        self.assertEqual(outcome.model_outcome([{'type': 'result'}], {})['explanation'], 'unavailable')

    def test_l29_redacts_before_truncation(self):
        report = outcome.model_outcome([{'type': 'result', 'result': 'x' * 7990 + 'secret-value ghp_abcdef sk-ant-private'}], {'ACTION_GITHUB_TOKEN': 'secret-value'})
        self.assertTrue(report['truncated'])
        self.assertNotIn('secret', report['text'])
        self.assertEqual(outcome.redact('ghp_abcdef sk-ant-private github_pat_abcdef', {}), '[redacted] [redacted] [redacted]')

    def test_l29_all_configured_workflow_secrets_reach_trusted_redaction(self):
        root = Path(__file__).resolve().parents[1]
        for workflow in ('build', 'build-row', 'review', 'retro'):
            source = (root / '.github/workflows' / (workflow + '.yml')).read_text()
            self.assertNotIn('uses: ./.github/actions/ledger', source)
            self.assertNotIn('uses: saadshahd/roundup/', source)
            self.assertIn('ref: main\n          path: .loop-ledger\n          sparse-checkout:', source)
            self.assertIn('.github/actions/ledger\n            loop\n          persist-credentials: false', source)
            # Retain the complete indented step, rather than depending on input order.
            ledger = source.split('uses: ./.loop-ledger/.github/actions/ledger', 1)[1].split('\n  #', 1)[0].split('\n  post:', 1)[0]
            names = set(re.findall(r'secrets\.([A-Z_]+)', source))
            for name in names:
                self.assertIn('secrets.' + name, ledger, workflow + ': ' + name)
            self.assertIn('steps.claude.outputs.github_token', ledger)
        action = (root / '.github/actions/ledger/action.yml').read_text()
        for name in ('redact-token', 'redact-oauth', 'redact-percy', 'redact-typesafe'):
            self.assertIn('inputs.' + name, action)
        self.assertIn('GH_TOKEN: ${{ github.token }}', action)
        self.assertNotIn('git show origin/main:', action)
        self.assertIn('git -C "$GITHUB_ACTION_PATH" show HEAD:loop/outcome.py', action)
        report = outcome.model_outcome([{'type': 'result', 'result': 'opaque-percy opaque-typesafe'}],
                                      {'PERCY_TOKEN': 'opaque-percy', 'TYPESAFE_API_KEY': 'opaque-typesafe'})
        self.assertEqual(report['text'], '[redacted] [redacted]')

    def test_l29_subtype_cannot_smuggle_unredacted_text(self):
        report = outcome.model_outcome([{'type': 'result', 'subtype': 'secret-value'}], {})
        self.assertEqual(report['result'], 'unknown')

    def test_l29_malformed_output_fails(self):
        for events in ({}, [1], [{'type': 'result', 'result': {}}]):
            with self.assertRaises(ValueError):
                outcome.model_outcome(events, {})

    def test_l92_explain_prints_one_line_of_redacted_json_for_the_completion_job(self):
        with tempfile.TemporaryDirectory() as directory:
            events = Path(directory) / 'run.json'
            events.write_text(json.dumps([{'type': 'result', 'subtype': 'success', 'result': 'tok-1 done\nsecond line'}]))
            out = io.StringIO()
            with patch.dict(os.environ, CLAUDE_CODE_OAUTH_TOKEN='tok-1'), patch.object(sys, 'argv', ['outcome.py', 'explain', str(events)]), \
                 contextlib.redirect_stdout(out):
                runpy.run_path(str(Path(outcome.__file__)), run_name='__main__')
        self.assertEqual(out.getvalue().count('\n'), 1)
        self.assertEqual(json.loads(out.getvalue())['text'], '[redacted] done\nsecond line')

    def pr(self):
        return dict(number=330, state='OPEN', isDraft=True, headRefOid='a' * 40, url='https://github.com/o/r/pull/330')

    def test_l81_successful_model_does_not_make_a_draft_delivered(self):
        self.assertEqual(outcome.delivery([self.pr()])['state'], 'draft')

    def test_l81_ready_observation_is_not_approval_or_authorship(self):
        pr = self.pr()
        pr.update(isDraft=False, headRefOid='b' * 40)
        report = outcome.delivery([pr])
        self.assertEqual(report['state'], 'ready')
        self.assertEqual(report['headRefOid'], 'b' * 40)
        self.assertIn('independent review', report['message'])
        self.assertNotIn('author', report)

    def test_l81_missing_and_closed_unmerged_are_not_deliveries(self):
        self.assertEqual(outcome.delivery([])['state'], 'no-delivery')
        pr = self.pr()
        pr['state'] = 'CLOSED'
        self.assertEqual(outcome.delivery([pr])['state'], 'no-delivery')

    def test_l81_current_draft_beats_historical_merged_pr(self):
        merged = self.pr()
        merged['state'] = 'MERGED'
        self.assertEqual(outcome.delivery([merged, self.pr()])['state'], 'draft')

    def test_l81_repair_reads_exact_pr_while_builder_excludes_history(self):
        pr = self.pr()
        pr['state'] = 'MERGED'
        with patch.object(outcome, 'gh', return_value=pr) as api, contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(outcome.observe('build/F7-re-run', '330'), 0)
            self.assertEqual(api.call_args.args[:3], ('pr', 'view', '330'))
        with patch.object(outcome, 'gh', return_value=[]) as api, contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(outcome.observe('build/F7-re-run'), 1)
            self.assertEqual(api.call_args.args[4:6], ('--state', 'open'))

    def test_l81_observe_fails_draft_and_network_error_stays_error(self):
        with patch.object(outcome, 'gh', return_value=[self.pr()]), contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(outcome.observe('build/F7-re-run'), 1)
        with patch.object(outcome, 'gh', side_effect=subprocess.CalledProcessError(1, 'gh')):
            with self.assertRaises(subprocess.CalledProcessError):
                outcome.observe('build/F7-re-run')


if __name__ == '__main__':
    unittest.main()
