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
| #67 | boxd.md corrections | docs |
| #69 | X8 made deterministic | X8 |
| #77, #92, #103 | loop: agent exec timeout 1800 s, the Reviewer VM replays the PR's commits, an empty approval commit replays | L5, L9, L16 |
| #79, #82 | architect: next batches, wording follow-ups | docs |
| #83 | `rpc`: a dropped call is `UNKNOWN_OUTCOME` | C1 |
| #84, #104 | `agents`: refuse a symlinked `.roundup/agents`; its test compares canonical paths | A13 |
| #85 | `audit-pads` coverage | P2, P3, P7 |
| #86 | `u45-chip`: `⌘J` chord rules and the chip's look | U45, U48 |
| #87 | `ux-todos`: Todo triage | U35 |
| #93, #95, #98 | perf: keystroke probe, macOS baselines, R12 pinned | R11, R12, K1–K3 |
| #96, #97, #99, #100, #101 | architect: U39 retired, R13 text, queue rows, R13 prompt, where a verdict is recorded | docs |
| #105, #107 | architect: D4 deadline and Builder prompt, U33 open-field rules | D4, U33 |
| closed | #90 (U39 hook, superseded by #93) and #91 (sweep-rupd-harness: its dedup grew the code, +52/−20, and net deletion was its only observer) | |

## Open

Every row points at a PR number where one exists; `gh pr list` is the truth when this file and it disagree.

| PR | Item | Scenarios | Module | Observer | State |
|---|---|---|---|---|---|
| #80 | spawn with a prompt (`⇧⌘N`) | U33 | `apps/desktop` (`src/rail/spawn*`) | `u33_` tests pass; the U32 case "cmd and shift" is narrowed to keep `⇧⌘T` doing nothing; `just check` green | Claimed-by: boxd-agents 2026-10-01. Spec is on main (#107). Resumes: rebase, a `u33_` test that a successful spawn ends the field, reuse `paneHarness` |
| #81 | Rail keyboard (V2 subset: up and down, a visible focus ring, Enter selects) | U31 | `apps/desktop` | `u31_` tests pass | in fix rounds after a boxd Reviewer's rejection: act only on keys whose target is a row (`↓` in the rename field, Enter on collapse or promote), and cover six untested guards |
| #88 | Pad edits never overwrite another Actor | U36 | `apps/desktop`: only `src/pads/**` | `u36_` tests pass; the u20 tests U36 names stay green | in fix rounds |
| #89 | reopen, App half | S5 | `crates/desktop` | `s5_` tests pass; `s1_` and `s3_` tests stay | in fix rounds |
| #94 | scrollback | U34 | `apps/desktop`: only `src/terminal/**` | `u34_` tests pass; QA RSS with ten Terminals | in VM review (conflict with #93 resolved) |
| #102 | load-aware perf comparison; amends rule 7 and `docs/perf.md` | R13 | `crates/perf`, `AGENTS.md` rule 7, `docs/perf.md` | `r13_` tests; the R1–R12 and K tests stay | in review |
| #106 | loop hardening: unique run logs and check VM names, slot wait, retry before the agent, per-prompt ref | L17–L20 | `loop/boxd.sh` | `loop/boxd.test.sh` | in review |
| none | D4 flake: one deadline, a readable failure, a state-based wait if a lost event is proven | D4 (a proven lost event: A14) | `crates/rup/tests/**` | the Builder reproduces first (about 30 runs, once under load); `just check` | ready: prompt `.work/prompts/d4-flake.md`; the Builder never edits the Daemon |
| none | MVP gate | U2–U21 screenshots; cold start, keystroke-to-render p95, RSS with 10 idle Agents | QA and Driver, no Builder | the Phase 3 screenshot pack and the gate report | runs in parallel with UX work; not a blocker for it |

## Next batch

Text for every id is in `scenarios/` (U37–U41 in `ui.md`, A13 in `agents.md`, C1 in `rpc.md`). A Builder edits only the files its row owns, never `scenarios/`, the README or this file. Observer for every row: `just check` green and the item's own tests (named by its id prefix) pass; UI rows also have QA drive a `just harness <seed>` page. "Starts" says when a row can be dispatched; rows marked now are on disjoint directories.

| Id | Item | Owns | Keeps green | Starts |
|---|---|---|---|---|
| audit-todos | coverage audit: T1, T2, T3, T5, T7 | `crates/todos/**` | existing tests | now; no PR yet |
| audit-perf | coverage audit: R1–R7 | `crates/perf/**` | existing tests | now; no PR yet |
| U41 | Rail by keyboard, the rest (V2) | `apps/desktop/src/keys/**`, one line in `src/App.tsx` | U31's and U32's tests | after U31 |
| U40 | the Drawer takes and gives back focus (V9) | `apps/desktop/src/drawer/**` | `u27_focus_already_in_another_field_is_left_alone`, the `u28_` tests; narrows `u27_with_no_terminal_shown_closing_the_drawer_focuses_nothing` | after U31 |
| U37 | reopen, webview half (V4) | `apps/desktop/src/app/**` | `u25_daemon_gone.test.tsx` for everything before a reopen | after S5 and U34 (the centre screen sits near `src/terminal`) |
| U38 | empty states (V6) | one small file per region in `src/rail`, `src/todos`, `src/pads`, `src/terminal`; narrows U14's test | U2's tests | after U34, U35 and U36 merge (they edit those directories) |

