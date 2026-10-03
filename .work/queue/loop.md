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
