# UI

The webview in `apps/desktop`: Solid, Vite and xterm.js with the WebGL addon (ADR 0005), drawn from `docs/wireframes.md` (its corrections and decisions win over the drawings) and timed by `docs/motion.md`. It is a client of the Daemon through the App seam (`app.md`) and nothing else. Tests run in Vitest with jsdom against a fake App seam; only one adapter file imports Tauri. The terminal emulator and the clock are injected, so no test needs a display, a GPU or real time. Exported names say Project, never folder: `folder` is the _Avoid_ word for Group. On-screen text may say folder, because the wireframes do.

Methods and events are the ones in `contracts/generated/methods.ts` and `Event.ts`. The text a scenario quotes is what the screen shows; capitals in the wireframes stand for ink, and the app never uppercases.

## Shell (U1 to U5)

**U1 calls and events.** Given the App seam, when the webview calls a method named in `RpcMethods`, then its params and its result are typed from `contracts/generated/methods.ts`. A failed call rejects with the Daemon's `code` and `message`. Every Event reaches every subscriber in the order the Daemon sent it.

**U2 first run.** Given no open Project (Screen 11), then the Rail region reads `agents and terminals` / `appear here, one per row,` / `nested by indent`. The centre reads `open a folder to start` and `choose folder…`. The Shelf reads `todos` / `agents add them as they` / `plan; so can you`, then `pads` / `◈ agent notes` / `◇ yours`. When a folder is chosen in the macOS chooser, or the App already has a Project (`project`), then the header reads `roundup   <Project name>` and the three regions show that Project. When `open_project` fails, `✕ <message>` replaces `open a folder to start` and is the only ink in the window. On `daemon-exited`, the centre shows `✕ daemon exited <code>` in ink.

**U3 Rail state.** Given an open Project, then the webview subscribes before its first `rail.tree` and `terminal.list` and keeps the Rail state current. `rail.changed` refetches the tree. `agent.status` replaces one node's Status. `terminal.exited` marks the node with that `terminal_id` as exited with its code, and a node with no `terminal_id` counts as exited. Events that arrive while a `rail.tree` call is in flight are applied after it, never dropped. At most one node is selected (none at start), and a node that leaves the tree is no longer selected. `nameOf(actor)` gives `you` for the user, the Rail name for an Agent's id, and otherwise the id.

**U4 ink.** A Kind maps to a Glyph: `needs-you` `●` in bold amber, `error` `✕` in bold red, `blocked` `⏸` grey, `working` `○` light grey, `idle` `·` light grey, `done` `✓` lightest grey. Only `needs-you` and `error` carry Ink. The most urgent of several Kinds follows the order in `CONTEXT.md` (`error` first). Elapsed time since a `since` reads `just now` under a minute, then `4m`, `2h`, `3d`, against the injected clock.

**U5 Drawer and last touch.** Given the main workspace, when a Drawer opens, then it slides in from the right over the terminal and the Shelf (about 180 ms, ease-out; instant under `prefers-reduced-motion`) with a light `close` word. One Drawer is open at a time, and the terminal pane's size never changes (its size observer reports nothing). For an item (`todo:<id>` or `pad:<name>`), the last-touch line reads `last  <name> <read|wrote> <hh:mm>`, from the newest Touch in `provenance.history`.

## Rail (U6 to U10)

**U6 rows.** Given a Rail tree, then each node is one row, indented 2 per depth under its parent, in `order`. An Agent or a Meta-agent shows its Status Glyph. A plain Group shows `▾`. A Terminal shows `○` while it runs and `✓` once it has exited. The selected row has a faint band. Rows never reorder when a Status changes. Clicking a row selects it.

**U7 live line.** Given a row, then its Live line is the Status label and its elapsed time (`asks: keep v1 routes?  4m`); for a Terminal it is `exited <code>` once exited. The Live line is hidden, shows on hover or selection, and shows unprompted when the Kind is `blocked`, `needs-you` or `error`; for the last two the name and the Glyph are in Ink.

**U8 folding.** Agents that have been `done` for 10 minutes or more fold into one `✓ n done` line per tree level, after their siblings, and a click unfolds them (wireframes decision 2). Clicking a Group's `▾` collapses it: the row then shows its most urgent descendant's Glyph and Ink, plus a light child count, and another click expands it (decision 1).

**U9 actions.** `+ agent` calls `agent.spawn {cwd: <Project path>, prompt: null, parent}`. `parent` is the selected Group or Meta-agent, else `null`, and the new row becomes selected, so its terminal shows. `+ terminal` calls `rail.spawnTerminal {cwd: <Project path>, parent}` with the same parent rule. `+ group` calls `rail.createGroup {name: "group", parent}`. Double-clicking a name edits it in place: Enter calls `rail.rename`; Esc or an empty name keeps the old one. Hovering a plain Group shows a light `promote`, which calls `rail.promote`. A failed call shows one line `✕ <message>` in Ink above `+ agent  + terminal  + group` until the next click. Rows come only from `rail.tree`; the webview never adds one itself.

