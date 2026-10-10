# UI: the Rail's layout survives a restart (U100)

Module: `apps/desktop` (the webview). Ids: U100.

Webview only. No App command, no Daemon method and nothing under `contracts/` changes: the webview keeps this state in its own storage, per Project path. `docs/motion.md` has no row for it, because nothing animates.

**U100 the Rail's layout survives a restart.** Given an open Project whose Rail has a selected row and one or more collapsed Groups (U3, U8), when the webview is closed and opened again on the same Project path, or reopens it after `daemon-exited` (S5, U37), then after the first `rail.tree` the same row is selected, its Terminal shows as a click on it would (U6), and the same Groups are collapsed (U8), all in the frame that first shows the tree: no expand, collapse or scroll animation runs for it, and no row shows expanded and then collapses. Before that tree arrives nothing is selected and nothing is drawn from the saved state. A Door selection also survives; Door folding is a separate U41 follow-up because the current Rail has no collapse control for one.

- Only these two facts are kept: which row is selected and which nodes are collapsed. Drag state, the unfolded `✓ n done` lines (U8), scroll position and open Drawers are not.
- A saved id that is not in the first `rail.tree` is ignored: a selected node that is gone leaves nothing selected (U3), and a gone collapsed node is not collapsed. The next write drops it from the saved state. A node is never created, selected or collapsed because of the saved state alone.
- A restored selection under a restored collapsed Group is never hidden: that Group is expanded, as U30 does, and the saved state records it expanded.
- Another Project path starts with nothing selected and nothing collapsed, and never reads or changes this Project's saved state.
- Restoring calls no method of its own, only the `terminal.resize` that U13 makes when the restored Terminal first shows, and moves no focus (U41): focus stays where the window put it, and the Rail never reorders. After restore, every selection and collapse behaves as U3, U8 and U30 say, and each change is saved.
- A saved value the webview cannot read is treated as absent and replaced by the next change. When the storage refuses a write, the webview shows one line `✕ <message>` in Ink above `+ agent  + terminal  + group`, as U9 does, and keeps working with the state it holds.

Tests (`u100_`) use the fake App and the U26 harness seeds `agents-10` and `tree-40` with jsdom's storage: mount, select a row and collapse a Group, unmount, mount again on the same path and assert the selection and the collapsed Groups after the first tree; again on another path; with a tree that lost the saved nodes; with an unreadable value; with a storage that throws on write; and assert the fake App saw no call beyond the ones U3 makes and the one `terminal.resize` of U13; plus a saved selection under a saved collapsed Group.
