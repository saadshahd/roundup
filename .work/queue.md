# Work queue

This is the queue until roundup can hold its own Todos (`docs/development-loop.md`); the rows are in `.work/queue/*.md`. One line per item: scenario ids, module, owned files and the observer that says the item is done (`AGENTS.md` rule 1). An item without scenario ids and an observer is not ready and is not dispatched. Scenarios with the same PR key ship as one PR in one worktree. Line counts are estimates, sized to the guide of about 2000 lines (#39).

Volatile state is not kept here (it goes stale within hours): no PR numbers, verdicts or merge status. Find a PR with `gh pr list --search "<id> in:title"`; `gh pr list --state merged` and `git log origin/main` are the record of what is done. A row names ids, an owner, owned files and an observer. Text for an id is its heading in `scenarios/` (`grep -n '^\*\*<id>[ .]' scenarios/*.md`); an id with no heading there is reserved in the id table below and its text arrives with its own PR, so check before dispatching it.

A Builder edits only the files its row owns, never `scenarios/`, the README or this file. Observer for every row: `just check` green and the item's own tests (named by its id prefix) pass; UI rows also have QA drive a `just harness <seed>` page.

The coverage audit: every scenario id in `scenarios/*.md` has a test with its lowercase id as a prefix, except the `L` ids, whose tests are shell scripts that name them `L<n>` (`loop/*.test.sh`), W1 (its observer is `just check`), F1 and F7 (records of runs of `spikes/spawn-boundary`), and the ids in flight. The ids above have exactly one test each. An audit Builder reads each of its scenarios clause by clause, adds one test per clause that no test asserts, adds none for a clause already covered, and puts a table of id, clause and test name in the PR. It adds no scenario text and changes no behavior; a clause that the code does not satisfy is a defect to report to the Architect, not to fix in the audit PR.

## In flight, Ready now, Waiting

Rows are in `.work/queue/<squad>.md`, one file per squad of `docs/squads.md`, and `.work/queue/architects.md` for rows no squad owns (contracts, spikes, rows that cross squads). Each file keeps the tables it has (In flight, Ready now, Waiting). To find a row, grep the id in `.work/queue/`.

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
- **How the Merger gates.** The Merger merges a PR only when rule 1 holds, all at once: CI `check` green; the scenario ids named in the PR have passing tests, in e2e where the scenario is end to end, and the item's named observer passes as well, never instead; an approval from a different id (for a PR of class `post`, `loop/rules.sh merge-ready` instead, and the independent verdict follows within 60 minutes, `AGENTS.md` rule 1 and L44 to L49); `loop/rules.sh trailers`, `size origin/main` and `vocab` clean; zero anti-slop findings. PRs merge in the order of their "merge after" column, and a stacked PR merges after the PR it stacks on. If main is red, the Merger stops and reverts; it never fixes forward (rule in `AGENTS.md`).
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
  | U51 | quick switcher |
  | U52 | first run: recent folders and the `claude` line |
  | U53 | Todos list motion |
  | U54 | keyboard help on `?` |
  | U55 | diagnose the empty-Terminal notch |
  | U56 | right-click menu: stop, remove |
  | U57 | no pane header |
  | U58 | an Agent's Pads under it |
  | U59 | the terminal blends with the app |
  | U60 | an Agent's Todos under it |
  | R11, R12 | #93, keystroke probe (merged) |
  | R13 | load-aware perf comparison |
  | A13 | symlinked agents directory |
  | A14 | hold an early Signal until the Agent is registered |
  | A15 | clean environment for every program the Daemon starts |
  | A16 | `rail.remove` |
  | T8 | `Todo.creator` |
  | S6 | first-run facts in the App seam (`recent_projects`, `claude_binary`) |
  | S7 | the smallest window (640 by 400) |
  | A17, A18, D5–D7, T9, P10 | #125, daemon squad robustness (A18 stacks after A14, #114) |
  | A19 | a removed node's id is never reused: written after A16 (#120) merges; reserved to the daemon squad |
  | C1 | dropped-call outcome (`scenarios/rpc.md`) |
  | L9–L14 | `boxd-swarm` (#73): L9 review, L10 input, L11 reboot, L12 swarm, L13 status, L14 kill; it also rewords L6 (secret) and L8 (cap) |
  | L15 | #66, `loop/boxd.sh check` and `bake` (merged) |
  | L16 | `review` replays an empty approval commit |
  | L17–L20 | loop hardening: run logs, swarm throttle, run events and retry, ref per prompt |
  | L21 | streamed agent run |
  | L22 | `review` keeps the branch's trailers across a merge |
  | L23 | VM tools |
  | L24 | agent search servers |
  | L25 | design critic run |
  | L26–L27 | `loop/wake.sh`: L26 `watch`, L27 `check`; architect-b's split of #133 |
  | L40 | `boxd check` does not depend on files outside the checkout (round cap on #140: amend; one more reject on the same clause retires it) |
  | U61–U79 | `scenarios/ui-rail.md`, writer-ui-rail |
  | U80–U99 | `scenarios/ui-surfaces.md`, writer-ui-surfaces |
  | U100–U129 | `scenarios/ui-*.md`, architect-b (U100 `ui-persist.md`, U101–U102 `ui-daily.md`, U103–U104 `ui-reload.md`, U105–U110 `ui-attention.md`, U112 `ui-shelf.md`) |
  | U130–U137 | `scenarios/ui-visual.md`, architect visual-system |
  | G1–G9 | `scenarios/worktrees.md`, architect-c (G1–G7 written; G8–G9 reserved for the UI half) |
  | B1–B29 | `scenarios/messages.md`, architect-b (B24–B27 from the anchor rules; B25, B26, B27 are spikes) |
  | L44–L49 | `scenarios/loop.md`: merge policy, `loop/rules.sh` `class`, `rounds`, `merge-ready`, `revert-due`, `dispatch` and stall kind f (rows below) |
  | L41–L42 | `scenarios/loop.md`: L41 `loop/rules.sh tokens` (row U130), L42 `loop/rules.sh delta` (done, with this reservation) |
  | H1–H29 | `scenarios/decisions.md` (H1–H10) and `scenarios/control.md` (H11–H17), permission Decisions and the control channel, architect-swarm (written; the UI half is architect-b's, from the `U` ranges) |
  | L43 | `loop/rules.sh ranges`: squad ranges hold only unused ids |
  | L57–L63 | `scenarios/loop.md`: L58 tests-frozen, L59 metrics, L60 friction, L61 VM start and upload checks, L62 reserved (one file per scenario id), L63 reject handling (L57 labels is the earlier id) |
  | E1–E9 | `scenarios/awareness.md`, architect-c (E1–E7 written; E8–E9 reserved) |
  | F1–F9 | `scenarios/spawn-boundary.md`, architect-c (F1–F5 and F7 written; F6 the Rail's `stray` badge; F8–F9 reserved) |
  | Y1–Y3 | UI verification: axe-core on the harness page, per-frame Drawer laws, the laws sheet; architect-c |

  Every Builder has ids; the Architect reserves more on request. `scenarios/ux.md` is gone: every candidate in it now has a `U` id above.
- **Every item names scenarios and an observer.** An item with no scenario ids, or with no observer a Reviewer can run, is returned to the Architect. A new idea starts as a scenario in `scenarios/` (an Architect PR), then becomes a row here.
- **Stop conditions.** If `loop/out/PAUSED` exists, a limit was hit: stop and tell the user. A defect that needs a change in `contracts/`, `CONTEXT.md` or the App seam stops the Builder and goes to the Architect.

## Linux VMs and CI

Once `crates/desktop` is a workspace member, every `just check` compiles Tauri, which makes macOS CI slower. On Linux, Tauri needs WebKitGTK and its dev packages. Unverified here: the `ru-toolchain` snapshot is believed to have none of them, so a `boxd` `just check` would fail in every module until the snapshot is rebaked (`loop/boxd.sh bake`). Default until the user decides (open question): run Builders in local worktrees.
