# Work queue: ui-rail

The rows this squad holds (`docs/squads.md`, "Moving the queue"). The protocol, the status rules and the `Ids`/`Holder` table stay in `.work/queue.md`; the rule for a row is the same as there.

## In flight

A Builder holds each row and its PR is in review. Nothing below is dispatched again.

| Item | Ids | Owns | Observer | Claimed by |
|---|---|---|---|---|
| spawn with a prompt (`⇧⌘N`) | U33 | `apps/desktop/src/rail/spawn*`, `Rail.tsx`, `u32_` and `u33_` tests | `u33_` tests pass; the U32 case "cmd and shift" keeps `⇧⌘T` doing nothing | boxd-agents |
| empty states | U38 | one small file per region in `src/rail`, `src/todos`, `src/pads`, `src/terminal` (`src/rail` shares `Rail.tsx` with U33: merges after it) | U2's tests; narrows U14's test | boxd-agents |
| the Drawer takes and gives back focus | U40 | `apps/desktop/src/drawer/**` | `u27_focus_already_in_another_field_is_left_alone`, the `u28_` tests | boxd-agents |

## Waiting

Scenario text is on main. The row starts when what it waits on has merged. A UI row ends its Item with `(Moves: <D ids>)`, the design-system checks it moves (`docs/design-system.md`, baseline protocol); the Builder's PR body repeats the line and corrects it if the diff moves other checks.

| Id | Item | Owns | Keeps green | Waits on |
|---|---|---|---|---|
| U41 | rename returns focus to the row (Moves: none) | `apps/desktop/src/rail/RailRow.tsx` (not `Rail.tsx`, which U33's row holds); starts from branch `architect/u41-rename-tdd` and does not edit `u41_rename_focus.test.tsx` | `u41_rename_focus.test.tsx` passes (4 of 5 red on main); `u9_`, `u140_` keep green | now |
| U22 | Rail drag edges: a release outside the Rail cancels, edge scroll while dragging; `.rail-tree` becomes the scroller (`flex: 1; min-height: 0; overflow-y: auto` beside the pinned bar, which moves out of it; `--rail-actions-height` moves to their common parent, a wrapper in `Rail.tsx` styled `display: flex; flex-direction: column; height: 100%`; `.rail-actions` stops being `position: sticky`) and `.rail` stops scrolling vertically (Moves: D1) | `apps/desktop/src/rail/drag.ts`, `dragTarget.ts`, `Rail.tsx`, `rail/styles.css`, `apps/desktop/src/styles.css`; starts from branch `architect/u22-drag-tdd` and does not edit `u22_drag_edges.test.tsx` | `u22_drag_edges.test.tsx` passes (red on main); `u22_drag`, `u22_drop_target`, `u44_` and `u31_` (scroll into view) keep green | now |
| U44, U46, U47, U49 | rail polish: rows under the pinned bar, hover jitter, `⌘T` selects the Terminal, Live line title (Moves: D1, D6) | `apps/desktop/src/rail/**` (one PR, so the four do not collide) | `u6_`–`u10_`, `u9_` tests, `u30_`, `u32_`, `u22_` | after U31 and U33 merge |
| U56 | right-click menu: stop, remove (Moves: D1, D6, D7, D8) | `apps/desktop/src/rail/menu/**`, one line each in `Rail.tsx`, `RailRow.tsx`, `src/rail/keys.ts` (created by U31) | `u9_`, `u31_`, `u33_` tests | after A16, U31, U33 and the rail polish PR (U44, U46, U47, U49); stacking allowed |
| U58 | an Agent's Pads under it, on demand (Moves: D1, D2, D4, D6) | `apps/desktop/src/rail/pads/**`, one line in `RailRow.tsx`, `src/pads/**`, one case in `src/keys/**` | `u20_` tests; narrows two named `u18_` tests | after U31, U33, U36, U41, U50 and U56; stacking allowed |
| U50 | the Pad's text fills its Drawer (Moves: D1, D2, D5) | `apps/desktop/src/pads/**` | `u20_` tests | after U38 merges (it adds a pads file) |
| U60 | an Agent's Todos under it (Moves: D1, D2, D4) | `apps/desktop/src/rail/pads/**` (shared with U58), `apps/desktop/src/todos/**` | `u15_`, `u17_`, `u35_`, U58's tests | after T8 on main; may stack on U58's branch |
| U41 | Meta-agent folding promised by U41 (Moves: D2, D6) | `apps/desktop/src/rail/layout.ts`, `RailRow.tsx`, `src/keys/**`; U100's saved layout should then accept Meta-agent ids | `u41_` Meta-agent keyboard and pointer cases, `u100_` reopen with a collapsed Meta-agent | now; coordinate with the U41 rename row in `RailRow.tsx` |
| U132, U133 | visual pass 2, colour and click targets: Kind tones, `--accent`, hover, pressed and focus states, 24 px hit areas; Rail menu button font uses a D2 type step (review finding on #267) | `apps/desktop/src/rail/**`, `apps/desktop/src/drawer/**`, `apps/desktop/src/todos/**`, `apps/desktop/src/pads/**` | every test of those folders; computed Rail menu button font size | after U130, U137, U35 and U36 merge |
