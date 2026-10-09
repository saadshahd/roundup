#!/usr/bin/env python3
import importlib.util
import time
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
                     state='open', labels=[{'name':'ready-for-agent'}],
                     body=f'Scenarios: U83\nSpecification: [scenarios/ui.md](https://github.com/a/b/blob/main/scenarios/ui.md)\nKey: {key}\nPriority: 2\n'), **changes)


class Orders(unittest.TestCase):
    def test_l34_only_known_modes_and_providers_are_executable(self):
        for value in ['Mode: skip-review','Provider: anything']:
            row=orders.classify([issue(body=issue()['body']+value+'\n')],{},[])[0]
            self.assertEqual(row['state'],'blocked')
        row=orders.order(issue(body=issue()['body']+'Mode: specify\nProvider: codex\n'))
        self.assertEqual((row['mode'],row['provider']),('specify','codex'))

    def test_l34_task_renders_the_authorized_issue_and_mode(self):
        output = io.StringIO()
        with patch.object(orders, 'gh', return_value=issue()), \
             patch.object(orders, 'read_orders', return_value=orders.classify([issue()], {}, [])), \
             patch.object(orders.trail, 'section', return_value='\n## Trail of #1') as section, \
             redirect_stdout(output):
            orders.task(1)
        section.assert_called_once_with(1)
        self.assertIn('## Trail of #1', output.getvalue())
        self.assertIn('Mode: implement', output.getvalue())
        self.assertIn('Refs #1', output.getvalue())

    def test_l92_a_ready_issue_that_keeps_ending_the_same_way_waits_with_its_last_entry(self):
        rows = orders.classify([issue()], {}, [])
        until = time.time() + 900
        with patch.object(orders.trail, 'plan_for', return_value=(dict(until=until, url='https://x/c1', diagnostic=True), [])):
            row = orders.apply_trail(rows)[0]
        self.assertEqual((row['state'], row['wait_url'], row['wait_until']), ('waiting', 'https://x/c1', int(until)))

    def test_l92_an_issue_with_no_wait_stays_ready_and_a_closed_one_reads_no_trail(self):
        rows = orders.classify([issue(), issue(number=2, key='U84', state='closed')], {}, [])
        with patch.object(orders.trail, 'plan_for', return_value=(dict(until=None, url=None, diagnostic=False), [])) as read:
            states = [r['state'] for r in orders.apply_trail(rows)]
        self.assertEqual(states, ['ready', 'cancelled'])
        read.assert_called_once()

    def test_l92_an_unreadable_trail_starts_no_run(self):
        with patch.object(orders.trail, 'plan_for', side_effect=subprocess.CalledProcessError(1, 'gh')):
            with self.assertRaises(subprocess.SubprocessError):
                orders.apply_trail(orders.classify([issue()], {}, []))

    def test_l34_task_rechecks_new_pr_ownership_before_starting(self):
        with patch.object(orders, 'gh', return_value=issue()), \
             patch.object(orders, 'read_orders', return_value=[dict(issue=1,state='in-flight')]):
            with self.assertRaisesRegex(ValueError, 'no longer eligible'):
                orders.task(1)

    def test_l34_specification_delivery_can_release_implementation(self):
        specification = issue(body=issue()['body']+'Mode: specify\n')
        output = io.StringIO()
        with patch.object(orders, 'gh', return_value=specification), \
             patch.object(orders, 'read_orders', return_value=orders.classify([specification], {}, [])), \
             patch.object(orders.trail, 'section', return_value=''), \
             redirect_stdout(output):
            orders.task(1)
        self.assertIn('Close this specification Issue', output.getvalue())
        self.assertNotIn('specification-only PR does not close', output.getvalue())

    def test_l34_ready_issue_does_not_consult_test_names_or_work_tables(self):
        self.assertEqual(orders.classify([issue()], {}, [])[0]['state'], 'ready')

    def test_l34_an_unapproved_issue_cannot_authorize_execution(self):
        self.assertEqual(orders.classify([issue(labels=[])], {}, [])[0]['state'], 'unspecified')

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
        row = orders.classify([issue(body='something to do')],{},[])[0]
        self.assertEqual(row['state'],'blocked')
        self.assertIn('Key',row['reason'])

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