**U10 Dock badge.** The Dock badge shows how many Agents have Kind `needs-you` or `error`, and the App shows no badge at zero (decision 4).

## Terminal pane (U11 to U14)

**U11 one emulator per Terminal.** Given an open Project, when `terminal.output {id, data}` arrives, then its base64-decoded bytes are handed, in order, to the emulator for Terminal `id`. That emulator is created on the Terminal's first output, whether or not its row is selected. `terminal.output` is never replayed, so an emulator created later would show a blank pane. Selecting a row shows its Terminal's emulator and no other. Tests observe the bytes handed to an injected emulator.

**U12 typing.** Given the selected Terminal runs, when the user types into the pane, then each keystroke's bytes go to `terminal.write {id, data}` (base64), in order. An exited Terminal takes no input.

**U13 size.** Given the pane shows a Terminal, then `terminal.resize {id, cols, rows}` is called with the emulator's fitted size: when that Terminal is first shown, and whenever the window resizes, at most once per animation frame. Opening a Drawer never resizes it. The WebGL renderer is used when the webview grants a WebGL context, else the DOM renderer, with one warning logged.

**U14 pane header.** The pane's top line reads `<name>  <kind> <elapsed>` for an Agent (`auth-refactor  working 12m`), and `<name>  exited <code>` for an exited Terminal. While the program runs, a light `stop` sits at the right: for an Agent it calls `agent.stop {id}`, for a Terminal `terminal.kill {id: <terminal_id>}`. With an open Project and nothing selected, the pane is empty.

## Todos (U15 to U17)

**U15 list.** Given Todos, then the Shelf's `todos` list shows the open ones in id order as `<glyph> #<id> <title>`. An open Todo is `·`. A blocked one is `⏸`, with a second line `waits on #<id>` naming each open blocker. Done Todos fold into one `✓ n done` line that unfolds on click. On `todo.created`, `todo.updated`, `todo.unblocked` and `todo.deleted`, the list refetches `todo.list`, which logs no Touch.

**U16 create.** Clicking the `+` beside `todos` opens one inline title field at the top of the list. Enter calls `todo.create {title}`, and the row appears through the event. Esc or an empty title closes the field and calls nothing. A failure shows `✕ <message>` in place of the field.

**U17 detail.** Clicking a Todo opens its Drawer: `#<id>  <title>` with its Glyph; `waits on` with each blocker and its own Glyph; `blocks` with each Todo it blocks; `+ blocker`, which offers the other Todos and calls `todo.setBlockers` (a `CONFLICT` shows inline); the body; the last-touch line (U5); and light `complete` and `delete` words (`todo.complete`, `todo.delete`). Double-clicking the title or the body edits it, and leaving the field calls `todo.update` once if the text changed. Opening the Drawer calls `todo.get` once, which is one read Touch. Background refreshes never call `todo.get`.

## Pads (U18 to U21)

**U18 list and ownership.** Given Pads, then the Shelf's `pads` list shows each by name, with `◈` when an Agent owns it and `◇` when the user does. Clicking `◈` calls `pad.setOwner {name, owner: <user>}`, and the Glyph becomes `◇`. On `pad.changed`, the list refetches `pad.list`, which logs no Touch.

**U19 create.** Clicking the `+` beside `pads` opens an inline name field. Enter calls `pad.create {name}`. A `CONFLICT` or `INVALID_PARAMS` shows `✕ <message>` in place of the field.

**U20 open and edit.** Clicking a Pad opens its Drawer: `<◈|◇> <name>   owned by <owner name>`, a light `export .md`, the text as markdown source in the monospace face, and the last-touch line (U5). Opening calls `pad.read` once, which is one read Touch. When the user owns the Pad, the text is editable, and leaving the field calls `pad.write` once if the text changed. When an Agent owns it, the text is read-only and an `append` field calls `pad.append`. A `pad.changed` for the open Pad, while the user is not editing it, refreshes its text from `pad.list`.

**U21 export.** `export .md` opens the macOS save chooser with `<name>.md` filled in. Choosing a path calls `pad.export {name, path}`, and cancelling calls nothing. An error shows `✕ <message>` in the Drawer.

## Stretch

**U22 drag.** Dragging a row shows a drop line whose left end is the depth the row lands at; moving sideways changes the depth. Release calls `rail.move {id, parent, index}`. A `CONFLICT` puts the row back and shows the message as in U9. The other rows make room as the pointer moves (motion.md, "Drag to reorder").
