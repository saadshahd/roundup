# Work queue: loop

The rows this squad holds (`docs/squads.md`, "Moving the queue"). The protocol, the status rules and the `Ids`/`Holder` table stay in `.work/queue.md`; the rule for a row is the same as there.

## In flight

A Builder holds each row and its PR is in review. Nothing below is dispatched again.

| Item | Ids | Owns | Observer | Claimed by |
|---|---|---|---|---|
| `review` keeps the branch's trailers across a merge | L22 | `loop/boxd.sh` and its tests | `loop/boxd.test.sh` | boxd-agents |
| the boxd check does not depend on files outside the checkout | L40 | `loop/boxd.sh` (shared with L22: merges after it), the tsc setup and their tests | `loop/boxd.test.sh` | boxd-agents |

## Waiting

Scenario text is on main. The row starts when what it waits on has merged. A UI row ends its Item with `(Moves: <D ids>)`, the design-system checks it moves (`docs/design-system.md`, baseline protocol); the Builder's PR body repeats the line and corrects it if the diff moves other checks.

| Id | Item | Owns | Keeps green | Waits on |
|---|---|---|---|---|
| L44–L49 | merge policy: `class`, `rounds`, `merge-ready`, `revert-due`, `dispatch` in `loop/rules.sh`, stall kind f in `loop/stalls.sh`, the `Scenarios:` check in `loop/boxd.sh build`, the new `loop/rules.sh` steps in `.github/workflows/loop.yml` | `loop/rules.sh`, `loop/rules.test.sh`, `loop/stalls.sh`, `loop/boxd.sh` (shared with L22 and L40: merges after them), `loop/boxd.test.sh`, `.github/workflows/loop.yml` | `loop/rules.test.sh`, `loop/boxd.test.sh` | L28 and L33 implemented (stall kinds, `base`), the U130 and L41 row merged first (it owns the same files) (the user opened the post lane on 2026-10-02) |
| L50 | `loop/rules.sh queue` (the move itself, the `Module:` lines and the README change are done) | `loop/rules.sh`, `loop/rules.test.sh` | `loop/rules.test.sh` | the L44-L49 row's `loop/rules.sh` PR merges (one writer of `loop/rules.sh`) |
| L51 | stall kind g in `loop/stalls.sh` (a rejected PR with no push for 60 minutes) | `loop/stalls.sh`, `loop/stalls.test.sh` (or the file L28's tests live in) | `l51_` tests | after the L44–L49 row's `loop/stalls.sh` PR merges (kind f; one writer of `loop/stalls.sh` at a time) |
| L43 | `loop/rules.sh ranges` | `loop/rules.sh`, `loop/rules.test.sh` | `l43_` tests | after the L41 row's `loop/rules.sh` PR and the L44–L49 row's PR merge (all three own `loop/rules.sh` and `loop/rules.test.sh`, one at a time); `docs/squads.md` is on main (#126) |
| L53 | `loop/rules.sh proof <pr>`, its call in `merge-ready` (L46), the `proof/pr-<n>` branch convention, AGENTS.md rules 1 and 8 (already edited here), the Builder and Reviewer prompts | `loop/rules.sh`, `loop/rules.test.sh`, `docs/development-loop.md` | `l53_` tests | after the L44–L49 row's `loop/rules.sh` PR merges (it owns `merge-ready`); until it lands the rule is by hand: no visible PR approves without screenshots in its body |
| L54 | `loop/rules.sh carry <pr>`, its call in `merge-ready` (L46) and `trailers` | `loop/rules.sh`, `loop/rules.test.sh`, `docs/development-loop.md` | `l54_` tests | after the L44–L49 row's `loop/rules.sh` PR merges (it owns `merge-ready` and `trailers`); #207 (per-squad queue files) removes most queue conflicts and is independent |
| L52 | standing QA sweep record, stall kinds `h` and `i` in `loop/stalls.sh`, the user-report line in `loop/rules.sh trailers`, and QA's prompt (`.agents/qa.md`) | `loop/stalls.sh`, `loop/stalls.test.sh`, `loop/rules.sh`, `loop/rules.test.sh`, `.agents/qa.md`, `docs/development-loop.md` | `l52_` tests | after the L51 row's PR merges (one writer of `loop/stalls.sh` at a time) and the L44–L49 row's `loop/rules.sh` PR |
| (loop tooling order) | Builders for the loop scenarios, as two lanes. Lane R (`loop/rules.sh`, one writer at a time): `.work/prompts/loop-rules-1.md` (L33, L44, L45) then `loop-rules-2.md` (L46 `merge-ready`) then `loop-rules-3.md` (L47–L49, `loop/boxd.sh` check) then `loop-rules-4.md` (L54, L53, L56, L55, L43, L50, L41). Lane S (`loop/stalls.sh`): `loop-stalls-1.md` (L28–L30) then `loop-stalls-2.md` (kinds f to k). The lanes edit different scripts and run side by side; both add a section to `docs/development-loop.md`, so the later PR merges main and keeps both sections; the L44–L49 row above is split into the three rules prompts and the stalls ones | see the prompts | `l33_` to `l56_` tests | the L41 row's `loop/rules.sh` PR merges first if it is open; `loop-rules-3.md` also waits for the L21, L22 and L40 PRs (`loop/boxd.sh`) |
| L55 | `loop/rules.sh settings`, stall kind `j`, the `merge-ready` status workflow in `.github/workflows/` (an architect edits `.github/`; the user flips `allow_auto_merge` and creates the ruleset) | `loop/rules.sh`, `loop/rules.test.sh`, `loop/stalls.sh`, `loop/stalls.test.sh`, `.github/workflows/merge-ready.yml` (written in the scenario PR; the Builder tests it) | `l55_` tests | after the L44–L49 row's `loop/rules.sh` PR and the L51 row's `loop/stalls.sh` PR merge |
| L56 | `loop/rules.sh copilot <pr>`, its call in `merge-ready`, stall kind `k`, the Driver's request and the Triage prompt (`.agents/triage.md`) | `loop/rules.sh`, `loop/rules.test.sh`, `loop/stalls.sh`, `loop/stalls.test.sh`, `.agents/triage.md`, `.agents/driver.md` | `l56_` tests | after the L55 row (same files)
| L61 | provisioning gap, exec-before-upload, sha256 verify, toolchain check, events `vm-start-failed` and `upload-failed` in `loop/boxd.sh` | `loop/boxd.sh`, `loop/boxd.test.sh` | `l61_` tests | after PR #184 (L21) merges; starts now otherwise |
