# UX enhancements

Candidate scenarios for someone running ten Agents from one window, after the MVP's U1 to U21 are on main. They sit in `apps/desktop` (V4 also in `crates/desktop`) and build on `ui.md`; everything here uses CONTEXT.md terms and only methods and events in `contracts/generated/`. No scenario here changes `contracts/`. Ids are `V1` to `V9`, in expected-value order; a candidate that is dispatched moves into `ui.md` under a `U` id and leaves this file (V1 is U30, V3 is U32 and U33, V4's App half is S5, V5 is U34, V7 is U35, V8 is U36, and `↑`, `↓`, a focus ring and `Enter` from V2 are U31): the first ones decide whether ten Agents are usable at all, the last ones polish what already works.

Tests run in Vitest with jsdom against the fake App seam (`apps/desktop/src/testing/fakeApp.ts`), as in `ui.md`; the clock and the terminal emulator are injected. A scenario that changes what the window looks like also says what QA captures, to `artifacts/ux/<id>/<step>.png`, in the Chrome run against a fake Daemon (`docs/development-loop.md`, step 7). Chords are written `⌘<key>`; a ⌘ chord is handled by the webview and never reaches a Terminal. Every on-screen string quoted is a proposal for the Design critic to score, not a wireframe fact.

## V2: moving between ten Agents

**V2 Rail by keyboard (the rest).** `↑`, `↓`, `Enter` and the focus ring are U31; this is what U31 leaves. Given an open Project, focus is in exactly one place: the Rail, the pane, or an open Drawer. `⌘1` puts it in the Rail, `⌘2` in the pane. With focus in the Rail:
- `←` collapses a Group or Meta-agent (U8, decision 1), or selects its parent when it is already collapsed or a leaf; `→` expands, or selects its first child;
- `F2` edits the selected name in place, as double-click does in U9.

Nothing in the Rail takes keystrokes while focus is in the pane, so typing `j` into a Terminal never moves the selection. Selecting a row by keyboard shows its Terminal exactly as clicking does, and calls no method of its own.

## V4: when the Daemon is gone

**V4 reopen after the Daemon exits (webview half).** The App half is S5 (`scenarios/app.md`). Given an open Project whose Daemon ends (`daemon-exited`), then the Rail and the Shelf keep the last state they showed, nothing in them takes a click, and no call is made from them. The centre shows U2's `✕ daemon exited <code>` in ink, and under it a light `reopen`. Clicking `reopen` calls `open_project {path: <Project path>}`. When that succeeds, the three regions refetch and show the new Daemon's Rail, Shelf and Terminals; when it fails, `✕ <message>` replaces the line, with `reopen` still offered. It starts after S5 is merged and takes the next free `U` id.

## V6: the first minutes

**V6 empty states and the first Agent.** Given an open Project with no Rail nodes, no Todos and no Pads, then the Rail reads `no agents yet` / `⌘N starts one`, the `todos` list reads `no todos yet`, the `pads` list reads `no pads yet`, and the pane, with nothing selected, reads `select an agent or a terminal` (this amends U14's "the pane is empty" when `ux-empty` ships; that PR changes U14's text, its test `u14_with_nothing_selected_the_pane_is_empty` and the code together). These lines show only after the first `rail.tree`, `todo.list`, `pad.list` and `terminal.list` have answered, never as a flash before the data arrives, and each disappears as soon as its list holds one item. When a call that fills a region fails, that region shows `✕ <message>` in ink and not its empty line. U2's text for no open Project is unchanged.

## V9: Drawer

**V9 Drawer by keyboard.** Given a Drawer open from a Todo or a Pad, then focus moves into it, and `Esc` closes it and returns focus to where it was before it opened: the Rail row, the pane or a Shelf row. `Esc` while an inline field inside the Drawer is being edited closes only that field (U17, U20) and keeps the Drawer open. The terminal pane's size never changes (U5). A keystroke typed into the pane before the Drawer opened is not lost, and none reaches the pane while the Drawer holds focus.
