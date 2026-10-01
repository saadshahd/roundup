# UI

The webview in `apps/desktop`: Solid, Vite and xterm.js with the WebGL addon (ADR 0005), drawn from `docs/wireframes.md` (its corrections and decisions win over the drawings) and timed by `docs/motion.md`. It is a client of the Daemon through the App seam (`app.md`) and nothing else. Tests run in Vitest with jsdom against a fake App seam. Only one adapter file imports Tauri; it belongs to U1, and every other scenario reaches Tauri through it. The terminal emulator and the clock are injected, so no test needs a display, a GPU or real time. Exported names say Project, never folder: `folder` is the _Avoid_ word for Group. On-screen text may say folder, because the wireframes do.

Methods and events are the ones in `contracts/generated/methods.ts` and `Event.ts`. The text a scenario quotes is what the screen shows; capitals in the wireframes stand for ink, and the app never uppercases.

## Shell (U1 to U5, U23, U24)

**U1 calls and events.** Given the App seam, when the webview calls a method named in `RpcMethods`, then its params and its result are typed from `contracts/generated/methods.ts`. A failed call rejects with the Daemon's `code` and `message`. Every Event reaches every subscriber in the order the Daemon sent it.

The adapter file is the only door to Tauri, and it offers four things besides calls and events:
- listening for `daemon-exited`;
- the macOS folder chooser;
- the macOS save chooser, with a suggested file name;
- setting the Dock badge count, where zero clears it.

Each has a fake and a test here. U2, U10 and U21 use them, so no later PR edits this file.

**U2 first run.** Given no open Project (Screen 11), then the Rail region reads `agents and terminals` / `appear here, one per row,` / `nested by indent`. The centre reads `open a folder to start` and `choose folder…`. The Shelf reads `todos` / `agents add them as they` / `plan; so can you`, then `pads` / `◈ agent notes` / `◇ yours`. When a folder is chosen in the macOS chooser, or the App already has a Project (`project`), then the header reads `roundup   <Project name>` and the three regions show that Project. When `open_project` fails, `✕ <message>` replaces `open a folder to start` and is the only ink in the window. On `daemon-exited`, see U25.

**U3 Rail state.** Given an open Project, then the webview subscribes before its first `rail.tree` and `terminal.list` and keeps the Rail state current. `rail.changed` refetches the tree. `agent.status` replaces one node's Status. `terminal.exited` marks the node with that `terminal_id` as exited with its code (`null` when a signal ended it). A node with no `terminal_id` counts as exited with no known code. Events that arrive while a `rail.tree` call is in flight are applied after it, never dropped. At most one node is selected (none at start), and a node that leaves the tree is no longer selected. `nameOf(actor)` gives `you` for the user, the Rail name for an Agent's id, and otherwise the id.

**U4 ink.** A Kind maps to a Glyph: `needs-you` `●` in bold amber, `error` `✕` in bold red, `blocked` `⏸` grey, `working` `○` light grey, `idle` `·` light grey, `done` `✓` lightest grey. Only `needs-you` and `error` carry Ink. The most urgent of several Kinds follows the order in `CONTEXT.md` (`error` first). Elapsed time since a `since` reads `just now` under a minute, then `4m`, `2h`, `3d`, against the injected clock.

**U5 Drawer and last touch.** Given the main workspace, when a Drawer opens, then it slides in from the right over the terminal and the Shelf (about 180 ms, ease-out; instant under `prefers-reduced-motion`) with a light `close` word. One Drawer is open at a time, and the terminal pane's size never changes (its size observer reports nothing).

For an item (`todo:<id>` or `pad:<name>`), the Drawer calls `provenance.history {item}` before its own read of the item. The last-touch line then reads `last  <name> <read|wrote> <hh:mm>`, from the newest Touch in that first fetch. Opening the Drawer is itself a read Touch by the user, and fetching history first keeps that read out of the line, so it never reads "you read" at the moment of opening.

**U23 column widths.** Given the main workspace, then the Rail is 20% of the window wide, never under 252px (its three action words fit on one line) nor over 320px, the Shelf is 14%, never under 160px nor over 280px, and the terminal is never under 690px. That floor is 80 text columns of the system monospace face at 13px (8px each: 80 x 8, plus the 14px xterm scrollbar and 32px of padding is 686px); the app's font stack resolves to that face in WKWebView. From 1102px wide the terminal therefore shows at least 80 columns. A name too long for the Rail ends in `…`.