From the audit of main 9784d54 (ux-auditor-1; screenshots in `/tmp/ux2-shots/`). The audit's browser driver left Enter held down after `press Enter`, which faked U42 (a Todo Drawer loop); U43 (rename) was a double-click that landed on the Live line. Both are dropped. The auditor re-checked the rest with dispatched events and confirms U44, U45 and U46; U47 to U50 do not depend on key events. A create-then-open call-count test (one `todo.get`) may be added as a test-only guard. A double-click on the Live line does nothing, which U9 allows. Each Builder of a row below reproduces the defect first, with dispatched events, and stops and reports if it does not reproduce.

| Id | Item | Owns | Keeps green | Starts |
|---|---|---|---|---|
| U44, U46, U47, U49 | rail polish: rows under the pinned bar, hover jitter, `⌘T` selects the Terminal, Live line title | `apps/desktop/src/rail/**` (one PR, so the four do not collide) | `u6_`–`u10_`, `u9_` tests, `u30_`, `u32_`, `u22_` | after U31 and U33 merge |
| U50 | the Pad's text fills its Drawer | `apps/desktop/src/pads/**` | `u20_` tests | after U36 merges |
| held | contrast of Live lines (`--light` 2.57:1, `--lightest` 1.68:1) and the empty-xterm notch | shared color tokens in `src/styles.css` (a shared file); the notch needs a diagnosis | U4, U25 | held for the lead's decision; moving Live lines to `--grey` changes U4's palette |

The coverage audit: every scenario id in `scenarios/*.md` has a test with its lowercase id as a prefix, except the `L` ids, whose tests are shell scripts that name them `L<n>` (`loop/*.test.sh`), W1 (its observer is `just check`), and the ids in flight. The ids above have exactly one test each. An audit Builder reads each of its scenarios clause by clause, adds one test per clause that no test asserts, adds none for a clause already covered, and puts a table of id, clause and test name in the PR. It adds no scenario text and changes no behavior; a clause that the code does not satisfy is a defect to report to the Architect, not to fix in the audit PR.

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
  | U31 | #81, Rail keyboard |
  | U32 | #75, spawn shortcuts (merged) |
  | U33 | #80, spawn with a prompt, boxd-agents |
  | S5 | #89, reopen, App half |
  | U34 | #94, scrollback |
  | U35 | #87, Todo triage (merged) |
  | U36 | #88, Pad edit safety |
  | U37 | reopen, webview half |
  | U38 | empty states |
  | U39 | retired: superseded by #93's build-time probe (`just perf-keystroke`, K1 to K3, R11); #90 closed. Rule 7's keystroke-to-render number is #93's real-window p95 in WKWebView, one probe and one pairing rule (the first render after the emulator parsed the typed character itself); no second in-app hook that pairs a keystroke with any later output, because it can end early on unrelated output and so bounds nothing. The Daemon-side cheap bound already exists as `just perf`'s write-to-output limit (R8) |
  | U40 | the Drawer's focus |
  | U41 | Rail by keyboard, the rest |
  | U42 | dropped: a false positive (a stuck key in the test driver), never an app defect |
  | U43 | dropped: not a bug (the automated double-click hit the Live line, not the name) |
  | U44, U46–U47, U49 | rail polish |
  | U45, U48 | #86, `⌘J` rules and the chip (merged) |
  | U50 | Pad text fills its Drawer |
  | R11, R12 | #93, keystroke probe (merged) |
  | R13 | #102, load-aware perf comparison |
  | A13 | #84, #104, symlinked agents directory (merged) |
  | C1 | #83, dropped-call outcome (merged; `scenarios/rpc.md`) |
  | L9–L14 | `boxd-swarm` (#73): L9 review, L10 input, L11 reboot, L12 swarm, L13 status, L14 kill; it also rewords L6 (secret) and L8 (cap) |
  | L15 | #66, `loop/boxd.sh check` and `bake` (merged) |
  | L16 | #103, review replays an empty approval commit (merged) |
  | L17–L20 | #106 loop hardening, boxd-agents: L17 run logs, L18 slot wait, L19 retry and events, L20 per-prompt ref |
  | L21 | reserved: a streaming agent run for `loop/boxd.sh`, after #106 merges |
  | A14 | reserved: only if the D4 Builder proves a lost status event on the signal path (A5) |

  Every Builder has ids; the Architect reserves more on request. `scenarios/ux.md` is gone: every candidate in it now has a `U` id above.
- **Every item names scenarios and an observer.** An item with no scenario ids, or with no observer a Reviewer can run, is returned to the Architect. A new idea starts as a scenario in `scenarios/` (an Architect PR), then becomes a row here.
- **Stop conditions.** If `loop/out/PAUSED` exists, a limit was hit: stop and tell the user. A defect that needs a change in `contracts/`, `CONTEXT.md` or the App seam stops the Builder and goes to the Architect.

## Linux VMs and CI

Once `crates/desktop` is a workspace member, every `just check` compiles Tauri, which makes macOS CI slower. On Linux, Tauri needs WebKitGTK and its dev packages. Unverified here: the `ru-toolchain` snapshot is believed to have none of them, so a `boxd` `just check` would fail in every module until the snapshot is rebaked (`loop/boxd.sh bake`). Default until the user decides (open question): run Builders in local worktrees.
