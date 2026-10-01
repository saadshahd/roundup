# UX enhancements

Candidate scenarios for someone running ten Agents from one window, after the MVP's U1 to U21 are on main. They sit in `apps/desktop` (V4 also in `crates/desktop`) and build on `ui.md`; everything here uses CONTEXT.md terms and only methods and events in `contracts/generated/`. No scenario here changes `contracts/`. Ids are `V1` to `V9`, in expected-value order; a candidate that is dispatched moves into `ui.md` under a `U` id and leaves this file (V1 is U30, V3 is U32 and U33, and `↑`, `↓`, a focus ring and `Enter` from V2 are U31): the first ones decide whether ten Agents are usable at all, the last ones polish what already works.

Tests run in Vitest with jsdom against the fake App seam (`apps/desktop/src/testing/fakeApp.ts`), as in `ui.md`; the clock and the terminal emulator are injected. A scenario that changes what the window looks like also says what QA captures, to `artifacts/ux/<id>/<step>.png`, in the Chrome run against a fake Daemon (`docs/development-loop.md`, step 7). Chords are written `⌘<key>`; a ⌘ chord is handled by the webview and never reaches a Terminal. Every on-screen string quoted is a proposal for the Design critic to score, not a wireframe fact.

## V2: moving between ten Agents

**V2 Rail by keyboard.** Given an open Project, focus is in exactly one place: the Rail, the pane, or an open Drawer. `⌘1` puts it in the Rail, `⌘2` in the pane. With focus in the Rail:
- `↑` and `↓` select the previous and next visible row, and stop at the ends;
- `←` collapses a Group or Meta-agent (U8, decision 1), or selects its parent when it is already collapsed or a leaf; `→` expands, or selects its first child;
- `Enter` moves focus to the pane, which is U12's typing target;
- `F2` edits the selected name in place, as double-click does in U9.

Nothing in the Rail takes keystrokes while focus is in the pane, so typing `j` into a Terminal never moves the selection. Selecting a row by keyboard shows its Terminal exactly as clicking does, and calls no method of its own.

## V4: when the Daemon is gone

**V4 reopen after the Daemon exits.** Given an open Project whose Daemon ends (`daemon-exited`), then the Rail and the Shelf keep the last state they showed, nothing in them takes a click, and no call is made from them. The centre shows U2's `✕ daemon exited <code>` in ink, and under it a light `reopen`. Clicking `reopen` calls `open_project {path: <Project path>}`. When that succeeds, the three regions refetch and show the new Daemon's Rail, Shelf and Terminals. When it fails, `✕ <message>` replaces the line, with `reopen` still offered. This needs one change at the App seam: S1's `CONFLICT` for a second `open_project` applies only while a Daemon runs, and after `daemon-exited` it starts a new one. That is an edit to `scenarios/app.md` S1 and to `crates/desktop`, shipped first as its own PR with Architect approval for the seam edit; the webview half follows. No Daemon contract changes.

## V5 to V6: heavy output and the first minutes

**V5 bounded scrollback.** Given ten Terminals that each print 100 000 lines, then every emulator holds at most 10 000 lines of scrollback (one named constant), so memory is steady and not growing with output (rule 7's 150 MB budget for ten idle Agents holds under heavy output; the gate measures the real number). When the user has scrolled up in a Terminal, new output never moves the view, and a light `↓ latest` shows at the bottom of the pane; clicking it, or typing, returns to the newest line and hides it. `terminal.output` is still handed to the emulator in order whether or not its row is selected (U11). The injected emulator in `src/terminal/emulator.ts` gains scroll position and scrollback size; that is a change inside the terminal module, not at the App seam.

**V6 empty states and the first Agent.** Given an open Project with no Rail nodes, no Todos and no Pads, then the Rail reads `no agents yet` / `⌘N starts one`, the `todos` list reads `no todos yet`, the `pads` list reads `no pads yet`, and the pane, with nothing selected, reads `select an agent or a terminal` (this amends U14's "the pane is empty" when `ux-empty` ships; that PR changes U14's text, its test `u14_with_nothing_selected_the_pane_is_empty` and the code together). These lines show only after the first `rail.tree`, `todo.list`, `pad.list` and `terminal.list` have answered, never as a flash before the data arrives, and each disappears as soon as its list holds one item. When a call that fills a region fails, that region shows `✕ <message>` in ink and not its empty line. U2's text for no open Project is unchanged.

## V7 to V9: Shelf and Drawer

**V7 Todo triage without the Drawer.** Given open Todos, then hovering an open row shows a light `complete`, which calls `todo.complete {id}`. It logs no read Touch and does not open the Drawer. On a blocked Todo's second line (`waits on #4, #5`), each `#<id>` is a click target that opens that blocker's Drawer (U17). The list is still driven by events, as in U15: the row changes only when `todo.updated` and `todo.unblocked` arrive and `todo.list` is refetched. A failed `todo.complete` shows `✕ <message>` in place of the row's second line until the next click.

**V8 a Pad edit never overwrites another Actor's change.** Given a Pad the user owns and is editing in its Drawer (U20), when `pad.changed` for that Pad arrives, then the draft stays untouched (U20) and a light `changed by <name>` line appears, with `keep mine` and `use theirs`. `<name>` is `nameOf` of the event's `actor`, and nothing is read from the Daemon for it: `pad.read` would log a Touch. Leaving the field with a changed draft does not call `pad.write` while that line shows. `keep mine` calls `pad.write` with the draft; `use theirs` replaces the draft with the text from `pad.list` and calls nothing else. Without an intervening change, leaving the field calls `pad.write` once, as in U20.

**V9 Drawer by keyboard.** Given a Drawer open from a Todo or a Pad, then focus moves into it, and `Esc` closes it and returns focus to where it was before it opened: the Rail row, the pane or a Shelf row. `Esc` while an inline field inside the Drawer is being edited closes only that field (U17, U20) and keeps the Drawer open. The terminal pane's size never changes (U5). A keystroke typed into the pane before the Drawer opened is not lost, and none reaches the pane while the Drawer holds focus.
