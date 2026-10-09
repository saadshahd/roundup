#!/usr/bin/env python3
"""L92: the Trail of a work Issue."""
import json
import os
import subprocess
import sys
import unittest
from unittest.mock import patch

import trail

NOW = 1_800_000_000.0
HEAD = 'a' * 40


def stamp(seconds):
    from datetime import datetime, timezone
    return datetime.fromtimestamp(NOW + seconds, timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')


def comment(identifier, at, body, login='github-actions[bot]', association='NONE'):
    return dict(id=identifier, created_at=stamp(at), html_url=f'https://github.com/o/r/issues/1#c{identifier}',
                user=dict(login=login), author_association=association, body=body)


def end(run, at, cause='build', delivery='no-delivery', head='none', login='github-actions[bot]', association='NONE'):
    return comment(run, at, f'<!-- trail {run} 1 -->\nEntry: end\nRole: builder\nRun: u\nCause: {cause}\nDelivery: {delivery}\nHead: {head}\nResult: success\n\nModel claim:\n\nnothing to do',
                   login, association)


def read(items):
    with patch.object(trail, 'gh', return_value=[items]):
        return trail.comments(1)


def plan(items, now=NOW, edit=0):
    return trail.plan(read(items), now, lambda: edit)


class Trail(unittest.TestCase):
    def test_l92_untrusted_marker_ignored(self):
        items = [end(1, -50), end(2, -40, login='stranger'), end(3, -30, login='writer', association='COLLABORATOR'),
                 end(4, -20, login='claude[bot]')]
        self.assertEqual([e['run'] for e in trail.entries(read(items))], ['1', '3', '4'])

    def test_l92_repeat_diagnoses_then_waits_two_unchanged_ends_make_the_next_run_diagnostic(self):
        self.assertFalse(plan([end(1, -100)])['diagnostic'])
        result = plan([end(1, -200), end(2, -100)])
        self.assertTrue(result['diagnostic'])
        self.assertIsNone(result['until'])

    def test_l92_repeat_diagnoses_then_waits_durations_double_to_two_hours(self):
        for count, expected in ((3, 900), (4, 1800), (5, 3600), (6, 7200), (7, 7200)):
            # Fix causes isolate the doubling from the rolling limit of build runs.
            items = [end(n, -(count - n), cause='check') for n in range(1, count + 1)]
            result = plan(items)
            self.assertAlmostEqual(result['until'] - NOW, expected, delta=1, msg=count)

    def test_l92_a_new_head_or_a_new_cause_resets_the_streak(self):
        items = [end(1, -300), end(2, -200), end(3, -100, head=HEAD)]
        self.assertFalse(plan(items)['diagnostic'])
        items = [end(1, -300), end(2, -200), end(3, -100, cause='check')]
        self.assertEqual(plan(items)['streak'], 1)

    def test_l92_writer_edit_or_comment_lifts_wait(self):
        base = [end(n, -10 + n, cause='check') for n in (1, 2, 3)]
        self.assertIsNotNone(plan(base)['until'])
        self.assertIsNotNone(plan(base + [comment(9, -2, 'try again', 'claude[bot]')])['until'])
        self.assertIsNotNone(plan(base + [comment(9, -2, 'drive-by', 'stranger')])['until'])
        self.assertIsNone(plan(base + [comment(9, -2, 'look at X', 'owner', 'OWNER')])['until'])
        self.assertIsNone(plan(base, edit=NOW - 2)['until'])
        self.assertIsNotNone(plan(base, edit=NOW - 5000)['until'])

    def test_l92_rolling_limit(self):
        items = [end(n, -3600 * n, head=str(n) * 40) for n in range(1, 7)] + [comment(99, -10, 'go on', 'owner', 'OWNER')]
        result = plan(items)
        self.assertAlmostEqual(result['until'], NOW - 3600 * 6 + 86400, delta=1)
        self.assertIsNone(plan(items[:5] + items[6:])['until'])
        self.assertIsNone(plan(items, now=NOW + 86400)['until'])

    def test_l92_fix_runs_do_not_count_against_the_build_limit(self):
        items = [end(n, -3600 * n, cause='check', head=str(n) * 40) for n in range(1, 8)]
        self.assertIsNone(plan(items)['until'])

    def test_l92_task_holds_last_six_and_writer_comments(self):
        items = [end(n, -1000 + n, head=str(n) * 40) for n in range(1, 9)]
        items += [comment(50, -500, 'old advice', 'owner', 'OWNER'), comment(51, -1, 'new advice', 'owner', 'OWNER'),
                  comment(52, -1, 'bot chatter', 'claude[bot]')]
        items = [i for i in items if i['id'] != 50] + [comment(50, -2000, 'ancient advice', 'owner', 'OWNER')]
        with patch.object(trail, 'gh', return_value=[items]), patch.dict(os.environ, GITHUB_RUN_ID='77', GITHUB_RUN_ATTEMPT='2'):
            text = trail.section(1, NOW)
        self.assertNotIn('ancient advice', text)
        self.assertIn('new advice', text)
        self.assertNotIn('bot chatter', text)
        order = [text.index(f'#c{n}') for n in range(3, 9)]
        self.assertEqual(order, sorted(order))
        self.assertNotIn('#c2)', text)
        self.assertIn('<!-- trail 77 2 -->', text)
        self.assertNotIn('Diagnostic run', text)

    def test_l92_an_issue_without_entries_gets_no_trail(self):
        with patch.object(trail, 'gh', return_value=[[comment(1, -5, 'hello', 'owner', 'OWNER')]]):
            self.assertEqual(trail.section(1, NOW), '')

    def test_l92_read_failure_raises(self):
        with patch.object(trail, 'gh', side_effect=subprocess.CalledProcessError(1, 'gh')):
            with self.assertRaises(subprocess.SubprocessError):
                trail.comments(1)
        with patch.object(trail, 'gh', return_value=[[dict(id=1)]]):
            with self.assertRaises(ValueError):
                trail.comments(1)

    def test_l92_retro_lists_repeats(self):
        issues = [[dict(number=1, title='a'), dict(number=2, title='b'), dict(number=3, title='pr', pull_request={})]]
        trails = {1: [[end(1, -3), end(2, -2)]], 2: [[end(1, -3), end(2, -2, cause='check')]]}
        def fake(*args, **kw):
            if args[1].startswith('repos/{owner}/{repo}/issues?'):
                return issues
            return trails[int(args[1].split('/')[4])]
        with patch.object(trail, 'gh', side_effect=fake), patch.object(trail.time, 'time', return_value=NOW):
            rows = trail.repeats()
        self.assertEqual([(r['issue'], r['count']) for r in rows], [(1, 2)])

    def test_l92_linked_issue_is_read_from_the_pr_body(self):
        self.assertEqual(trail.linked_issue('x\nRefs #433\n'), 433)
        self.assertIsNone(trail.linked_issue('no link'))


class Cli(unittest.TestCase):
    def test_l92_read_failure_exits_4(self):
        import tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as bin:
            Path(bin, 'gh').write_text('#!/bin/sh\nexit 1\n')
            Path(bin, 'gh').chmod(0o755)
            for args in (['section', '1'], ['pr-section', '1'], ['repeats']):
                run = subprocess.run([sys.executable, '-I', trail.__file__, *args], env={'PATH': bin}, capture_output=True, text=True)
                self.assertEqual(run.returncode, 4, args)
                self.assertNotIn('Traceback', run.stderr)

    def test_l92_changed_before_commit_is_asked_in_the_task_with_the_run_marker(self):
        with patch.object(trail, 'gh', return_value=[[end(1, -5)]]), patch.dict(os.environ, GITHUB_RUN_ID='9', GITHUB_RUN_ATTEMPT='3'):
            text = trail.section(1, NOW)
        self.assertIn('Before your first commit', text)
        self.assertIn('Entry: changed', text)
        self.assertIn('<!-- trail 9 3 -->', text)


class End(unittest.TestCase):
    ENV = dict(GITHUB_RUN_ID='55', GITHUB_RUN_ATTEMPT='1', GITHUB_SERVER_URL='https://github.com', GITHUB_REPOSITORY='o/r')

    def run_end(self, existing, branch_head, result=None, delivery=None):
        posted = []
        def fake(*args, data=None):
            if args[1].endswith('/comments') and data:
                posted.append(json.loads(data)['body'])
                return None
            return [existing]
        import outcome
        with patch.object(trail, 'gh', side_effect=fake), patch.object(trail, 'branch_head', return_value=branch_head), \
             patch.object(outcome, 'observation', return_value=delivery or dict(state='draft', headRefOid=HEAD)), \
             patch.dict(os.environ, {**self.ENV, 'RESULT': json.dumps(result) if result else ''}):
            trail.end(1, 'builder', 'build', 'build/L92', 'b' * 40)
        return posted

    def test_l92_end_entry_after_cancel(self):
        [body] = self.run_end([], HEAD, dict(result='error_max_turns', text='ran out'))
        for line in ('<!-- trail 55 1 -->', 'Entry: end', 'Role: builder', 'Run: https://github.com/o/r/actions/runs/55',
                     'Cause: build', 'Delivery: draft', f'Head: {HEAD}', 'Result: error_max_turns', 'Changed: none', 'ran out'):
            self.assertIn(line, body)

    def test_l92_end_entry_of_a_run_that_left_nothing(self):
        [body] = self.run_end([], 'b' * 40, delivery=dict(state='no-delivery'))
        self.assertIn('Head: none', body)
        self.assertIn('Result: missing', body)
        self.assertNotIn('Changed: none', body)

    def test_l92_changed_before_commit_spares_changed_none(self):
        changed = comment(7, -5, '<!-- trail 55 1 -->\nEntry: changed\nChanged: new tool, saw it fail', 'claude[bot]')
        [body] = self.run_end([changed], HEAD)
        self.assertNotIn('Changed: none', body)

    def test_l92_end_entry_is_posted_once_per_run(self):
        self.assertEqual(self.run_end([end(55, -5)], HEAD), [])


if __name__ == '__main__':
    unittest.main()