**U24 narrow window.** Given a window under 1102px, then the Shelf moves under the Rail in the left column (the Rail takes the upper 65% of the height, the Shelf the lower 35%, each scrolling on its own) and the terminal takes the full height on the right, so the Shelf's lists, its `+` words and the Todo and Pad Drawers stay reachable. Under 942px the terminal gives width and the Rail stays at 252px. A Drawer never covers the Rail: it is at most the window width minus 252px.

## Rail (U6 to U10, U32)

**U6 rows.** Given a Rail tree, then each node is one row, indented 2 per depth under its parent, in `order`. An Agent or a Meta-agent shows its Status Glyph. A plain Group shows `▾`. A Terminal shows `○` while it runs and `✓` once it has exited. The selected row has a faint band. Rows never reorder when a Status changes. Clicking a row selects it.

**U7 live line.** Given an Agent or Meta-agent row, then its Live line is the Status label and its elapsed time (`asks: keep v1 routes?  4m`). A running Terminal has no Live line. An exited Terminal's Live line depends on how it ended:
- `exited <code>` when the code is known;
- `exited by signal` when the code is `null` (A3's label);
- `exited` when the node has no `terminal_id`.

The Live line is hidden, shows on hover or selection, and shows unprompted when the Kind is `blocked`, `needs-you` or `error`. For `needs-you` and `error`, the name and the Glyph are also in Ink.

**U8 folding.** Agents that have been `done` for 10 minutes or more fold into one `✓ n done` line per parent, after their siblings, and a click unfolds them (wireframes decision 2). Clicking a Group's `▾` collapses it: the row then shows its most urgent descendant's Glyph and Ink, plus a light child count, and another click expands it (decision 1).

**U9 actions.** `+ agent` calls `agent.spawn {cwd: <Project path>, prompt: null, parent}`. `parent` is the selected Group or Meta-agent, else `null`, and the new row becomes selected, so its terminal shows. `+ terminal` calls `rail.spawnTerminal {cwd: <Project path>, parent}` with the same parent rule. `+ group` calls `rail.createGroup {name: "group", parent}`. Double-clicking a name edits it in place: Enter calls `rail.rename`; Esc or an empty name keeps the old one. Hovering a plain Group shows a light `promote`, which calls `rail.promote`. A failed call shows one line `✕ <message>` in Ink above `+ agent  + terminal  + group` until the next click. Rows come only from `rail.tree`; the webview never adds one itself.

**U10 Dock badge.** The Dock badge shows how many Agents have Kind `needs-you` or `error`, and there is no badge at zero (decision 4). The badge is set through U1's adapter.

**U32 spawn shortcuts and pinned actions.** Given an open Project, then `⌘N` does what `+ agent` does (U9) and `⌘T` does what `+ terminal` does, with the same parent rule and the same `✕ <message>` line on failure. A ⌘ chord is handled by the webview and never reaches a Terminal, and Caps Lock does not change it; the same keys with Shift, Ctrl or Alt, or without ⌘, do nothing, except `⇧⌘N`, which is U33; `⇧⌘T` still does nothing. Neither chord acts while a spawn call is in flight or after `daemon-exited`. The Rail's `+ agent  + terminal  + group` line stays at the bottom edge of the Rail while its rows scroll, so the actions are reachable with ten Agents.

**U33 spawn with a prompt.** Given an open Project, when the user presses `⇧⌘N`, then an inline prompt field opens at the top of the Rail, focused. Enter calls `agent.spawn {cwd: <Project path>, prompt: <text>, parent}` with U9's parent rule, and an empty field calls it with `prompt: null`. The new row becomes selected and focus goes to the pane, so the user can type at once. Esc closes the field and calls nothing. A failed call shows U9's `✕ <message>` line and keeps the field's text. This narrows U32: its test case "cmd and shift" in `apps/desktop/src/rail/u32_spawn_shortcuts.test.tsx` must exclude `⇧⌘N` (and keep `⇧⌘T` doing nothing) in the same PR. As in U32, the chord is handled by the webview and never reaches a Terminal, Caps Lock does not change it, other modifier sets do nothing, and it does not act while a spawn call is in flight or after `daemon-exited`. `⌘N` is unchanged: it still spawns with no prompt and no field (U32).

## Terminal pane (U11 to U14, U34)

**U11 one emulator per Terminal.** Given an open Project, when `terminal.output {id, data}` arrives, then its base64-decoded bytes are handed, in order, to the emulator for Terminal `id`. That emulator is created on the Terminal's first output, whether or not its row is selected. `terminal.output` is never replayed, so an emulator created later would show a blank pane. Selecting a row shows its Terminal's emulator and no other. Tests observe the bytes handed to an injected emulator.

**U12 typing.** Given the selected Terminal runs, when the user types into the pane, then each keystroke's bytes go to `terminal.write {id, data}` (base64), in order. An exited Terminal takes no input.

**U13 size.** Given the pane shows a Terminal, then `terminal.resize {id, cols, rows}` is called with the emulator's fitted size: when that Terminal is first shown, and whenever the window resizes, at most once per animation frame, until `daemon-exited` (U25). Opening a Drawer never resizes it. The WebGL renderer is used when the webview grants a WebGL context, else the DOM renderer, with one warning logged.

**U14 pane header.** The pane's top line depends on the selected row:

| Selected | Header |
|---|---|
| An Agent | `<name>  <kind> <elapsed>` (`auth-refactor  working 12m`) |
| A running Terminal | `<name>` alone |
| An exited Terminal | `<name>  ` followed by its U7 wording: `exited <code>`, `exited by signal` or `exited` |

While the program runs, a light `stop` sits at the right: for an Agent it calls `agent.stop {id}`, for a Terminal `terminal.kill {id: <terminal_id>}`. With an open Project and nothing selected, the pane is empty.

**U34 bounded scrollback.** Given Terminals that print without end, then every emulator keeps at most 10 000 lines of scrollback, one named constant that the real xterm emulator is constructed with, so memory stays steady however much a Terminal prints; rule 7's 150 MB budget for ten idle Agents holds under heavy output, and the MVP gate measures the real number. `terminal.output` is still handed to the emulator in order whether or not its row is selected (U11). When the user has scrolled up in a Terminal, new output never moves the view, and a light `↓ latest` shows at the bottom of the pane; clicking it, or typing into the pane (U12), returns to the newest line and hides it. The `Emulator` type in `src/terminal/emulator.ts` gains what the pane needs to know whether the view is at the bottom, to hear scrolling, and to return to the newest line; the fake emulator in `paneHarness.tsx` gains the same, and the pane is tested through it. The bound itself is tested without a display, on the options the real emulator is built with. No existing test is narrowed.

## Todos (U15 to U17, U35)

**U15 list.** Given Todos, then the Shelf's `todos` list shows the open ones in id order as `<glyph> #<id> <title>`. An open Todo is `·`. A blocked one is `⏸`, with a second line naming each open blocker in id order (`waits on #4, #5`). Done Todos fold into one `✓ n done` line that unfolds on click. On `todo.created`, `todo.updated`, `todo.unblocked` and `todo.deleted`, the list refetches `todo.list`, which logs no Touch.

**U16 create.** Clicking the `+` beside `todos` opens one inline title field at the top of the list. Enter calls `todo.create {title}`, and the row appears through the event. Esc or an empty title closes the field and calls nothing. A failure shows `✕ <message>` in place of the field.

**U17 detail.** Clicking a Todo opens its Drawer, which shows:
- `#<id>  <title>` with its Glyph;
- `waits on`, with each blocker and its own Glyph;
- `blocks`, with each Todo it blocks;
- `+ blocker`, which offers the other Todos and calls `todo.setBlockers` (a `CONFLICT` shows inline);
- the body;
- the last-touch line (U5);
- light `complete` and `delete` words (`todo.complete`, `todo.delete`).

Double-clicking the title or the body edits it, and leaving the field calls `todo.update` once if the text changed. Opening the Drawer calls `provenance.history {item: "todo:<id>"}` and then `todo.get` once; that `todo.get` is the one read Touch. Background refreshes never call `todo.get`.

**U35 Todo triage without the Drawer.** Given open Todos, then hovering or focusing an open row shows a light `complete`, which calls `todo.complete {id}`. It logs no read Touch and does not open the Drawer. On a blocked Todo's second line (`waits on #4, #5`), each `#<id>` is a click target that opens that blocker's Drawer (U17), and that click never also opens the blocked Todo's own Drawer. The list is driven by events as in U15: a row changes only when `todo.updated` and `todo.unblocked` arrive and `todo.list` is refetched. A failed `todo.complete` shows `✕ <message>` in place of the row's second line until the next click. Narrowing: the text of a row at rest stays exactly `<glyph> #<id> <title>`, so `complete` is not part of the row's text until it is hovered or focused, and the second line's text stays exactly `waits on #<id>, #<id>` with nothing added (`u15_todo_list.test.tsx` asserts `waits on #4, #6` and `waits on #6`).

## Pads (U18 to U21, U29, U36)

**U18 list and ownership.** Given Pads, then the Shelf's `pads` list shows each by name with an owner mark: `◈` when an Agent owns it, `◇` when the user does. The owner mark is not a Glyph; Glyphs mark Kinds. Clicking `◈` calls `pad.setOwner {name, owner: <user>}`, and the mark becomes `◇`. On `pad.changed`, the list refetches `pad.list`, which logs no Touch.

**U19 create.** Clicking the `+` beside `pads` opens an inline name field. Enter calls `pad.create {name}`. A `CONFLICT` or `INVALID_PARAMS` shows `✕ <message>` in place of the field.

**U20 open and edit.** Clicking a Pad opens its Drawer: its owner mark and name, then `owned by <owner name>`; a light `export .md`; the text as markdown source in the monospace face; and the last-touch line (U5). Opening calls `provenance.history {item: "pad:<name>"}` and then `pad.read` once; that `pad.read` is the one read Touch. When the user owns the Pad, the text is editable, and leaving the field calls `pad.write` once if the text changed. When an Agent owns it, the text is read-only and an `append` field calls `pad.append`. A `pad.changed` for the open Pad, while the user is not editing it, refreshes its text from `pad.list`.

**U21 export.** `export .md` opens the macOS save chooser, through U1's adapter, with `<name>.md` filled in. Choosing a path calls `pad.export {name, path}`, and cancelling calls nothing. An error shows `✕ <message>` in the Drawer.

**U29 long names.** A Pad whose name is longer than its column still keeps one line in the Shelf's `pads` list: its owner mark and name stay together on that line, and the name is cut with `…` and carries its full name as a tooltip. The Drawer's first line does the same, and `export .md`, `close` and the name never overlap.

**U36 a Pad edit never overwrites another Actor's change.** Given a Pad the user owns and is editing in its Drawer (U20), when `pad.changed` for that Pad arrives from an Actor other than the user, then the draft stays untouched (U20) and a light `changed by <name>` line appears, with `keep mine` and `use theirs`. `<name>` is `nameOf` of the event's `actor`, and nothing is read from the Daemon for it, because `pad.read` would log a Touch. The line shows only while the draft differs from the text it started from, which is the last text the Drawer loaded or saved. A draft equal to that text is clean: another Actor's change then refreshes the field as U20 says, with no line, and a draft typed back to that text is clean again, so its line goes and U20's refresh applies. While the line shows, leaving the field does not call `pad.write`. `keep mine` calls `pad.write` with the draft; `use theirs` replaces the draft with the text from `pad.list` and calls nothing else. A `pad.changed` whose actor is the user, which is the echo of the user's own write, never shows the line. Without another Actor's change, leaving the field calls `pad.write` once, as in U20, which this scenario qualifies and no more. Narrowing, test by test:
- `u20_a_change_skipped_while_editing_is_caught_up_after_the_write_settles` is replaced, because its end state (an Agent's line overwritten by the user's blur) is what this scenario forbids. The replacement keeps its setup: the user owns `mine`, edits it to `mine, edited`, an Agent-actor `pad.changed` arrives with `mine\nagent line`. It now expects the `changed by` line, no `pad.write` on blur, the field still `mine, edited` and the stored text still `mine\nagent line`; then `keep mine` calls `pad.write` and the stored text becomes `mine, edited`, while `use theirs` instead makes the field and the stored text `mine\nagent line` with no write. Its sibling `u20_a_change_skipped_while_editing_refreshes_on_blur_when_nothing_was_typed` is unchanged.
- `u20_typing_back_to_the_original_text_never_sends_it_over_an_agents_append` keeps passing. At the Agent's change the draft is dirty (`mine, typing` against `mine`), so the line shows; typing back to `mine` makes it clean, which clears the line and catches up the skipped change at that input, not only on blur, so the field refreshes to `mine plus agent` and no `pad.write` happens.
- `u20_clicking_back_in_while_a_save_is_in_flight_never_loses_an_agents_append` keeps passing. At the Agent's change the draft is clean (just saved), so there is no line and U20's refresh applies.
- `u20_pad_changed_while_the_user_is_editing_leaves_the_text_alone`, `u20_a_write_reply_never_overwrites_text_typed_after_a_quick_refocus` and `u20_a_pad_handed_over_while_the_cursor_is_in_the_text_saves_what_is_typed` keep passing as written.

## Drawer keys (U27 to U28)

**U27 focus returns.** Given a selected Agent or Terminal whose pane had the keyboard, when its Drawer closes (U5), keyboard focus goes back to that Terminal, so typing reaches it without a click. With no Terminal shown, or with focus already in another field, focus is left alone.

**U28 Esc closes.** While a Drawer is open, Esc closes it. Esc inside an inline field or a Pad's text field is that field's own key (U16, U19) and the Drawer stays open; Esc inside a Terminal goes to the program, and a closed Drawer does nothing with it.

## Jump to what needs you (U30)

**U30 jump.** Given a Rail with Agents or Meta-agents of Kind `needs-you` or `error`, then the header reads `<n> need you` after the Project name, where `<n>` is the count U10 puts on the Dock badge, and nothing is shown at zero. Pressing `⌘J`, or clicking that text, selects the most urgent of them: Kind `error` before `needs-you`, and within a Kind the one whose Status `since` is oldest, ties in the Daemon's order. Its row scrolls into view and its Terminal shows; any selection that lands under a collapsed Group expands that Group, so the row is never hidden. Doing it again selects the next in that order and wraps after the last, and from a selection outside that order it selects the first. With none, `⌘J` changes nothing. Terminals never count and the Rail never reorders (wireframes decision 3).

## Later UX (U37 to U50)

Ordered by what each needs; each names its owner files in `.work/queue.md`.

**U37 reopen after the Daemon exits.** Needs S5 (`scenarios/app.md`). Given U25's state after `daemon-exited`, then the header reads as U25 says and, after the exit text, a light `reopen`. Clicking it calls `open_project {path: <Project path>}`. When that succeeds, the header returns to `roundup   <Project name>`, the Rail, the Shelf and the pane are enabled again, every region refetches (`rail.tree`, `terminal.list`, `todo.list`, `pad.list`), and the webview subscribes again so the new Daemon's Events arrive; the greyed Rail shows the new Daemon's tree, never a mix. When it fails, `✕ <message>` replaces the exit text in Ink and `reopen` is still offered. Narrowing U25: "that is the only place the exit shows" now includes `reopen`; its disabled controls come back on success; `u25_daemon_gone.test.tsx` keeps passing for everything before a reopen.

**U38 empty states and the first Agent.** Given an open Project with no Rail nodes, no Todos and no Pads, then the Rail reads `no agents yet` / `⌘N starts one`, the `todos` list reads `no todos yet`, the `pads` list reads `no pads yet`, and the pane with nothing selected reads `select an agent or a terminal` and shows no header or `stop`. These lines show only after the first `rail.tree`, `todo.list`, `pad.list` and `terminal.list` have answered, never as a flash before the data arrives, and each disappears as soon as its list holds one item. When a call that fills a region fails, that region shows `✕ <message>` in Ink and not its empty line. U2's text for no open Project is unchanged. Narrowing U14: "the pane is empty" becomes the line above, and `u14_with_nothing_selected_the_pane_is_empty` is renamed and changed in the same PR as the code. Any existing test that asserts an empty list shows nothing is narrowed to the new line and named in the PR.

**U39 keystroke-to-render timing.** Given the webview loaded with `?perf` in its URL, then every keystroke typed into the pane (U12) is stamped at its `keydown`; the next `terminal.output` Event that arrives after it, once the Event's handlers have run and one animation frame has passed, closes the stamp, and the elapsed milliseconds from `performance.now()` are appended to `window.__perf.keystrokeToRender`. `window.__perf.p95()` returns the 95th percentile of those values, `null` while there are none. A keystroke whose output never arrives within 1 second is dropped from the list and counted in `window.__perf.unanswered`. Without `?perf` nothing is recorded, no `__perf` global exists and no listener is installed. The screen looks the same either way. The measurement is a bound, not a pixel readout: it ends at the frame after the handlers, which is what rule 7's 16 ms p95 budget is judged by in WKWebView (the MVP gate reads it on the laptop). Tests drive the fake App seam with an injected clock and frame function.

**U40 the Drawer takes and gives back focus.** Given a Drawer opens (U5), then keyboard focus moves into it, and while it holds focus no keystroke reaches the Terminal (U12 sends nothing). When it closes, focus returns to the element that had it when the Drawer opened, if that element is still in the window: a Rail row, a Shelf row or the pane's Terminal (U27's case); a keystroke typed into the pane before the Drawer opened is not lost. Focus that moved elsewhere while the Drawer was open is left alone. The pane's size never changes (U5). Narrowing U27: `u27_with_no_terminal_shown_closing_the_drawer_focuses_nothing` now holds when nothing had focus before the Drawer opened, and a focused Rail or Shelf row is refocused; `u27_focus_already_in_another_field_is_left_alone` and the U28 tests keep passing as written. Needs U31 (a focusable Rail row).

