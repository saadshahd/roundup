# UI: the Shelf holds still (U112)

Module: `apps/desktop` (the webview). Ids: U112.

**U112 hovering a Shelf row never moves a row.** Reported by QA (F7, sweep 2): hovering an open Todo row shows `complete` inline, the title wraps and the next row moves down 15 px. Given the Shelf's open Todos (U35), then showing or hiding `complete` on hover or focus changes no row's top edge, no row's height and no row's wrapping: the same rule U46 sets for the Rail's Live line, here for the Shelf. `complete` takes no width from the title: it overlays the row's right edge or sits in space reserved at rest, and the row's text at rest is unchanged (U35). The same holds for any other word a Shelf row shows on hover or focus. Tests (`u112_`) read the top edge, height and the title's line count of every row before and after hover and focus in the harness, on a list whose longest title nearly fills its row, and fail on main today.

## Work

Rows a Builder can take; `loop/rules.sh ready` prints each one's state.

| Ids | Item | Owns | Keeps green | After |
|---|---|---|---|---|
| U112 | Shelf row holds still on hover/focus | `apps/desktop/src/todos/**`, Shelf rules in `styles.css` | `u112_` layout test, `just check`, browser proof | — |
