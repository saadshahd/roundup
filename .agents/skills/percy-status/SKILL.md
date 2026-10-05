---
name: percy-status
description: "Read the latest Percy build state for the current branch. No polling, no classification — just a quick status check. Use when you want to know 'is there a build, and is it done yet?'"
argument-hint: "[optional: branch name; defaults to current branch]"
---

# Percy Status

Lightweight status check. Returns the most recent Percy build for the given branch (or current branch) without polling or doing any classification work.

## Steps

### 1. Resolve branch

If `$ARGUMENTS` is blank, use `git rev-parse --abbrev-ref HEAD`. Otherwise use the provided branch name.

### 2. Find the latest build

```
GET /api/v1/builds?filter[branch]=<branch>&page[limit]=1
```

Headers: `Authorization: Token token="$PERCY_TOKEN"`, `User-Agent: curl/8.7.1`.

### 3. Report

Read these fields from `data[0].attributes`:
- `build-number` → build number
- `web-url` → clickable URL
- `state` → `pending | processing | finished | finished_processing | failed`
- `review-state`, `review-state-reason`
- `total-snapshots`, `total-snapshots-unreviewed`
- `total-comparisons`, `total-comparisons-diff`
- `ai-details.total-potential-bugs`, `ai-details.all-ai-jobs-completed`, `ai-details.summary-status`

Output:

```
Branch: <branch>
Build: #<build-number> · [open in Percy ↗](<web-url>)
State: <state> · review: <review-state> (<review-state-reason>)
Snapshots: <total-snapshots> total · <total-snapshots-unreviewed> unreviewed
Comparisons: <total-comparisons> total · <total-comparisons-diff> with diffs
AI: <total-potential-bugs> potential bugs · summary=<summary-status> · jobs-done=<all-ai-jobs-completed>
Created: <relative time from created-at>
```

If no build exists for this branch, say so and suggest `/percy:run-build`.

If the build is still in progress, suggest the user wait or run `/percy:review` to start polling.

## Important

- Don't poll in this skill. One read, one report, done.
- Don't classify or recommend reject/iterate here. That's `/percy:review` and `/percy:gate`.