**U41 Rail by keyboard, the rest.** Needs U31. Given an open Project, focus is in exactly one place: the Rail, the pane or an open Drawer. `⌘1` puts it in the Rail and `⌘2` in the pane, with U32's chord rules (handled by the webview, never reaching a Terminal, Caps Lock neutral, other modifier sets do nothing). With focus in the Rail, `←` collapses a Group or Meta-agent (U8, decision 1), or selects its parent when it is already collapsed or a leaf, and `→` expands it, or selects its first child; `F2` edits the selected name in place as double-click does (U9). Nothing in the Rail takes keystrokes while focus is in the pane, so typing `j` into a Terminal never moves the selection. Selecting a row by keyboard shows its Terminal exactly as clicking does and calls no method of its own.

**U42 a Todo's Drawer reads once.** Given a Todo created from the inline field (U16), when its Drawer opens (U17, U5), then `provenance.history {item: "todo:<id>"}` and `todo.get {id}` are each called once, and no call is made while it stays open or after it closes. Found on `just harness tree-40`: after creating a Todo from the `+` field, opening any Todo's Drawer called `todo.get` more than 1000 times a second and never stopped, which on a real Daemon is a flood of read Touches. The cause is not known; the Builder writes the failing test first (create through the field, then open, then count the calls after a settled wait) and diagnoses from it. Opening a Todo that was not just created, and a Pad's Drawer, already read once and keep doing so.

