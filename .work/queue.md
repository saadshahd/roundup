# Work queue

This is the queue until roundup can hold its own Todos (`docs/development-loop.md`). One line per item: scenario ids, module, owned files and the observer that says the item is done (`AGENTS.md` rule 1). An item without scenario ids and an observer is not ready and is not dispatched. Scenarios with the same PR key ship as one PR in one worktree. Line counts are estimates, sized to the guide of about 2000 lines (#39).

Volatile state is not kept here (it goes stale within hours): no PR numbers, verdicts or merge status. Find a PR with `gh pr list --search "<id> in:title"`; `gh pr list --state merged` and `git log origin/main` are the record of what is done. A row names ids, an owner, owned files and an observer. Text for an id is its heading in `scenarios/` (`grep -n '^\*\*<id>[ .]' scenarios/*.md`); an id with no heading there is reserved in the id table below and its text arrives with its own PR, so check before dispatching it.

A Builder edits only the files its row owns, never `scenarios/`, the README or this file. Observer for every row: `just check` green and the item's own tests (named by its id prefix) pass; UI rows also have QA drive a `just harness <seed>` page.

## In flight

A Builder holds each row and its PR is in review. Nothing below is dispatched again.

| Item | Ids | Owns | Observer | Claimed by |
|---|---|---|---|---|
| Rail keyboard | U31 | `apps/desktop/src/rail`: `Rail.tsx`, `RailRow.tsx`, `keys.ts`, `railFixture.tsx`, `styles.css`, the `u31_` and `keys` tests | `u31_` tests pass | boxd-agents |
| spawn with a prompt (`⇧⌘N`) | U33 | `apps/desktop/src/rail/spawn*`, `Rail.tsx`, `u32_` and `u33_` tests | `u33_` tests pass; the U32 case "cmd and shift" keeps `⇧⌘T` doing nothing | boxd-agents |
| Pad edits never overwrite another Actor | U36 | `apps/desktop/src/pads/**` | `u36_` tests pass; the u20 tests U36 names stay green | boxd-agents |
| reopen, App half | S5 | `crates/desktop/**` | `s5_` tests pass; `s1_` and `s3_` tests stay green | boxd-agents |
| D4 flake: one deadline and a readable failure | D4 | `crates/rup/tests/**` | the Builder reproduces first; `just check` | boxd-agents |
| hold an early Signal until the Agent is registered | A14 | `crates/agents/**` | `a14_` tests fail before the fix and pass after | boxd-agents |
| `review` keeps the branch's trailers across a merge | L22 | `loop/boxd.sh` and its tests | `loop/boxd.test.sh` | boxd-agents |
| the boxd check does not depend on files outside the checkout | L40 | `loop/boxd.sh`, the tsc setup and their tests | `l40_` tests | boxd-agents |
| MVP gate | U2-U21 screenshots; cold start, keystroke-to-render p95, RSS with 10 idle Agents | QA and Driver, no Builder | the Phase 3 screenshot pack and the gate report | QA |

## Ready now

Scenario text is on main and nothing it needs is unmerged. Every row owns a directory no other ready row owns, so all start at once. Start the first row you have a free Builder for.

| Id | Item | Owns | Keeps green |
|---|---|---|---|
| audit-todos | coverage audit: T1, T2, T3, T5, T7 | `crates/todos/**` | existing tests |
| audit-pads | coverage audit: P2, P3, P7 | `crates/pads/**` | existing tests |
| audit-perf | coverage audit: R1-R7 | `crates/perf/**` | existing tests |
| sweep-rupd-harness | one shared way for tests to start a `rupd` and wait for `daemon.ping` | tests under `crates/rup/tests/**` and `crates/rupd/tests/**` | every existing test unchanged; the Builder lists each duplicated block in the PR first, and stops if sharing needs a new crate |

## Waiting

Scenario text is on main. The row starts when what it waits on has merged.

| Id | Item | Owns | Keeps green | Waits on |
|---|---|---|---|---|
| U37 | reopen, webview half | `apps/desktop/src/app/**` | `u25_daemon_gone.test.tsx` before a reopen | S5 |
| U38 | empty states | one small file per region in `src/rail`, `src/todos`, `src/pads`, `src/terminal` | U2's tests; narrows U14's test | U36 |
| U40 | the Drawer takes and gives back focus | `apps/desktop/src/drawer/**` | `u27_focus_already_in_another_field_is_left_alone`, the `u28_` tests | U31 |
| U41 | Rail by keyboard, the rest | `apps/desktop/src/keys/**`, one line in `src/App.tsx` | U31's and U32's tests | U31 |
| U44, U46, U47, U49 | rail polish (one PR) | `apps/desktop/src/rail/**` | `u6_` to `u10_`, `u30_`, `u32_`, `u22_` tests | U31 and U33 |
| U50 | the Pad's text fills its Drawer | `apps/desktop/src/pads/**` | `u20_` tests | U36 |
| U4, U25 | contrast: text at 4.5:1 | `src/styles.css`, `src/rail/styles.css`, the contrast tests, `u5_drawer.test.tsx` | `u4_`, `u5_`, `u25_` tests | the user's go |
| U55 | diagnose the empty-Terminal notch | none: a report and `artifacts/ux/U55/` | the report and two screenshots | the user's go |

The coverage audit: every scenario id in `scenarios/*.md` has a test with its lowercase id as a prefix, except the `L` ids, whose tests are shell scripts that name them `L<n>` (`loop/*.test.sh`), W1 (its observer is `just check`), and the ids in flight. The ids above have exactly one test each. An audit Builder reads each of its scenarios clause by clause, adds one test per clause that no test asserts, adds none for a clause already covered, and puts a table of id, clause and test name in the PR. It adds no scenario text and changes no behavior; a clause that the code does not satisfy is a defect to report to the Architect, not to fix in the audit PR.


## Deferred past the MVP (no scenario yet)

- motion.md rows other than the Drawer slide and drag;
- Terminals named from their first command;
- the `● 1 below` line (U30 covers jumping, not the pinned line), provenance letters, the Todo `on` field and the Pad storage switch;
- Inbox, Messages, Routes, Extensions and history;
- packaging (a signed `.app`).

## Swarm protocol

**Stacking.** A Builder may branch from an unmerged base PR's branch when its scenario says "after X" or "may stack", opens its PR against `main` as `AGENTS.md` says, and merges only after its base has merged. Its own commits touch no file its base's commits touch, so the two stay reviewable apart.

**Writers.** A writer is an agent that writes scenarios and no code: the Architect gives it one scenario file and one id range, and records both in the id table below. It owns the scenario file and the id range the Architect gave it, edits no other scenario file and not this file, and never decides a contract change, a new RPC method or an App seam command; those and any change to a merged scenario go to the Architect.

Agents coordinate only through this repo: this file, `scenarios/`, PRs and their commit trailers. The WIP limit, the `Claimed-by:` line and the one-day claim expiry are queue policy added by the Architect, not `AGENTS.md` rules; change them here.

- **Claiming.** An item is claimed by a `Claimed-by: <agent-id> <date>` line under its row. Only the Architect pushes it to main (`AGENTS.md`: nobody else pushes there). A Builder requests a claim by being named in a dispatch; a Builder never claims by editing this file in its own PR, and does not start an item that has no claim. A claim with no branch or PR after a day is released by the Architect.
- **WIP limit.** At most 12 Builders work at once (matching `BOXD_MAX_VMS`), counting every open PR that is not waiting on review. The Architect dispatches no thirteenth. `boxd` VMs are a further limit, `BOXD_MAX_VMS`, default 12 (`AGENTS.md`).
- **One id per author, a different id per reviewer.** Every authored commit carries `Author-Agent: <id>`. The Reviewer's approval is an empty commit that carries only `Reviewed-by-Agent: <id>`, and its id differs from every `Author-Agent` in the PR. It is the newest commit: anything pushed after it needs a new approval. `loop/rules.sh trailers` reads `git log --no-merges`, so it does not see a merge of main made after the approval; the Merger checks that by hand (`git log --first-parent` shows no merge above the approval). A merge of main that touches only a lockfile gets a fresh empty `Reviewed-by-Agent` commit and no more. A Reviewer is never given the Builder's rationale (rule 5).
- **How the Merger gates.** The Merger merges a PR only when rule 1 holds, all at once: CI `check` green; the scenario ids named in the PR have passing tests, in e2e where the scenario is end to end, and the item's named observer passes as well, never instead; an approval from a different id; `loop/rules.sh trailers`, `size origin/main` and `vocab` clean; zero anti-slop findings. PRs merge in the order of their "merge after" column, and a stacked PR merges after the PR it stacks on. If main is red, the Merger stops and reverts; it never fixes forward (rule in `AGENTS.md`).
- **Stale branches.** A Builder whose branch is behind main merges `origin/main` into its branch (the repo's practice, for example on `builder/mcp`), runs `just check`, pushes, and asks for a new approval. A stacked branch merges its base's head first, then main. It never rebases a branch with an approval or a review in flight.
- **`Cargo.lock` and `pnpm-lock.yaml` conflicts.** Two PRs that add packages both rewrite the lockfile, and the merge leaves conflict markers inside it. Never edit them by hand. Take main's file and let the tool regenerate the branch's additions: `git checkout origin/main -- Cargo.lock && cargo update -w`, and `git checkout origin/main -- pnpm-lock.yaml && pnpm install`. Then commit the result with the merge.
- **Scenario ids are reserved, not taken.** When the Architect dispatches an item, it reserves that item's ids in a line under its row, `ids: U27–U28`, pushed to main like a claim. A Builder writes scenarios and tests only under its reserved ids and asks the Architect for more rather than picking the next free number. A Reviewer blocks a PR whose ids are not reserved to it, or that clash with an id in another open PR or on main. This is queue policy added by the Architect, like the claim line. Current allocation:

  | Ids | Holder |
  |---|---|
  | U23–U24 | #60 `ux-layout` |
  | U25 | #61 `ux-daemon` |
  | U26 | #62 `chrome-harness` (renumbered from U23) |
  | U27–U28 | `ux-drawer` (focus on close, Esc, the Drawer edge) |
  | U29 | `ux-pads` (the Pad row) |
  | U30 | #74, jump to what needs you (merged) |
  | U31 | Rail keyboard, builder-ux-drawer |
  | U32 | #75, spawn shortcuts (merged) |
  | U33 | spawn with a prompt, boxd-agents |
  | S5 | reopen, App half, boxd-agents |
  | U34 | scrollback, boxd-agents |
  | U35 | Todo triage, boxd-agents |
  | U36 | Pad edit safety, boxd-agents |
  | U37 | reopen, webview half |
  | U38 | empty states |
  | U39 | retired: superseded by #93's build-time probe (`just perf-keystroke`, K1 to K3, R11); #90 closed. Rule 7's keystroke-to-render number is #93's real-window p95 in WKWebView, one probe and one pairing rule (the first render after the emulator parsed the typed character itself); no second in-app hook that pairs a keystroke with any later output, because it can end early on unrelated output and so bounds nothing. The Daemon-side cheap bound already exists as `just perf`'s write-to-output limit (R8) |
  | U40 | the Drawer's focus |
  | U41 | Rail by keyboard, the rest |
  | U42 | dropped: a false positive (a stuck key in the test driver), never an app defect |
  | U43 | dropped: not a bug (the automated double-click hit the Live line, not the name) |
  | U44, U46–U47, U49 | rail polish |
  | U45, U48 | `⌘J` rules and the chip |
  | U50 | Pad text fills its Drawer |
  | U55 | empty-Terminal notch diagnosis |
  | R11, R12 | #93, keystroke probe (merged) |
  | R13 | load-aware perf comparison |
  | A13 | symlinked agents directory |
  | C1 | dropped-call outcome (`scenarios/rpc.md`) |
  | L9–L14 | `boxd-swarm` (#73): L9 review, L10 input, L11 reboot, L12 swarm, L13 status, L14 kill; it also rewords L6 (secret) and L8 (cap) |
  | L15 | #66, `loop/boxd.sh check` and `bake` (merged) |
  | U61–U79 | `scenarios/ui-rail.md`, writer-ui-rail |
  | U80–U99 | `scenarios/ui-surfaces.md`, writer-ui-surfaces |

  Every Builder has ids; the Architect reserves more on request. `scenarios/ux.md` is gone: every candidate in it now has a `U` id above.
- **Every item names scenarios and an observer.** An item with no scenario ids, or with no observer a Reviewer can run, is returned to the Architect. A new idea starts as a scenario in `scenarios/` (an Architect PR), then becomes a row here.
- **Stop conditions.** If `loop/out/PAUSED` exists, a limit was hit: stop and tell the user. A defect that needs a change in `contracts/`, `CONTEXT.md` or the App seam stops the Builder and goes to the Architect.

## Linux VMs and CI

Once `crates/desktop` is a workspace member, every `just check` compiles Tauri, which makes macOS CI slower. On Linux, Tauri needs WebKitGTK and its dev packages. Unverified here: the `ru-toolchain` snapshot is believed to have none of them, so a `boxd` `just check` would fail in every module until the snapshot is rebaked (`loop/boxd.sh bake`). Default until the user decides (open question): run Builders in local worktrees.
