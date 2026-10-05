---
name: percy-gate
description: "Enforce the visual review loop before a UI change can ship. Runs /percy:review, then auto-iterates on the fix for anything that needs fixing — likely-bugs, unplanned changes the plugin is confident weren't intended, AND any human-rejected build (including after a manual review of a flagged comparison). Surfaces reasoning, rejects on confirmation, prompts the agent to fix, and re-runs — up to 3 iterations with a delta-gate and snapshot budget. The single skill to run before merging UI changes."
argument-hint: "[optional: build ID; defaults to latest build on current branch]"
---

# Percy Gate

The enforcement skill. Wraps `/percy:review` and turns its per-comparison verdicts into action: surface real bugs with reasoning, reject the build, prompt the agent to iterate, re-review. Loop terminates on a clean review, hitting the iteration cap, the delta-gate, or the snapshot budget.

In v1, **every reject decision is confirmed by the human** before Percy is actually called. v2 will auto-reject high-confidence cases.

## Prerequisites

- `/percy:setup` has been run.
- `.percy/config.yml` exists.
- A Percy Visual Intent section **should** exist for the current branch (run `/percy:visual-intent` first if not). If missing, the skill enters NO-INTENT degraded mode — see Step 7. Auto-reject and the iteration loop are disabled in that mode.

## Loop policy

Three stop conditions, all enforced:

1. **Hard cap:** maximum 3 iterations per session (configurable via `PERCY_GATE_MAX_ITERATIONS`).
2. **Delta gate:** stop early if iteration N has ≥ the *iterate-set* count (likely-bugs + confident-unintended unplanned, or the rejected build's changed comparisons) of iteration N-1. The agent is thrashing, not converging.
3. **Snapshot budget:** stop early if total snapshots used this session ≥ `PERCY_SNAPSHOT_BUDGET` (default 50). Count only **iteration builds** — builds the loop itself triggers — the initial CI build doesn't count (it was spent before the gate ran, and charging it means one nightly-sized build exhausts the budget before iteration 1). The default can be raised via `PERCY_SNAPSHOT_BUDGET`; `/percy:setup` may size it as `max(50, 3× typical build size)` from its recent-builds probe.

On any early-stop, surface the reason explicitly:
> Stopping at iteration 2: bug count went 3 → 3 (delta-gate). Handing back to human.

## Steps

### Iteration state (per session)

```
iteration = 0
prior_likely_bug_count = null
snapshots_used = 0
```

### 1. Run `/percy:review`

Invoke `/percy:review` and collect the structured verdict object. The relevant per-comparison fields:
- `verdict` — one of `expected | likely-bug | unplanned-change | needs-human | likely-flaky-test | asset-capture-issue`
- `regions[]` — Percy's per-region data (titles, descriptions, `change_reason`, `visual_quality`)
- `reasoning` — the agent's per-comparison rationale
- `code_change_matched` — whether a code change in the git diff corresponds to this comparison's flagged surface

Aggregate (v0.3 verdict tiers):
- `likely_bugs` — comparisons with verdict `likely-bug`
- `unplanned_changes` — comparisons with verdict `unplanned-change`. Split by the `actionable_unintended` flag from `/percy:review`:
  - **`actionable_unplanned`** — `unplanned-change` where the agent is **confident the change is unintended** (a real side-effect / ripple the developer did not mean to introduce — not a plausibly-deliberate-but-unstated change). These are fixable without a human deciding, so they go into the **iterate set** alongside bugs.
  - **`ambiguous_unplanned`** — `unplanned-change` that could plausibly be a deliberate-but-unstated change. These still go to human review (Case B) — the agent shouldn't auto-revert something the developer may have meant.
- `needs_human` — comparisons the agent couldn't resolve from Percy text + code context + intent
- `likely_flaky_tests` — comparisons with verdict `likely-flaky-test` (snapshot history / dynamic-content)
- `asset_capture_issues` — comparisons with verdict `asset-capture-issue` (Percy network-logs flagged asset failures)
- `expected` — comparisons with verdict `expected`

**Iterate set** = `likely_bugs` + `actionable_unplanned`. This is the set the plugin will reject + auto-iterate on. (Bugs *and* changes the plugin is confident weren't intended both get fixed; only genuinely ambiguous cases wait for a human.)

(There is no `ambiguous` tier — `/percy:review` resolves those inline via `change_reason` + code context, producing `expected`, `likely-bug`, `unplanned-change`, or `needs-human`.)

### 2. Decide

**Pre-flight — write creds.** Before showing **any** reject/approve confirm, check `write_creds_verified` in `.percy/config.yml` (or `PERCY_USERNAME` + `PERCY_ACCESS_KEY` in the env). If missing, say so up front — "I can classify but can't reject/approve — run `/percy:setup` to add your BrowserStack access key" — and present the verdicts as recommendations with dashboard deep-links instead. Never walk the user through a y/n confirm and then 401 after they say yes.

**Case D — the build was already rejected by a human (check this FIRST):**

Before evaluating the plugin's own verdicts, look at the resolved build's `review-state` (from `/percy:review` Step 1). If it is `rejected` / `changes_requested` **and the plugin did not just reject it itself this invocation** — i.e. a *human* rejected it (directly in Percy, or after a Case B handoff where they reviewed a flagged comparison and decided it's wrong) — then the decision is already made. **Do not re-derive a verdict to decide whether to reject; go straight to the iterate-on-fix loop.**

- Run `/percy:review` only to gather *what changed and why* (Percy's per-region `change_description` / `visual_quality_reason` for the build's changed comparisons) — not to re-decide.
- **Confirm before fixing (Step 4's fix-confirmation gate).** The reject already happened, so skip Step 3 — but the agent still must NOT edit code without a go-ahead. Surface what was rejected + what you'd change, and ask. Then **jump to Step 4** with that list.
- The same loop caps apply (Step 5): 3 iterations, delta-gate, snapshot budget.

This is the key path that makes iteration fire on **any** reject — the plugin's own *and* a human's — including after a manual review of a `needs-human` / flagged comparison.

**Case R — regression run (no PR): triage, don't iterate.**

If `/percy:review` ran in **regression mode** (default-branch / non-PR / scheduled run — intent = the **accumulated `## Percy Visual Intent` of PRs merged since the last approved baseline**, or empty if nothing merged), the gate's job changes: there is no PR diff to fix, so the **iterate loop does not apply**. Instead, triage:

- **Everything expected or noise** → changes either match a merged PR's intent or are dynamic/flaky. Summarize with attribution ("7 changes: 4 match merged intents ([PR #123](url), [PR #124](url)), 3 recurring dynamic surfaces") and recommend **approve**; one confirm clears the whole run. This is the QA time-saver: what used to be snapshot-by-snapshot eyeballing becomes one line.
- **Genuine anomalies (`likely-bug`)** → these came from *outside* this repo (a deploy, backend/data change, environment drift). Surface each with Percy's reasoning + the snapshot deep-link, recommend flagging (one confirm), and hand off for investigation — e.g. "correlate with what shipped since the last approved run." Do **not** prompt a code-fix iteration; there's nothing in this working tree to edit. If the run is anomalies-only, a build-level **reject** is fine — but know it's **terminal**: a rejected build refuses every further review action (`409 "Can only review non-rejected builds"`, including `unapprove`), so once rejected there is no partial approval later. If any snapshot might still need approving, use `request_changes` instead.
- **Mixed** → **don't reject the whole build** — that would hold the baseline hostage to a handful of anomalies. Use **snapshot-scoped reviews**: the `/reviews` endpoint accepts a `snapshots` relationship alongside the build:
  ```json
  "relationships": {
    "build": { "data": { "type": "builds", "id": "<build_id>" } },
    "snapshots": { "data": [{ "type": "snapshots", "id": "<snapshot_id>" }, ...] }
  }
  ```
  Two actions:
  1. **Approve** the explained + noise snapshots — `action: "approve"` + the `snapshots` relationship (one confirm, listing them with their matched PR#s). Scoping is respected: exactly those snapshots flip to `approved`.
  2. **Flag** the anomaly snapshots with **`action: "request_changes"`** + the `snapshots` relationship (one confirm, with Percy's reasoning + deep-links) — then hand those to QA for investigation as above. Scoping is respected: the flagged snapshots go `changes_requested`, the build becomes `changes_requested (changes_requested_snapshot)`, and the earlier approvals stick.

  ⚠️ **Never send `action: "reject"` with a `snapshots` relationship.** Verified live: reject **ignores the snapshot scope entirely** — it rejects the whole build (the response even lists every snapshot id), wipes any per-snapshot approvals already made, and leaves the build in the terminal rejected state above. Per-snapshot "reject" in Percy is `request_changes`.

  **Baseline mechanics:** a `changes_requested` build still becomes the next build's base build. The approved snapshots' surfaces are baseline now (next run diffs them at 0.0), and the flagged snapshot diffs against its *flagged* capture — so after the external fix lands, the next morning shows a "fix restored parity" diff on exactly that snapshot; approve it and the run is clean. This is what "approve the explained snapshots so the baseline advances" buys.

Loop caps don't apply (no loop). Everything else — human confirm before approve/reject, verbatim Percy reasoning, deep links — unchanged.

**Case A — clean (zero likely-bugs, zero unplanned-changes, zero needs-human, zero flaky-tests, zero asset-capture-issues):**

Every comparison is `expected` — the build is visually clean per stated intent. Recommend auto-approving the Percy build and **wait for human confirmation** (symmetric with the reject path; same v1 human-in-the-loop guardrail).

Surface a summary table so the user can sanity-check what they're about to approve:

> ✅ **Percy gate verdict: PASS**
>
> All N snapshots changed in ways that match stated intent:
> - [`<snapshot>`](<snapshot deep-link>) (<browser> @ <width>px) — matches intent: "<intent clause>"
> - [`<snapshot>`](<snapshot deep-link>) (<browser> @ <width>px) — matches intent: "<intent clause>"
> - … (truncate to 10 rows; show "…and X more" if more)
>
> Proceed with approve? (y/n)

Approve guardrails — only run this path if **all** of these hold:
- `likely_bugs`, `unplanned_changes`, `needs_human`, `likely_flaky_tests`, `asset_capture_issues` are all empty
- At least one comparison was AI-processed (don't auto-approve in NO-AI mode)
- Visual Intent was present (don't auto-approve in NO-INTENT mode — that's already gated upstream by `/percy:review`)

On confirm: call the Percy reviews endpoint with `action: "approve"` — same auth and endpoint as reject, different action value. See "On confirm: approve" below for the request body.

If the user says no, stop without calling the API. The Percy build stays in `unreviewed` state — same outcome as before this skill ran.

**Case A.1 — only `likely-flaky-test` verdicts (no real bugs):**
- Print: "🔁 Percy gate found N flaky-test captures (snapshot pre-loaded). Build itself is visually clean."
- For each `likely-flaky-test` comparison, surface:
  > `<snapshot_name>` was [blank / incomplete / loader-rendered] in [X of last 20] builds. Likely a wait-condition bug in the test, not a UI regression.
  > Recommend fixing the test's page-load wait before next run. [open the flagged snapshot ↗](<dashboard_url>)
- Do NOT auto-reject. The build passes UI review; test instrumentation is the real fix.

**Case A.2 — only `asset-capture-issue` verdicts:**
- Print: "⚙️ Percy gate found N broken-image comparisons caused by Percy's asset capturer, not UI regressions."
- Surface the failing resource URLs from `/api/v1/network-logs`.
- Recommend reviewing Percy's [Snapshot Debug docs on asset capture](https://www.browserstack.com/docs/percy/troubleshoot/self-debug) — usually a CDN allowlist or `network-idle` timeout issue.
- Do NOT auto-reject.

**Case B — only `needs-human` + `ambiguous_unplanned` (iterate set is empty):**
- This is the residual that genuinely needs a person: comparisons the agent couldn't resolve, plus unplanned changes that *might* be deliberate-but-unstated (so the plugin won't auto-revert them). Actionable unplanned changes are NOT here — they're in the iterate set (Case C).
- Don't recommend reject. Surface these comparisons with their dashboard links and Percy's per-region reasoning verbatim.
- **If the human then rejects** one of these in Percy after reviewing, the next `/percy:gate` invocation picks it up via **Case D** and iterates on the fix.
- Output format:
  > ⚠️ Percy gate found N comparisons needing human review (no clear bugs).
  > 
  > Needs human (couldn't resolve from Percy reasoning + code context + intent):
  > 1. `<Snapshot>` / `<Browser>` / `<width>px` — Percy: "<region change_description>"
  >    Percy's concern: "<visual_quality_reason>"
  >    Agent's note: "<why it couldn't decide>"
  >    [open the flagged snapshot ↗](<dashboard_url>)
  > 
  > Unplanned changes (diff exists, not in intent, Percy didn't flag as bug):
  > 1. `<Snapshot>` / `<Browser>` / `<width>px` — Percy: "<region change_description>"
  >    Not covered by stated intent. Verify this wasn't an unintended side effect.
  >    [open the flagged snapshot ↗](<dashboard_url>)
- Wait for human acknowledgement. Do not reject. Hand off — the human approves/rejects in Percy.

**Case C — iterate set non-empty (likely-bugs and/or confident-unintended unplanned changes):**

Surface the iterate set with full Percy reasoning, grouped by snapshot. Include both `likely_bugs` and `actionable_unplanned` — for the latter, frame it as "a real change that wasn't in your stated intent and looks unintended." (If the developer says it *was* deliberate, the fix is to add it to the Visual Intent and re-run, not to revert — call this out in the prompt.)

> 🛑 Percy gate found N items to fix — M likely-bugs + K unintended changes (iteration <i>/<cap>):
>
> Snapshot `<name>`:
>   - <browser> @ <width>px — Region: "<change_title>"
>     Percy: "<change_description>"
>     Why it's a bug: "<visual_quality_reason>"
>     Why this isn't intent: <agent's reasoning>
>     Cluster bug-promoted from sibling: yes/no
>     [open the flagged snapshot ↗](<dashboard_url>)
>
> Snapshot `<name>`:
>   - ...
>
> I believe these are wrong and want to try to fix them. Reject this build and let me attempt the fix? (y/n)
> — If any are actually deliberate, tell me and I'll add them to the Visual Intent instead of changing code.

Wait for human confirmation. This single yes authorizes **both** the Percy reject **and** the agent's fix attempt. **Do not auto-reject — and do not edit code — without it (v1).**

### 3. On confirm: reject *or* approve

Both actions hit the same Percy reviews endpoint, with different `action` values. Auth differs from reads:

- Both `reject` and `approve` require **BrowserStack user credentials** (`PERCY_USERNAME` + `PERCY_ACCESS_KEY`), not the project `PERCY_TOKEN`.
- Project tokens return 403 on `/reviews`.

**On confirm: reject** (Case C):
```
POST /api/v1/reviews
Authorization: Basic <base64(PERCY_USERNAME:PERCY_ACCESS_KEY)>
User-Agent: curl/8.7.1
Content-Type: application/vnd.api+json

{
  "data": {
    "type": "reviews",
    "attributes": {
      "action": "reject",
      "reason": "Auto-rejected by percy-gate: <N> likely-bug(s) — <short summary>"
    },
    "relationships": {
      "build": { "data": { "type": "builds", "id": "<build_id>" } }
    }
  }
}
```

**On confirm: approve** (Case A):
```
POST /api/v1/reviews
Authorization: Basic <base64(PERCY_USERNAME:PERCY_ACCESS_KEY)>
User-Agent: curl/8.7.1
Content-Type: application/vnd.api+json

{
  "data": {
    "type": "reviews",
    "attributes": {
      "action": "approve",
      "reason": "Auto-approved by percy-gate: all <N> snapshot diff(s) match stated visual intent"
    },
    "relationships": {
      "build": { "data": { "type": "builds", "id": "<build_id>" } }
    }
  }
}
```

Verify the response status is 201. If 401 ("No user found"), the BrowserStack creds are missing or aren't linked to a Percy user — surface the actual error and point the user at `/percy:setup` (Step 1c adds and verifies the access key), then stop. On 409, **read the error body before concluding anything**: "approve action is already performed" / "reject action is already performed" means the build was already actioned — surface and stop without retrying. But a 409 can also fire **right after build finalization** while snapshots are still processing — body: `"Can only review finished builds"` (the build can sit in `state: processing` for a minute or more after the CLI prints "Finalized"). For that body, poll `GET /builds/<id>` until `state: finished`, then retry the review once.

### 4. Fix-confirmation gate, then hand iteration back to the agent

**The agent must NOT edit code to "fix" a visual issue without an explicit go-ahead.** Before the first fix attempt of the loop, the user has to have confirmed:
- **Case C** — the combined "reject this build and let me attempt the fix? (y/n)" already covers it (one yes authorizes reject *and* fix).
- **Case D** (human already rejected) — there was no reject prompt, so ask the fix-confirmation here before editing:
  > 🔧 This build was rejected. I think these <N> changes are wrong and want to try to fix them:
  > - <snapshot> / <browser> @ <width>px — <one-line: what's wrong>
  > Want me to attempt the fix? (y/n) — if any were actually intended, tell me and I'll leave them.

On **no** → stop and hand back to the human; don't touch code. On **yes** → proceed with the prompt below. (The 3-iteration cap, delta-gate, and snapshot budget still bound everything after the yes.)

Prompt (covers Case C *and* Case D — plugin-rejected or human-rejected):
> Iteration <i>: build rejected. Fix the <N> items above — **while preserving the developer's stated visual intent.** Your job is "keep the intent, remove the defect," NOT "make Percy green by any means."
>
> **Before editing, re-read the `## Percy Visual Intent` for this branch** (PR description or `.percy/intent-<branch>.md`). Then fix under these rules:
> 1. **Preserve the stated intent.** The fix must keep expressing what the developer declared they wanted. Change the *implementation* that causes the defect — not the goal.
> 2. **Fix the cause, not the change.** Prefer the smallest edit that removes the defect while keeping the goal — e.g. `height` → `min-height`, raise/clamp the cap, adjust the one property that overflows — over reverting the rule or commit that introduced the intended change.
> 3. **Never silently revert to baseline.** If a candidate fix would make the build identical to the approved baseline (the intended change fully gone), that is a **FAILED fix, not a success** — do not do it.
> 4. **If the intent genuinely can't be satisfied without the defect, STOP and ask — don't decide for the user:** "This change can't be done as stated without <defect>. Options: (a) revert it, (b) try alternative approach X, (c) you adjust the intent." Wait for their call.
>
> - For an **unintended change**: if it was actually deliberate, add it to the Visual Intent and re-run instead of reverting; otherwise fix per the rules above.
> - When done, commit and either let CI re-run Percy or run `/percy:run-build`. Then re-invoke `/percy:gate`.

> **Worked counter-example — don't "fix" by reverting the feature.** Intent: *"give content cards a uniform fixed height so the grid lines up"* (`.card { height: 150px; overflow: hidden }`). Percy correctly flags a bug: text-heavy cards get **clipped**. The WRONG fix is to delete the `height`/`overflow` — that removes the intended change entirely and leaves the build identical to `main` (no diff). The RIGHT fix keeps the goal and removes the defect: `height` → `min-height: 150px` (uniform-ish, no clipping), or a taller cap, or clamping the text. Reverting to baseline is the one outcome that is *never* a valid fix when the developer declared that change as intended.

Track (delta-gate uses the full iterate set, not just bugs):
```
iteration += 1
prior_iterate_count = len(iterate_set)   # likely_bugs + actionable_unplanned (Case C); or the rejected build's changed-comparison count (Case D)
# snapshots_used counts ITERATION builds only — add each fix-attempt build's
# total_snapshots when it lands; the initial CI build is never charged.
```

Stop and return — the agent fixes, kicks a new build, and re-invokes `/percy:gate`.

### 5. On next invocation: check stop conditions first

When `/percy:gate` is re-invoked:
- **Empty-diff guard (check FIRST, when `iteration > 0`).** If the new build has `total-comparisons-diff === 0` vs the approved baseline — i.e. the change vanished — the fix likely **reverted the intended change** instead of fixing the defect. Do **not** treat this as a PASS. Surface:
  > ⚠️ The fix appears to have undone your change entirely — this build has no diff vs the approved baseline, so the intended change is gone. That's not a valid fix. Handing back: re-apply the intent and fix the *defect* (e.g. `min-height` instead of removing `height`), or tell me to stop.
  
  Stop the loop and hand to the human. Never auto-approve an empty-diff build that came out of an active fix iteration.
- If `iteration >= max_iterations` → stop, surface "Hit max iterations. Handing back to human."
- If `len(new_iterate_set) >= prior_iterate_count` → stop, surface "Not converging (was N, now M items). Handing back to human."
- If `snapshots_used >= snapshot_budget` → stop, surface "Snapshot budget exhausted."
- Otherwise: go to Step 1 with the new build, update `prior_iterate_count` from the new iterate-set count.

### 6. NO-AI mode (degraded path)

If `/percy:review` returns NO-AI fallback output:
- **Do not auto-reject AND do not auto-approve** — there's no AI signal to justify either action.
- Show the user all changed snapshots with dashboard links.
- Recommend manual review.
- Surface the cause (free tier vs browser-pool config vs browser-upgrade limbo — per `/percy:review` Step 8.2).
- Stop. No iteration loop.

### 7. NO-INTENT mode (degraded path)

If `/percy:review` returns NO-INTENT degraded output (verdicts like `candidate-bug` and `unverified-change` instead of `likely-bug` etc.):
- **Do not auto-reject AND do not auto-approve.** Without a stated intent, auto-rejecting would frequently flag intentional changes; auto-approving would silently ship regressions Percy correctly flagged.
- Surface the full candidate-bug list to the user with dashboard links and Percy's `change_description` + `visual_quality_reason` verbatim.
- Strong prompt to run `/percy:visual-intent` first:
  > ⚠️ Cannot recommend a binary verdict — no Visual Intent was provided. Each ⚠️ candidate-bug below needs your eyes to decide if it's intentional or a regression.
  > 
  > To get auto-verdicts, run `/percy:visual-intent`, then re-run `/percy:gate` — the existing build is re-reviewed; no rebuild needed. The intent doc lets the agent filter intentional changes (A/B tests, feature toggles, redesigns) from genuine bugs.
- Show all candidate-bug comparisons with deep-links. Show all unverified-change comparisons collapsed (one count, not per-row) since they're low-priority.
- Stop. No iteration loop in NO-INTENT mode either.

## Important

- **One human confirmation per reject in v1.** Don't reject builds without explicit user approval.
- **No code edits without a go-ahead (v1).** The agent never edits code to "fix" a visual issue until the user confirms — the combined reject+fix yes (Case C) or the fix-confirmation (Case D). It surfaces *what it found and why it thinks it's wrong* first; the user can also say "that was intentional" to send it to intent instead of a code change.
- **Surface Percy reasoning verbatim** (`change_description`, `visual_quality_reason`). The user needs to see *why* Percy thinks something is a bug, not your paraphrase.
- **Links must be specific + clickable** (see `/percy:review` → "Link formatting"). Every link you surface to the user is a **Markdown** link — `[<snapshot> — <browser> @ <width>px](<deep-link>)`, never a bare/angle-bracketed URL (those render as plain text in the IDE) and never inside a code fence. **Deep-link to the specific snapshot that has the issue** (`…/changed/<snapshot_id>`), not the build root — if a single comparison is flagged, point straight at it. Reserve the build-root link for build-wide actions (e.g. "approve the whole build"). When you tell the user to review the GitHub PR, link it too: `[PR #<n>](<pr_url>)`.
- **The delta-gate matters more than the iteration cap.** If iteration 2 has the same bugs as iteration 1, the agent isn't converging — stop early.
- **Snapshot budget protects the user's Percy quota.** Don't iterate past it even if other conditions allow.
- **State is per-session.** If the user closes the IDE and reopens, the iteration counter resets. That's intentional — a fresh session deserves a fresh budget.
- **Reject and approve use the same auth.** Both actions hit `POST /api/v1/reviews` with HTTP Basic Auth (`PERCY_USERNAME` + `PERCY_ACCESS_KEY`, BrowserStack creds), not the project `PERCY_TOKEN`. Only the `action` field differs (`"reject"` vs `"approve"`).
- **Auto-approve is symmetric with auto-reject.** Both require human confirmation in v1; both stop without calling the API if the user declines; both refuse to fire in NO-AI or NO-INTENT degraded modes. Treat them as mirrored paths — same guardrails, same auth, same UX.