**U43 rename works on a selected row.** Given a row that is already selected (its Terminal pane has the keyboard), when the user double-clicks its name, then the in-place field opens and keeps focus, as it does on an unselected row or a Group (U9). Found on `agents-10`: on an already selected row the field never appeared, probably because the pane takes focus back and cancels the field on blur. Enter, Esc and an empty name behave as U9 says.

**U44 a row brought into view is never under the pinned actions.** Given the Rail's pinned `+ agent  + terminal  + group` line (U32), when a row is scrolled into view by `⌘J` (U30), by selection, or because `⌘N` or `+ agent` just spawned it, then the whole row, Live line included, sits above that line. Found at 1280x800 on `tree-40`: a jumped-to row and a freshly spawned Agent were half under the bar. The Rail's scroll area reserves the bar's height at its bottom edge, and the bar's height is one value that both use. QA measures the row against the bar on `just harness tree-40` at three window widths; the unit test asserts the reservation.

**U45 `⌘J` follows U32's chord rules.** `⌘J` is handled by the webview, never reaches a Terminal, and Caps Lock does not change it; the same key with Shift, Ctrl or Alt, or without ⌘, does nothing. Found: with Caps Lock on, `⌘J` (key `J`) did nothing, and `⌃⌘J` and `⌥⌘J` jumped. This amends U30. Narrowing: `u30_a_plain_j_changes_nothing`, `u30_ctrl_j_changes_nothing` and `u30_cmd_shift_j_changes_nothing` keep passing; new cases cover Caps Lock, `⌃⌘J` and `⌥⌘J`.

