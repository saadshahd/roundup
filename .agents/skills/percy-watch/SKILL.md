---
name: percy-watch
description: "Watch for a Percy build to finish processing (via Percy API polling or GitHub PR check status) and auto-invoke /percy:gate when it's ready. Use when the user has just pushed code / opened a PR and wants the gate to fire automatically when the Percy build completes — no manual /percy:gate invocation needed."
argument-hint: "[branch name, build ID, or PR number — defaults to current branch]"
---

# Percy Watch

Background a poll on a Percy build (or its GitHub PR check) and automatically invoke `/percy:gate` once the build is processed. Eliminates the "wait for build, remember to run gate" manual step.

## When to invoke

- Auto-invoked from `/percy:visual-intent` Step 5 when the user accepts the watch handoff.
- Invoked directly when the user has already injected intent + pushed and wants to keep working until the verdict comes back.
- NOT invoked in NO-INTENT or NO-AI degraded modes — the watcher will fire `/percy:gate` and `/percy:gate` will surface its own degraded-mode messaging.

## Prerequisites

- `.percy/config.yml` exists (run `/percy:setup` if not).
- A push has happened on the current branch, OR a PR exists with a Percy CI check, OR a build ID has been explicitly provided.

## Steps

### 1. Resolve what to watch

In priority order:

1. **If `$ARGUMENTS` is a Percy build ID (numeric)** → watch that build directly.
2. **If `$ARGUMENTS` is a PR number / URL** → use that PR.
3. **Otherwise** → use the current branch (`git rev-parse --abbrev-ref HEAD`):
   - First, check if there's an open PR for this branch: `gh pr view --json number,url 2>/dev/null`. If yes, prefer the GitHub-PR-check path (Step 2 path B).
   - If no PR, fall back to direct Percy-API polling on the latest build for this branch (Step 2 path A).

### 2. Pick a watching path

#### Path A — Percy API polling (laptop-side, no GitHub integration required)

Use when there's no PR yet, or when the customer doesn't have the Percy GitHub integration installed.

Run a background loop:
```bash
# Pseudocode — adapt to host agent's background-task mechanism
# (Claude Code: Bash with run_in_background, or Monitor tool with a tight grep)
while :; do
  state=$(curl -sS -H "Authorization: Token token=\"$PERCY_TOKEN\"" \
    -H "User-Agent: curl/8.7.1" \
    "https://percy.io/api/v1/builds?filter[branch]=<branch>&page[limit]=1" \
    | jq -r '.data[0].attributes | "\(.state)|\((.["ai-details"].["all-ai-jobs-completed"]))"')
  echo "[$(date +%H:%M:%S)] $state"
  case "$state" in
    "finished|true"|"finished_processing|true") exit 0 ;;
    "failed|"*) echo "BUILD-FAILED"; exit 1 ;;
  esac
  sleep 30
done
```

Poll cadence: 30 seconds. Don't poll faster — Percy build processing typically takes 1-3 minutes after CLI upload, and tight polling wastes API quota.

**Zero-build branch:** if no build exists for the branch after 2 polls (`.data` is empty) AND no Percy CI check has appeared on the PR, stop and ask instead of polling null for 20 minutes:
> No Percy build found — this repo's CI may not run Percy. Run `/percy:run-build` now? (y/n)

On yes → invoke `/percy:run-build`, then resume watching. On no → stop the watch cleanly.

Timeout: 20 minutes. If the build doesn't finish by then, fall back to messaging the user: "Percy build still processing after 20 min — check the dashboard manually."

#### Path B — GitHub PR check status (when Percy GitHub integration is installed)

Use when there's an open PR. Faster signal than polling Percy directly — the Percy check transitions on the PR as soon as Percy posts its status.

**Watch ONLY the Percy check — never wait on the repo's other CI checks.** (`gh pr checks --watch` blocks until *all* checks finish, so a slow unrelated job — a 40-minute e2e suite — would delay the visual gate for no reason, and an unrelated *failing* check must not stop the visual review.) Poll the Percy check by name instead:

