#!/usr/bin/env python3
import importlib.util
import unittest
import subprocess
import io
from contextlib import redirect_stdout
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('orders', 'loop/orders.py')
orders = importlib.util.module_from_spec(spec)
spec.loader.exec_module(orders)


def issue(number=1, key='U83', **changes):
    return dict(dict(number=number, title='Explain missing prior output', html_url=f'https://github.com/a/b/issues/{number}',
                     state='open', labels=[], author_association='OWNER', user=dict(login='saadshahd'),
                     body=f'Scenarios: U83\nSpecification: [scenarios/ui.md](https://github.com/a/b/blob/main/scenarios/ui.md)\nKey: {key}\nPriority: 2\n'), **changes)


class Orders(unittest.TestCase):
    def test_l34_only_known_modes_are_executable(self):
        row=orders.classify([issue(body=issue()['body']+'Mode: skip-review\n')],{},[])[0]
        self.assertEqual(row['state'],'blocked')
        self.assertEqual(orders.order(issue(body=issue()['body']+'Mode: specify\n'))['mode'],'specify')

    def test_l34_task_renders_the_authorized_issue_and_mode(self):
        output = io.StringIO()
        with patch.object(orders, 'gh', return_value=issue()), \
             patch.object(orders, 'read_orders', return_value=orders.classify([issue()], {}, [])), \
             redirect_stdout(output):
            orders.task(1)
        self.assertIn('Mode: implement', output.getvalue())
        self.assertIn('Refs #1', output.getvalue())

    def test_l34_task_rechecks_new_pr_ownership_before_starting(self):
        with patch.object(orders, 'gh', return_value=issue()), \
             patch.object(orders, 'read_orders', return_value=[dict(issue=1,state='in-flight')]):
            with self.assertRaisesRegex(orders.NotReady, 'no longer ready'):
                orders.task(1)

    def test_l34_an_issue_that_is_no_longer_ready_exits_3_and_starts_no_run(self):
        with patch.object(orders, 'task', side_effect=orders.NotReady('taken')), \
             patch.object(orders.sys, 'argv', ['orders.py', 'task', '1']):
            with self.assertRaises(SystemExit) as stop:
                orders.main()
        self.assertEqual(stop.exception.code, 3)

    def test_l34_specification_delivery_can_release_implementation(self):
        specification = issue(body=issue()['body']+'Mode: specify\n')
        output = io.StringIO()
        with patch.object(orders, 'gh', return_value=specification), \
             patch.object(orders, 'read_orders', return_value=orders.classify([specification], {}, [])), \
             redirect_stdout(output):
            orders.task(1)
        self.assertIn('Close this specification Issue', output.getvalue())
        self.assertNotIn('specification-only PR does not close', output.getvalue())

    def test_l34_ready_issue_does_not_consult_test_names_or_work_tables(self):
        self.assertEqual(orders.classify([issue()], {}, [])[0]['state'], 'ready')

    def test_l34_every_open_issue_a_collaborator_or_loop_app_filed_is_work(self):
        for author, login in [('MEMBER', 'someone'), ('COLLABORATOR', 'someone'), ('CONTRIBUTOR', 'claude[bot]'),
                              ('NONE', 'github-actions[bot]')]:
            with self.subTest(author=author, login=login):
                row = orders.classify([issue(author_association=author, user=dict(login=login))], {}, [])[0]
                self.assertEqual(row['state'], 'ready')

    def test_l34_an_outside_issue_runs_only_once_a_collaborator_labels_it(self):
        outside = issue(author_association='NONE', user=dict(login='stranger'))
        self.assertEqual(orders.classify([outside], {}, [])[0]['state'], 'unspecified')
        self.assertEqual(orders.classify([dict(outside, labels=[{'name': 'loop:work'}])], {}, [])[0]['state'], 'ready')
        self.assertEqual([r['state'] for r in orders.classify([issue(), dict(outside, number=2)], {}, [])],
                         ['ready', 'unspecified'])
        with patch.object(orders, 'gh', return_value=outside):
            with self.assertRaisesRegex(orders.NotReady, 'no longer authorized'):
                orders.task(1)

    def test_l79_an_issue_waiting_on_the_user_is_held_except_a_stall(self):
        held = [{'name': 'flag:needs-user'}]
        row = orders.classify([issue(labels=held)], {}, [])[0]
        self.assertEqual((row['state'], row['reason']), ('waiting', 'waits on the user (flag:needs-user)'))
        stall = issue(key='stall-20261010-03', labels=held, body=issue(key='stall-20261010-03')['body'].replace(
            'U83', 'L94').replace('ui.md', 'loop-rules.md'))
        self.assertEqual(orders.classify([stall], {}, [])[0]['state'], 'ready')

    def test_l34_an_implementation_without_acceptance_on_main_is_specified_first(self):
        for scenarios, file in [('U9999', 'ui.md'), ('U83', 'nowhere.md'), ('the drawer flow', 'ui.md')]:
            with self.subTest(scenarios=scenarios, file=file):
                body = issue()['body'].replace('Scenarios: U83', f'Scenarios: {scenarios}').replace('ui.md', file)
                row = orders.classify([issue(body=body)], {}, [])[0]
                self.assertEqual((row['state'], row['mode'], row['respecify']), ('ready', 'specify', True))
                output = io.StringIO()
                with patch.object(orders, 'gh', return_value=issue(body=body)), \
                     patch.object(orders, 'read_orders', return_value=[row]), redirect_stdout(output):
                    orders.task(1)
                self.assertIn('Mode: specify', output.getvalue())
                self.assertIn('keeps Mode: implement', output.getvalue())
                self.assertNotIn('Closes #1', output.getvalue())

    def test_l34_explicit_pr_link_preserves_a_manual_agents_owner(self):
        prs=[dict(number=12,headRefName='codex/fix',body='Refs #1',isCrossRepository=False)]
        self.assertEqual(orders.classify([issue()],{},prs)[0]['state'],'in-flight')
        prs[0]['body']='Refs #10'
        self.assertEqual(orders.classify([issue()],{},prs)[0]['state'],'ready')
        prs[0].update(body='Refs #1',isCrossRepository=True)
        self.assertEqual(orders.classify([issue()],{},prs)[0]['state'],'ready')

    def test_l34_existing_branch_owner_survives_migration(self):
        row = orders.classify([issue()], {}, [{'number':12,'headRefName':'build/U83'}])[0]
        self.assertEqual((row['state'], row['reason']), ('in-flight', '#12'))

    def test_l34_only_completed_dependencies_unblock_work(self):
        for state, reason, expected in [('open',None,'waiting'),('closed','not_planned','waiting'),('closed','completed','ready')]:
            with self.subTest(state=state, reason=reason):
                rows = orders.classify([issue()], {1:[dict(number=2,state=state,state_reason=reason)]}, [])
                self.assertEqual(rows[0]['state'], expected)

    def test_l34_duplicate_keys_block_both_orders(self):
        self.assertEqual([r['state'] for r in orders.classify([issue(),issue(2)],{},[])], ['blocked','blocked'])

    def test_l34_malformed_order_is_visible_and_not_ready(self):
        row = orders.classify([issue(body='Key: U83\nsomething to do')],{},[])[0]
        self.assertEqual(row['state'],'blocked')
        self.assertIn('Specification',row['reason'])

    def test_l34_a_request_without_queue_fields_is_specified_first(self):
        request = issue(7, body='The send button stays enabled without a Door.')
        row = orders.classify([request],{},[])[0]
        self.assertEqual((row['state'],row['mode'],row['slug'],row['priority']),('ready','specify','issue-7',50))
        output = io.StringIO()
        with patch.object(orders, 'gh', return_value=request), \
             patch.object(orders, 'read_orders', return_value=[row]), \
             redirect_stdout(output):
            orders.task(7)
        self.assertIn('Mode: implement, so the queue builds it', output.getvalue())
        self.assertNotIn('Closes #7', output.getvalue())

    def test_l34_priority_comes_from_issues(self):
        urgent = issue(2,'U84',body=issue(2,'U84')['body'].replace('Priority: 2','Priority: 0'))
        self.assertEqual([r['issue'] for r in orders.classify([issue(),urgent],{},[])],[2,1])

    def test_l34_dependency_cycle_never_dispatches(self):
        rows = orders.classify([issue(),issue(2,'U84')],
                               {1:[issue(2,'U84')],2:[issue()]},[])
        self.assertEqual([row['state'] for row in rows],['blocked','blocked'])
        self.assertTrue(all('cycle' in row['reason'] for row in rows))

    def test_l34_api_failure_is_not_an_empty_queue(self):
        with patch.object(orders, 'gh', side_effect=subprocess.CalledProcessError(1, 'gh')):
            with self.assertRaises(subprocess.CalledProcessError):
                orders.read_orders()

    def test_l34_paginated_issue_and_dependency_pages_are_all_read(self):
        def api(*args):
            if args[0] == 'pr': return []
            if '/dependencies/' in args[1]:
                return [[dict(number=3,state='closed',state_reason='completed')],
                        [dict(number=4,state='open',state_reason=None)]]
            return [[issue()], [issue(2,'U84')]]
        with patch.object(orders, 'gh', side_effect=api):
            rows=orders.read_orders()
        self.assertEqual(len(rows),2)
        self.assertTrue(all(row['state']=='waiting' and '#4' in row['reason'] for row in rows))


if __name__ == '__main__':
    unittest.main()
