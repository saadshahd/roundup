# Work queue

This is the queue until roundup can hold its own Todos (`docs/development-loop.md`). One line per item: scenario ids, module, owned files and the observer that says the item is done (`AGENTS.md` rule 1). An item without scenario ids and an observer is not ready and is not dispatched. Scenarios with the same PR key ship as one PR in one worktree. Line counts are estimates, sized to the guide of about 2000 lines (#39).

Status of the MVP slice as of this file: everything below under "Merged" is on main. The agents stack is the critical path.

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

## Open

Every row points at a PR number; `gh pr list` is the truth when this file and it disagree.

| PR | Item | Scenarios | Module | Observer | State |
|---|---|---|---|---|---|
| #54 | `agents-stack` | A1–A8 | `crates/agents`, `crates/rup` | `just check` green; `a1_`…`a8_` tests pass | open, mergeable; fixes pushed, reviews in flight. Supersedes #17–#37 (still open, to be closed) |
| #55 | `agents-mvp` | A9–A12 | `crates/agents` | `just check` green; `a9_`…`a12_` tests pass | approved; stacked on #54, merges after it |
| #57 | `ui-drag` (stretch) | U22 | `apps/desktop` (`src/rail/drag*`) | `u22_` tests pass | open |
| #58 | swarm board and UX scenarios | none (docs) | `.work`, `scenarios` | reviewer approval; `loop/rules.sh vocab` | open, this PR |
| #59 | boxd `review` subcommand | L-series (`loop.md`) | `loop/` | `just check`; the `l<n>_` tests it names | open |
| none yet | `daemon-e2e` | D2–D4 | `crates/rup` (`tests/e2e*`) | `d2_`…`d4_` pass; D4 prints the measured MB | in flight; needs #55 merged to run |
| none yet | `app-dev` | S1 (the `just app` dev window) | `crates/desktop` / `justfile` | `just app <project>` opens a working window | in flight (builder-app-dev), no PR open |
| #60 | `ux-layout` | U23–U24 | `apps/desktop` | `u23_`, `u24_` tests pass | open |
| #61 | `ux-daemon` | U25 | `apps/desktop` | `u25_` test passes | open |
| #62 | `chrome-harness` | U26 | `apps/desktop` | `u26_` test passes; QA can drive the real App on a fake Daemon in Chrome | open |
| none yet | `ux-drawer` | U27–U28 | `apps/desktop` | `u27_`, `u28_` tests pass | in flight (builder-ux-drawer), no PR open; ids reserved |
| none yet | `ux-pads` | U29 | `apps/desktop` | `u29_` test passes | in flight (builder-ux-pads), no PR open; ids reserved |
| none | MVP gate | U2–U21 screenshots; cold start, keystroke-to-render p95, RSS with 10 idle Agents | QA and Driver, no Builder | the Phase 3 screenshot pack and the gate report | after #54, #55 and `daemon-e2e` (`ui-drag` excluded) |

Critical path: #54 → #55 → `daemon-e2e` → MVP gate.

## UX enhancements (`scenarios/ux.md`)

Start after the MVP gate unless the Architect says otherwise; ordered by expected value. Observer for every row: `just check` green, the `v<n>_` tests named in the scenario pass in Vitest against the fake App seam (`apps/desktop/src/testing/fakeApp.ts`), and QA captures the screenshots the scenario names. Every row is a candidate: the Architect confirms it before it is dispatched. V6 also edits U14 in the `ux-empty` PR, with the test `u14_with_nothing_selected_the_pane_is_empty` and the code, so main never disagrees with its scenarios.

| Order | PR key | Scenarios | Module | Owns | Est. lines | Merge after | Extra observer |
|---|---|---|---|---|---|---|---|
| 1 | `ux-jump` | V1 | `apps/desktop` | `src/rail/jump*` | 300–500 | MVP gate | QA screenshots `artifacts/ux/V1/` with ten Agents, before and after a jump |
| 2 | `ux-keys` | V2, V9 | `apps/desktop` | `src/keys/**`, plus one line in `src/App.tsx` that mounts it | 600–900 | MVP gate | QA drives the Rail and a Drawer by keyboard only |
| 3 | `ux-spawn` | V3 | `apps/desktop` | `src/rail/spawn*` | 300–500 | `ux-keys` (it uses the focus model) | none |
| 4 | `ux-reopen-app` | V4 (App half) | `crates/desktop` | `crates/desktop/**`, `scenarios/app.md` S1 | 200–400 | MVP gate | a Rust test named `s1_…` for a second `open_project` after exit |
| 5 | `ux-reopen-ui` | V4 (webview half) | `apps/desktop` | `src/app/reopen*` | 200–400 | `ux-reopen-app` | QA kills the Daemon and reopens |
| 6 | `ux-scrollback` | V5 | `apps/desktop` | `src/terminal/scroll*`, `src/terminal/emulator.ts` | 400–600 | MVP gate | QA RSS with ten Terminals printing 100 000 lines each |
| 7 | `ux-empty` | V6 | `apps/desktop` | one small file per region: `src/rail/empty*`, `src/todos/empty*`, `src/pads/empty*`, `src/terminal/empty*` (four directories, one module) | 200–400 | MVP gate | QA screenshots `artifacts/ux/V6/` of a fresh Project |
| 8 | `ux-todos` | V7 | `apps/desktop` | `src/todos/**` | 300–500 | MVP gate | none |
| 9 | `ux-pads` | V8 | `apps/desktop` | `src/pads/**` | 300–500 | MVP gate | none |

A shared webview file (`src/app`, `src/state`, `src/ink`, `src/drawer`, `src/App.tsx`, `package.json`, `pnpm-lock.yaml`) is edited only as the scenario's row says. Any other change to one stops the Builder and is reported to the Architect.

## Deferred past the MVP (no scenario yet)

- motion.md rows other than the Drawer slide and drag;
- Terminals named from their first command;
- Screen 11's recent folders and its `claude` version / not-found line;
- the `● 1 below` line (V1 covers jumping, not the pinned line), provenance letters, the Todo `on` field and the Pad storage switch;
- Inbox, Messages, Routes, Extensions and history;
- packaging (a signed `.app`).

## Swarm protocol

Agents coordinate only through this repo: this file, `scenarios/`, PRs and their commit trailers. The WIP limit, the `Claimed-by:` line and the one-day claim expiry are queue policy added by the Architect, not `AGENTS.md` rules; change them here.

- **Claiming.** An item is claimed by a `Claimed-by: <agent-id> <date>` line under its row. Only the Architect pushes it to main (`AGENTS.md`: nobody else pushes there). A Builder requests a claim by being named in a dispatch; a Builder never claims by editing this file in its own PR, and does not start an item that has no claim. A claim with no branch or PR after a day is released by the Architect.
- **WIP limit.** At most 6 Builders work at once, counting every open PR that is not waiting on review. The Architect dispatches no seventh. `boxd` VMs are a further limit of 4 (`AGENTS.md`).
- **One id per author, a different id per reviewer.** Every authored commit carries `Author-Agent: <id>`. The Reviewer's approval is an empty commit that carries only `Reviewed-by-Agent: <id>`, and its id differs from every `Author-Agent` in the PR. It is the newest commit: anything pushed after it needs a new approval. `loop/rules.sh trailers` reads `git log --no-merges`, so it does not see a merge of main made after the approval; the Merger checks that by hand (`git log --first-parent` shows no merge above the approval). A merge of main that touches only a lockfile gets a fresh empty `Reviewed-by-Agent` commit and no more. A Reviewer is never given the Builder's rationale (rule 5).
- **How the Merger gates.** The Merger merges a PR only when rule 1 holds, all at once: CI `check` green; the scenario ids named in the PR have passing tests, in e2e where the scenario is end to end, and the item's named observer passes as well, never instead; an approval from a different id; `loop/rules.sh trailers`, `size origin/main` and `vocab` clean; zero anti-slop findings. PRs merge in the order of their "merge after" column, and a stacked PR merges after the PR it stacks on. If main is red, the Merger stops and reverts; it never fixes forward (rule in `AGENTS.md`).
- **Stale branches.** A Builder whose branch is behind main merges `origin/main` into its branch (the repo's practice, for example on `builder/mcp`), runs `just check`, pushes, and asks for a new approval. A stacked branch merges its base's head first, then main. It never rebases a branch with an approval or a review in flight.
- **`Cargo.lock` and `pnpm-lock.yaml` conflicts.** Two PRs that add packages both rewrite the lockfile, and the merge leaves conflict markers inside it. Never edit them by hand. Take main's file and let the tool regenerate the branch's additions: `git checkout origin/main -- Cargo.lock && cargo update -w`, and `git checkout origin/main -- pnpm-lock.yaml && pnpm install`. Then commit the result with the merge.
- **Scenario ids are reserved, not taken.** When the Architect dispatches an item, it reserves that item's ids in a line under its row, `ids: U27–U28`, pushed to main like a claim. A Builder writes scenarios and tests only under its reserved ids and asks the Architect for more rather than picking the next free number. A Reviewer blocks a PR whose ids are not reserved to it, or that clash with an id in another open PR or on main. This is queue policy added by the Architect, like the claim line. Current allocation:

  | Ids | Holder |
  |---|---|
  | U23–U24 | #60 |
  | U25 | #61 |
  | U26 | #62 (harness; renumbered from U23) |
  | U27–U28 | `ux-drawer` (focus on close, Esc, the Drawer edge) |
  | U29 | `ux-pads` (the Pad row) |

  `ux-daemon` and `ux-layout` have no ids yet; the Architect reserves them on request. V1–V9 in `scenarios/ux.md` are the Architect's candidates and are not reserved to any Builder; `ux-drawer` overlaps V9, so whichever lands second drops the overlap.
- **Every item names scenarios and an observer.** An item with no scenario ids, or with no observer a Reviewer can run, is returned to the Architect. A new idea starts as a scenario in `scenarios/` (an Architect PR), then becomes a row here.
- **Stop conditions.** If `loop/out/PAUSED` exists, a limit was hit: stop and tell the user. A defect that needs a change in `contracts/`, `CONTEXT.md` or the App seam stops the Builder and goes to the Architect.

## Linux VMs and CI

Once `crates/desktop` is a workspace member, every `just check` compiles Tauri, which makes macOS CI slower. On Linux, Tauri needs WebKitGTK and its dev packages. Unverified here: the `ru-toolchain` snapshot is believed to have none of them, so a `boxd` `just check` would fail in every module until the snapshot is rebaked (`loop/boxd.sh bake`). Default until the user decides (open question): run Builders in local worktrees.
