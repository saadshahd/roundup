# Work queue

This is the queue until roundup can hold its own Todos (`docs/development-loop.md`). One line per item: scenario ids, module, owned files and the observer that says the item is done (`AGENTS.md` rule 1). An item without scenario ids and an observer is not ready and is not dispatched. Scenarios with the same PR key ship as one PR in one worktree. Line counts are estimates, sized to the guide of about 2000 lines (#39).

Status as of this file: the MVP slice is merged except the MVP gate, which QA and the Driver run in parallel with UX work; see "Open" for what is left.

## Merged

| PR | Item | Scenarios |
|---|---|---|
| #42 | MVP queue and scenarios | |
| #43 | contract `rail.spawnTerminal` | serves A10, U9 |
| #44 | contract JSON schemas for Todo and Pad params | serves M1 |
| #45 | `workspace`, the check covers the App | W1 |
| #46 | `daemon-attached` (`crates/rupd`) | D1 |
| #47 | `ui-shell` | U1–U5 |
| #48 | `app` (`crates/desktop`) | S1–S3 |
| #49 | `ui-pads` | U18–U21 |
| #50 | `ui-todos` | U15–U17 |
| #51 | `ui-rail` | U6–U10 |
| #52 | `mcp` (`crates/rup`) | M1–M3 |
| #53 | `ui-terminal` | U11–U14 |
| #56 | X8 fix: the slow-subscriber test streams 2 MB | X8 |
| #36, #40 | pads-4, terminal-6 | P5, P8, P9, X9, X10 |
| #54, #55 | `agents-stack`, `agents-mvp` | A1–A12 |
| #57 | `ui-drag` | U22 |
| #58, #72 | swarm board and UX scenarios; id reservations | docs |
| #59, #66, #71, #73 | boxd: review, check and bake, `BOXD_MAX_VMS`, swarm, status, kill | L1–L15 |
| #60, #61 | `ux-layout`, `ux-daemon` | U23–U25 |
| #62 | `chrome-harness` | U26 |
| #63, #64 | `ux-drawer`, `ux-pads` | U27–U29 |
| #65 | `app-dev`, the window loads the webview | S4 |
| #68 | perf: `just perf` and budgets | R1–R7 |
| #70 | `daemon-e2e` | D2–D4 |
| #74 | `u30-chip`, jump to what needs you | U30 (was V1) |
| #75 | `ux-spawn`, spawn shortcuts and pinned actions | U32 |

## Open

Every row points at a PR number where one exists; `gh pr list` is the truth when this file and it disagree.

| PR | Item | Scenarios | Module | Observer | State |
|---|---|---|---|---|---|
| #67 | boxd.md corrections | docs | `docs` | reviewer approval | open |
| none yet | Rail keyboard (V2 subset: up and down, a visible focus ring, Enter selects) | U31 | `apps/desktop` | `u31_` tests pass | in flight (builder-ux-drawer) |
| none yet | spawn with a prompt (V3's field; `⇧⌘N`) | U33 | `apps/desktop` (`src/rail/spawn*`) | `u33_` tests pass, and the U32 test case "cmd and shift" is narrowed to keep `⇧⌘T` doing nothing; `just check` green; the PR comes out of `loop/boxd.sh swarm` with no laptop worktree | Claimed-by: boxd-agents 2026-10-01 |
| none yet | scrollback (V5) | U34 | `apps/desktop`: only `src/terminal/**` | `u34_` tests pass; `just check` green; QA RSS with ten Terminals printing 100 000 lines each | ready, ids reserved; no existing test narrowed |
| none yet | Todo triage (V7) | U35 | `apps/desktop`: only `src/todos/**` | `u35_` tests pass; `just check` green; `u15_todo_list.test.tsx` unchanged and green | ready, ids reserved; row text at rest and the second line's text must stay exactly as U15 asserts |
| none yet | Pad edits never overwrite another Actor (V8) | U36 | `apps/desktop`: only `src/pads/**` | `u36_` tests pass; `just check` green; the three `u20_` tests U36 names stay green | ready, ids reserved |
| none yet | reopen, App half (V4) | S5 | `crates/desktop`: only `crates/desktop/**` | `s5_` Rust tests pass; `just check` green; `s1_a_second_open_project_is_conflict` and `s3_after_the_daemon_exits_rpc_fails_with_internal` keep passing | ready, ids reserved |
| none | MVP gate | U2–U21 screenshots; cold start, keystroke-to-render p95, RSS with 10 idle Agents | QA and Driver, no Builder | the Phase 3 screenshot pack and the gate report | runs in parallel with UX work; not a blocker for it |

## UX enhancements (`scenarios/ux.md`)

Candidates left after S5, U34 to U36 (V4's App half, V5, V7, V8) and U30 (V1), U32 and U33 (V3), U31 (part of V2) and U27–U28 (most of V9) moved into `scenarios/ui.md`. Ordered by expected value. Observer for every row: `just check` green, the tests named in the scenario pass in Vitest against the fake App seam (`apps/desktop/src/testing/fakeApp.ts`), and QA captures the screenshots the scenario names. A row moves into `ui.md` under a reserved `U` id when it is dispatched.

| Order | PR key | Scenarios | Module | Owns | Merge after | Extra observer |
|---|---|---|---|---|---|---|
| 1 | `ux-keys` | V2 rest (Left and Right fold, F2, ⌘1 and ⌘2) | `apps/desktop` | `src/keys/**`, one line in `src/App.tsx` | U31 | QA drives the Rail by keyboard only |
| 3 | `ux-reopen-ui` | V4 (webview half; takes U37 when S5 is merged) | `apps/desktop` | `src/app/reopen*` | `ux-reopen-app` | QA kills the Daemon and reopens |
| 5 | `ux-empty` | V6 (amends U14, with its test and code) | `apps/desktop` | one small file per region, four directories | none | QA screenshots of a fresh Project |
| 8 | V9 leftovers | V9 minus U27–U28 | `apps/desktop` | `src/drawer/**` | reconcile with U27–U28 first | none |

The Architect has already moved each dispatched scenario's text into `ui.md` or `app.md`; a Builder edits only the files its row owns, never `scenarios/`, the README or this file, so parallel Builders cannot collide there.

A shared webview file (`src/app`, `src/state`, `src/ink`, `src/drawer`, `src/App.tsx`, `package.json`, `pnpm-lock.yaml`) is edited only as the scenario's row says. Any other change to one stops the Builder and is reported to the Architect.

## Deferred past the MVP (no scenario yet)

- motion.md rows other than the Drawer slide and drag;
- Terminals named from their first command;
- Screen 11's recent folders and its `claude` version / not-found line;
- the `● 1 below` line (U30 covers jumping, not the pinned line), provenance letters, the Todo `on` field and the Pad storage switch;
- Inbox, Messages, Routes, Extensions and history;
- packaging (a signed `.app`).

## Swarm protocol

Agents coordinate only through this repo: this file, `scenarios/`, PRs and their commit trailers. The WIP limit, the `Claimed-by:` line and the one-day claim expiry are queue policy added by the Architect, not `AGENTS.md` rules; change them here.

- **Claiming.** An item is claimed by a `Claimed-by: <agent-id> <date>` line under its row. Only the Architect pushes it to main (`AGENTS.md`: nobody else pushes there). A Builder requests a claim by being named in a dispatch; a Builder never claims by editing this file in its own PR, and does not start an item that has no claim. A claim with no branch or PR after a day is released by the Architect.
- **WIP limit.** At most 6 Builders work at once, counting every open PR that is not waiting on review. The Architect dispatches no seventh. `boxd` VMs are a further limit, `BOXD_MAX_VMS`, default 12 (`AGENTS.md`).
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
  | S5 | reopen, App half (V4) |
  | U34 | scrollback (V5) |
  | U35 | Todo triage (V7) |
  | U36 | Pad edit safety (V8) |
  | U37 | reserved: reopen, webview half (V4), after S5 |
  | U38 | reserved: empty states (V6, amends U14 with its test and code) |
  | L9–L14 | `boxd-swarm` (#73): L9 review, L10 input, L11 reboot, L12 swarm, L13 status, L14 kill; it also rewords L6 (secret) and L8 (cap) |
  | L15 | #66, `loop/boxd.sh check` and `bake` (merged) |

  Every Builder has ids now; the Architect reserves more on request. V1–V9 in `scenarios/ux.md` are the Architect's candidates and are not reserved to any Builder; `ux-drawer` overlaps V9, so whichever lands second drops the overlap.
- **Every item names scenarios and an observer.** An item with no scenario ids, or with no observer a Reviewer can run, is returned to the Architect. A new idea starts as a scenario in `scenarios/` (an Architect PR), then becomes a row here.
- **Stop conditions.** If `loop/out/PAUSED` exists, a limit was hit: stop and tell the user. A defect that needs a change in `contracts/`, `CONTEXT.md` or the App seam stops the Builder and goes to the Architect.

## Linux VMs and CI

Once `crates/desktop` is a workspace member, every `just check` compiles Tauri, which makes macOS CI slower. On Linux, Tauri needs WebKitGTK and its dev packages. Unverified here: the `ru-toolchain` snapshot is believed to have none of them, so a `boxd` `just check` would fail in every module until the snapshot is rebaked (`loop/boxd.sh bake`). Default until the user decides (open question): run Builders in local worktrees.