**U46 hovering or selecting never moves a row.** Given a Rail with Agents whose Live line shows only on hover or selection (U7), then showing or hiding that line changes no row's position: every row's top edge is the same before and after. Found on `agents-10` and `tree-40`: hovering a row without a visible Live line pushed every row below it down by 15px, which makes sweeping the pointer down the Rail jitter (`docs/motion.md`: nothing snaps). The Builder chooses between reserving the line's height and overlaying it; reserving doubles every Agent row, so the Builder shows the choice on `agents-10` at 1280x800 before committing to it. U7's "hidden, shows on hover or selection" is unchanged for what the user sees.

**U47 a spawned Terminal is selected and shown.** Given `+ terminal` or `⌘T` (U9, U32), when `rail.spawnTerminal` succeeds, then the new Terminal's row becomes selected and its pane shows, as it does for an Agent. Found: after `⌘N` then `⌘T`, the selection stayed on the Agent and the new Terminal existed but was not shown, so the user could not type into it. Narrowing: `u9_plus_terminal_spawns_a_terminal_under_the_selected_group`, `u9_plus_terminal_spawns_at_the_top_level_with_no_selection` and `u32_cmd_t_spawns_a_terminal_under_the_selected_group` keep passing as written and gain the selection check.

**U48 the attention chip looks like the control it is.** Given the header's `<n> need you` (U30), then it is in Ink weight, has a hover state, and its `title` names the chord (`⌘J`). It still shows nothing at zero and still jumps on click and on `⌘J`. In U25's state (after `daemon-exited`) it stays and `⌘J` still selects, since selecting a row calls nothing.