```bash
while :; do
  bucket=$(gh pr checks <pr_number> --json name,bucket \
    --jq '[.[] | select(.name | test("[Pp]ercy"))][0].bucket')
  case "$bucket" in
    pass|fail) echo "PERCY-CHECK:$bucket"; break ;;
  esac
  sleep 30
done
```

If the result is `pass` or `fail`, proceed to Step 3 (other CI checks keep running on their own — they're the repo's business, not the visual gate's). If no Percy check ever appears (`bucket` stays empty/pending with no Percy entry), fall back to Path A — the customer may not have the Percy GitHub integration installed.

### 3. On completion: invoke /percy:gate

Once the build is in a terminal state (Path A returns success or Path B's Percy check resolves), **invoke the `percy-gate` skill via the Skill tool**. Pass the build ID if known, otherwise let `/percy:gate` re-resolve from the current branch.

Surface a one-line "ready" message to the user before invoking, so they know what's happening:

> 🔔 Percy build ready (build #<num>). Running `/percy:gate`…

If the build failed (Path A returned `BUILD-FAILED`, or Path B's Percy check returned `fail`), don't invoke `/percy:gate` — surface the failure to the user with the dashboard link and stop:

> ⚠️ Percy build failed (not a UI regression — a CI/infra error). See [the build ↗](<build_url>). Re-push or investigate the Percy build directly.

### 3b. Optional — keep watching for a human reject (drives auto-iterate)

If the first `/percy:gate` run ended in a **Case B handoff** (needs-human / ambiguous unplanned changes surfaced for the user to decide), the human may then reject the build in Percy after reviewing. To make iteration fire on that decision, optionally keep a light poll on the build's **review-state**:

```bash
state=$(curl -sS -H "Authorization: Token token=\"$PERCY_TOKEN\"" -H "User-Agent: curl/8.7.1" \
  "https://percy.io/api/v1/builds/<id>" | jq -r '.data.attributes["review-state"]')
# changes_requested / rejected → invoke /percy:gate (it detects the rejected state via Case D and starts the fix loop)
```

- Poll cadence 60s, with a generous timeout (a human may take a while). Or simply rely on the user re-invoking `/percy:gate` after they reject — either way **Case D** in `/percy:gate` picks up the rejected build and iterates on the fix.
- Stop polling once the build is approved or the user cancels.

### 4. Backgrounding

This skill is meant to run **non-blockingly** — the user keeps working in their IDE while the watcher polls. Implementation hints by host agent:

- **Claude Code**: use `Bash` with `run_in_background: true` for the poll loop, or use `Monitor` with a `tail -f` of the poll output and a strict grep filter (`tail -f <file> | grep -E --line-buffered "finished\\|true|BUILD-FAILED"`).
- **Cursor / Codex / Antigravity**: use the host's equivalent backgrounding primitive. If none exists, run the poll inline but surface a "watching — Ctrl-C to cancel" message so the user can interrupt.

When the background watcher fires, the agent re-enters the conversation with the verdict surfaced and `/percy:gate` invoked — same UX as if the user had typed `/percy:gate` themselves.

### 5. Cleanup

- If the user explicitly cancels the watch (Ctrl-C, "stop watching", agent session closed), stop the background task gracefully.
- Don't leave orphaned poll loops running across sessions.

## Important

- **This skill is a thin orchestrator over polling + invocation.** All actual verdict logic lives in `/percy:review` / `/percy:gate`. Don't duplicate it here.
- **Path B is preferred** when available — gh-based check watching is more reliable than Percy API polling because it observes the same signal the customer's other tooling does.
- **Don't auto-merge or auto-act based on the verdict.** `/percy:gate`'s own human-in-the-loop confirm on reject/approve still fires once the verdict comes back. Watcher just removes the "remember to invoke gate" friction, not the "review the verdict" friction.
- **Fail open, never block.** If the watcher itself errors (network issue, gh CLI missing, etc.), surface the error and tell the user to invoke `/percy:gate` manually. Don't wedge.
