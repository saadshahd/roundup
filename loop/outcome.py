#!/usr/bin/env python3
"""L29/L81/L92: model claims for the Trail and independently observed PR delivery."""
import html
import json
import os
from pathlib import Path
import re
import subprocess
import sys

from dispatch import gh


def redact(text, environment):
    for name, value in environment.items():
        if value and re.search(r'TOKEN|SECRET|PASSWORD|API_KEY', name, re.I):
            text = text.replace(value, '[redacted]')
    return re.sub(r'\b(?:sk-ant-[\w-]+|gh[pousr]_[\w]+|github_pat_[\w]+)\b', '[redacted]', text)


def model_outcome(events, environment):
    if not isinstance(events, list) or any(not isinstance(event, dict) for event in events):
        raise ValueError('execution output must be an array of objects')
    result = next((event for event in reversed(events) if event.get('type') == 'result'), None)
    text = '' if result is None else result.get('result', '')
    if not isinstance(text, str):
        raise ValueError('final model result must be text')
    text = redact(text, environment)
    subtype = result.get('subtype') if result else 'missing'
    known = ('missing', 'success', 'error_max_turns', 'error_during_execution', 'error_max_budget_usd', 'error_max_structured_output_retries')
    return {'kind': 'model-claim', 'result': subtype if subtype in known else 'unknown',
            'text': text[:8000], 'truncated': len(text) > 8000,
            'explanation': 'available' if text else 'unavailable'}


def delivery(prs):
    """Report observed state only; readiness is not verified completion or authorship."""
    if not isinstance(prs, list):
        raise ValueError('PR response must be an array')
    opened = [pr for pr in prs if pr['state'] == 'OPEN']
    if len(opened) > 1:
        raise ValueError('multiple open PRs for one branch')
    pr = opened[0] if opened else next((pr for pr in prs if pr['state'] == 'MERGED'), None)
    if pr is None:
        return {'state': 'no-delivery', 'message': 'No open or merged PR observed.'}
    fields = {key: pr[key] for key in ('number', 'headRefOid', 'url')}
    if pr['state'] == 'MERGED':
        return {**fields, 'state': 'merged', 'message': 'Merged PR observed; goal verification remains separate.'}
    if pr['isDraft']:
        return {**fields, 'state': 'draft', 'message': 'PR remains a draft; this run did not leave a ready delivery.'}
    return {**fields, 'state': 'ready', 'message': 'Ready PR observed; independent review and required gates still apply.'}


def observation(branch, number=None):
    fields = 'number,state,isDraft,headRefOid,url'
    # Only an explicitly identified repair PR may use a merged state. Old merged PRs
    # on a reused Builder branch cannot make a new empty run look delivered.
    prs = [gh('pr', 'view', number, '--json', fields)] if number else gh(
        'pr', 'list', '--head', branch, '--state', 'open', '--limit', '100', '--json', fields)
    return delivery(prs)


def observe(branch, number=None):
    report = observation(branch, number)
    # This is an observation, never a verdict, approval, or attribution of a concurrent push.
    text = 'Delivery observation: ' + json.dumps(report)
    print(text)
    if os.environ.get('GITHUB_STEP_SUMMARY'):
        with open(os.environ['GITHUB_STEP_SUMMARY'], 'a') as summary:
            summary.write('<pre>' + html.escape(text) + '</pre>\n')
    return 0 if report['state'] in ('ready', 'merged') else 1


if __name__ == '__main__':
    try:
        if len(sys.argv) == 3 and sys.argv[1] == 'explain':
            events = json.loads(Path(sys.argv[2]).read_text()) if sys.argv[2] and Path(sys.argv[2]).is_file() else []
            print(json.dumps(model_outcome(events, os.environ)))
        elif len(sys.argv) in (3, 4) and sys.argv[1] == 'observe':
            sys.exit(observe(sys.argv[2], sys.argv[3] if len(sys.argv) == 4 else None))
        else:
            sys.exit('usage: outcome.py explain <execution-file> | observe <branch> [pr]')
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError):
        # Never echo malformed execution data or subprocess output, which may hold credentials.
        print('Outcome evidence unavailable: extraction or GitHub observation failed.', file=sys.stderr)
        sys.exit(4)