**U49 a Live line never hides what the Agent asked.** Given an Agent whose Status label is longer than the Rail row (`asks: keep v1 routes? …` in a 252px Rail), then the Live line shows `…` (U23) and its `title` is the full label and elapsed time. The pane header (U14) is unchanged.

**U50 the Pad's text fills its Drawer.** Given a Pad's Drawer (U20), then the text field takes the Drawer's remaining height, with no native resize grip, below the owner line and above the last-touch line. Found: it was fixed at about 219px of a 769px Drawer. Everything U20 says about owning, editing and appending is unchanged.

## Stretch

**U22 drag.** Dragging a row shows a drop line whose left end is the depth the row lands at; moving sideways changes the depth. Release calls `rail.move {id, parent, index}`. A `CONFLICT` puts the row back and shows the message as in U9. The other rows make room as the pointer moves (motion.md, "Drag to reorder").

**U25 Daemon gone.** Given an open Project, when `daemon-exited` arrives, then the header reads `roundup   <Project name>   ✕ daemon exited <code>`, or `✕ daemon exited by signal` when the code is `null`, the part after the name in Ink, and that is the only place the exit shows. The Pane sends no `terminal.write` and no `terminal.resize`; `stop`, `+ agent`, `+ terminal` and `+ group` are disabled. The Rail is greyed, not emptied or faded: each row keeps its Glyph, its name and its Live line, and every Live line keeps at least 2:1 contrast against its background, so the user still sees what each Agent was doing when the Daemon went away. Other controls (Todos, Pads, rename, promote) are not disabled: their calls fail with `INTERNAL` (S3) and show as the usual `✕ <message>` line.

