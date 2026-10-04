# Work queue

The queue until roundup holds its own Todos: rows live in `.work/queue/<squad>.md` (`docs/squads.md`, "Moving the queue"); this file keeps the protocol and the id table. A row names its scenario ids, module, owned files and an observer; a row missing ids or an observer a Reviewer can run is not dispatched. Rows sharing a PR key ship as one PR in one worktree.

No volatile state here (PR numbers, verdicts, merge status): `gh pr list --search "<id> in:title"` and `git log origin/main` are the record. An id's text is its heading in `scenarios/` (`grep -n '^\*\*<id>[ .]' scenarios/*.md`); an id with no heading is only reserved below.

A Builder edits only the files its row owns, never `scenarios/`, the README or this file. Every row's observer: `just check` green and the tests named by its ids pass; a UI row also has QA drive `just harness <seed>`.

**Coverage audit.** Every scenario id has a test prefixed with its lowercase id, except `L` ids (named `L<n>` in `loop/*.test.sh`), W1 (`just check`), F1 and F7 (records of `spikes/spawn-boundary` runs) and ids in flight. An audit Builder adds one test per clause no test asserts, with a table of id, clause and test in the PR; it changes no scenario or behaviour, and reports an unmet clause to the Architect.

## Deferred past the MVP (no scenario yet)

- motion.md rows other than the Drawer slide and drag;
- Terminals named from their first command;
- the `● 1 below` line (U30 covers jumping, not the pinned line), provenance letters, the Todo `on` field and the Pad storage switch;
- Inbox, Messages, Routes, Extensions and history;
- packaging (a signed `.app`).

Product roadmap: [Rooms, Doors and Sketches](../docs/wireframes.md#accepted-product-direction-rooms-doors-and-sketches); it needs scoped scenarios before dispatch.

## Swarm protocol

Agents coordinate only through this repo: queue files, `scenarios/`, PRs and their trailers.

- **Dispatch.** A squad's writer dispatches its own rows; rows in `.work/queue/architects.md` are an architect's. At most 12 Builders at once, counting every open PR not waiting on review.
- **Ids.** A squad allocates from its range in `docs/squads.md`; the architects reserve from theirs and record holders below. A Builder writes scenarios and tests only under its ids. A Reviewer blocks ids not held by the PR, or clashing with another open PR or `main`.
- **Merge order.** PRs merge in their rows' "merge after" order; a stacked PR (its scenario says "after X" or "may stack") branches from its base, touches no file its base touches, and merges after it.
- **Stale branches.** Merge `origin/main` into the branch (a stacked branch merges its base first), run `just check`, push, ask for a new approval; never rebase a branch with an approval or a review in flight.
- **Lockfile conflicts.** Take main's file and regenerate: `git checkout origin/main -- Cargo.lock && cargo update -w`, `git checkout origin/main -- pnpm-lock.yaml && pnpm install`; commit with the merge.
- **Out of scope.** A defect that needs a change in `contracts/`, `GLOSSARY.md` or the App seam stops the Builder and goes to the Architect.

**Id table** (who holds each reserved id):

| Ids | Holder |
|---|---|
| U23–U24 | `ux-layout` |
| U25 | `ux-daemon` |
| U26 | `chrome-harness` |
| U27–U28 | `ux-drawer` (focus on close, Esc, the Drawer edge) |
| U29 | `ux-pads` (the Pad row) |
| U30 | jump to what needs you |
| U31 | Rail keyboard, builder-ux-drawer |
| U32 | spawn shortcuts |
| U33 | spawn with a prompt, boxd-agents |
| S5 | reopen, App half, boxd-agents |
| U34 | scrollback, boxd-agents |
| U35 | Todo triage, boxd-agents |
| U36 | Pad edit safety, boxd-agents |
| U37 | reopen, webview half |
| U38 | empty states |
| U39 | retired: superseded by `just perf-keystroke` (K1 to K3, R11) |
| U40 | the Drawer's focus |
| U41 | Rail by keyboard, the rest |
| U42 | dropped: a test-driver stuck key, not an app defect |
| U43 | dropped: not a bug |
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
| R11, R12 | keystroke probe |
| R13 | load-aware perf comparison |
| V1–V7 | formal proofs of the Messages laws (`scenarios/proofs.md`); V4's pure `step` is the `V4` row of `.work/queue/architects.md`, M8 and M9 are its `V4b` row, both held by architect-b |
| A13 | symlinked agents directory |
| A14 | hold an early Signal until the Agent is registered |
| A15 | clean environment for every program the Daemon starts |
| A16 | `rail.remove` |
| T8 | `Todo.creator` |
| S6 | first-run facts in the App seam (`recent_projects`, `claude_binary`) |
| S7 | the smallest window (640 by 400) |
| A17, A18, D5–D7, T9, P10 | daemon squad robustness (A18 stacks after A14, ) |
| A19 | a removed node's id is never reused: written after A16  merges; reserved to the daemon squad |
| C1 | dropped-call outcome (`scenarios/rpc.md`) |
| L9–L14 | `boxd-swarm` : L9 review, L10 input, L11 reboot, L12 swarm, L13 status, L14 kill; it also rewords L6 (secret) and L8 (cap) |
| L15 | `loop/boxd.sh check` and `bake` |
| L16 | `review` replays an empty approval commit |
| L17–L20 | loop hardening: run logs, swarm throttle, run events and retry, ref per prompt |
| L21 | streamed agent run |
| L22 | `review` keeps the branch's trailers across a merge |
| L23 | VM tools |
| L24 | agent search servers |
| L25 | design critic run |
| L26–L27 | `loop/wake.sh`: L26 `watch`, L27 `check`; architect-b's split of |
| L40 | `boxd check` does not depend on files outside the checkout |
| U61–U79 | `scenarios/ui-rail.md`, writer-ui-rail |
| U80–U99 | `scenarios/ui-surfaces.md`, writer-ui-surfaces |
| U100–U129 | `scenarios/ui-*.md`, architect-b (U100 `ui-persist.md`, U101–U102 `ui-daily.md`, U103–U104 `ui-reload.md`, U105–U110 `ui-attention.md`, U112 `ui-shelf.md`) |
| U130–U137 | `scenarios/ui-visual.md`, architect visual-system |
| U138–U141 | `scenarios/ui-keyboard.md`, architect-c (ux-auditor-1's keyboard sweep: complete focus, Todo edit keys, F2 on the focused row, Pad append scroll) |
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
