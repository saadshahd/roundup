#!/usr/bin/env python3
"""L2, L23, L24, L89: what the workflows grant each run, and the scripts they run before a model starts."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent


def action_inputs(workflow):
    """The `with:` block and env of a workflow's claude-code-action step."""
    source = (ROOT / f'.github/workflows/{workflow}.yml').read_text()
    return source.split('uses: anthropics/claude-code-action@v1', 1)[1].split('      - ', 1)[0]


def git(path, *args):
    return subprocess.run(['git', '-C', path, *args], text=True, capture_output=True, check=True).stdout.strip()


def new_repo(path):
    git(path, 'init')
    git(path, 'config', 'user.name', 'test')
    git(path, 'config', 'user.email', 'test@test')


class Permissions(unittest.TestCase):
    def test_l23_writing_runs_request_workflow_changes_and_read_only_ci_evidence(self):
        for workflow in ('build-issue', 'fix'):
            inputs = action_inputs(workflow)
            block = inputs.split('additional_permissions: |\n', 1)[1]
            granted = []
            for line in block.splitlines():
                if not line.startswith('            '):
                    break
                granted.append(line.strip())
            self.assertEqual(set(granted), {'actions: read', 'checks: read', 'workflows: write'}, workflow)
            self.assertNotIn('github_token:', inputs, workflow)

    def test_l24_the_review_run_keeps_the_read_only_workflow_token(self):
        inputs = action_inputs('review')
        self.assertIn('github_token: ${{ github.token }}', inputs)
        self.assertNotIn('additional_permissions:', inputs)
        source = (ROOT / '.github/workflows/review.yml').read_text()
        job = source.split('  review-run:', 1)[1].split('    steps:', 1)[0]
        self.assertNotIn(': write', job)


class Build(unittest.TestCase):
    def test_l23_new_work_issues_start_a_pick_without_an_intake_label(self):
        source = (ROOT / '.github/workflows/build.yml').read_text()
        self.assertIn('types: [opened, reopened, unlabeled]', source)
        pick = source.split('  pick:', 1)[1].split('    runs-on:', 1)[0]
        for text in ("github.event.label.name == 'flag:needs-user'", '["saadshahd", "claude[bot]", "github-actions[bot]"]'):
            self.assertIn(text, pick)
        for text in ('loop:work', 'COLLABORATOR', 'author_association'):
            self.assertNotIn(text, pick)
        self.assertNotIn('ready-for-agent', source)

    def test_l23_sixteen_build_jobs_at_most(self):
        source = (ROOT / '.github/workflows/build.yml').read_text()
        self.assertIn("default: '16'", source)
        self.assertIn("MAX: ${{ inputs.max || '16' }}", source)

    def test_l23_each_issue_whose_model_worked_starts_the_next_pick_when_its_own_build_ends(self):
        caller = (ROOT / '.github/workflows/build.yml').read_text()
        self.assertIn('uses: ./.github/workflows/build-issue.yml', caller)
        self.assertNotIn('  after:', caller)
        source = (ROOT / '.github/workflows/build-issue.yml').read_text()
        build = source.split('  build:', 1)[1].split('\n  after:', 1)[0]
        after = source.split('\n  after:', 1)[1]
        # The Builder's job holds no write; only `after`, which runs no model, dispatches.
        self.assertNotIn('actions: write', build)
        for grant in ('issues: write', 'pull-requests: write'):
            self.assertNotIn(grant, build)
        self.assertIn('needs: build', after)
        self.assertIn('actions: write', after)
        step = after.split('gh workflow run build.yml', 1)[0].rsplit('      - ', 1)[1]
        # Only a job whose model did work dispatches, so a usage limit ends the chain instead of spinning it.
        self.assertIn("env.RAN == 'true'", step)


class Stall(unittest.TestCase):
    def test_l94_workflow_calls_no_model_and_holds_only_issue_writes(self):
        source = (ROOT / '.github/workflows/stall.yml').read_text()
        for name in ('build.yml', 'claude-code-action', 'claude', 'anthropic'):
            self.assertNotIn(name, source.lower().replace('loop/stall.py', ''), name)
        block = source.split('permissions:\n', 1)[1].split('\n\n', 1)[0]
        self.assertEqual({line.strip() for line in block.splitlines()},
                         {'issues: write', 'contents: read', 'pull-requests: read', 'actions: read'})
        self.assertIn("cron: '41 * * * *'", source)
        self.assertNotIn(': write', source.split('jobs:', 1)[1])


class Scripts(unittest.TestCase):
    def test_l24_proposed_instructions_do_not_control_the_reviewer(self):
        with tempfile.TemporaryDirectory() as path:
            root = Path(path)
            new_repo(path)
            trusted = ['AGENTS.md', 'CLAUDE.md', '.claude/rule.md', '.agents/builder.md', 'loop/rules.sh']
            for name in trusted:
                target = root / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text('trusted')
            git(path, 'add', '.')
            git(path, 'commit', '-m', 'main')
            base = git(path, 'rev-parse', 'HEAD')
            for name in trusted + ['apps/AGENTS.md', '.claude/added.md']:
                target = root / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text('proposed')
            (root / 'product.txt').write_text('proposed product')
            git(path, 'add', '.')
            git(path, 'commit', '-m', 'PR')
            head = git(path, 'rev-parse', 'HEAD')
            subprocess.run(['bash', str(ROOT / 'loop/review-checkout.sh'), base], cwd=path, check=True)
            self.assertEqual(git(path, 'rev-parse', 'HEAD'), head)
            self.assertEqual((root / 'product.txt').read_text(), 'proposed product')
            for name in trusted:
                self.assertEqual((root / name).read_text(), 'trusted')
            self.assertFalse((root / 'apps/AGENTS.md').exists())
            self.assertFalse((root / '.claude/added.md').exists())

    def test_l2_builder_hook_stamps_commits_and_preserves_existing_attribution(self):
        with tempfile.TemporaryDirectory() as path:
            new_repo(path)
            subprocess.run(['bash', str(ROOT / 'loop/author.sh'), 'builder-test'], cwd=path,
                           env={**os.environ, 'RUNNER_TEMP': path}, check=True)
            git(path, 'commit', '--allow-empty', '-m', 'change')
            self.assertEqual(git(path, 'log', '-1', '--format=%(trailers:key=Author-Agent,valueonly)'), 'builder-test')
            git(path, 'commit', '--amend', '--allow-empty', '--no-edit')
            self.assertEqual(git(path, 'log', '-1', '--format=%(trailers:key=Author-Agent,valueonly)'), 'builder-test')
            git(path, 'commit', '--allow-empty', '-m', 'change', '-m', 'Author-Agent: another-author')
            self.assertEqual(git(path, 'log', '-1', '--format=%(trailers:key=Author-Agent,valueonly)'), 'another-author')


class L89(unittest.TestCase):
    def test_l89_required_gate_rejects_failed_cancelled_and_skipped_lanes(self):
        text = (ROOT / '.github/workflows/check.yml').read_text()
        command = text.split('        run: test ')[1].splitlines()[0].strip()
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