## Dev harness

**U26 harness seeds.** Given a seed name (`first-run`, `agents-10`, `tree-40`, `daemon-exits`, `conflict`), when the App mounts on a fake Daemon holding that seed, then the Rail region shows what the seed describes. `first-run` has no open Project (U2). `agents-10` has ten Agents of every Kind. `tree-40` has forty nodes: Groups nested two deep, a Meta-agent with children and Terminals; with eight Todos and four Pads. `daemon-exits` sends `daemon-exited` with code `1` once the App has loaded (U2). `conflict` makes the next call after the App has loaded fail with `CONFLICT` and the one after succeed. Every Rail, Todo and Pad write the App can make changes the fake Daemon and sends the Events the real one sends. `just harness <seed>` serves the same App in a browser, and the page can send Events and failures to it (`docs/development-loop.md`).

## Deferred past the MVP

No scenario yet:
- **Motion (`docs/motion.md`):** every row except the Drawer slide (U5) and drag (U22), that is the Live line crossfade, the done-fold accordion, the new-row layout animation, the needs-you pulse and the Inbox shake.
- **Rail:** Terminals named from their first command (A10 names them after the shell); the `● 1 below` line.
- **Screen 11:** the list of recent folders, and the `claude` version / `✕ claude not found` line.
- **Missing contract pieces:** provenance letters (decision 5); the Todo `on` field; the Pad storage switch.
