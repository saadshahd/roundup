# U152 to U156 rendered observer

Headless agent-browser, one session per size and theme, on `just harness tree-40 5199` (U26, fake Daemon). 1280×800 and 700×800, light and dark. `observe.sh W theme` is the run; `observer.txt` is its output for all four.

- U152: `remove` by keyboard and by pointer on Todo 4 (`waits on #5`, then no `waits on` line); `complete` by pointer and by Tab from the row (the Shelf's `✓ n done` goes up one each); add by pointer (`#9 ship it`); `+ blocker` #1 on Todo 2 by keyboard (`waits on #1`, `todo.setBlockers {id: 2, blockers: [1]}`). `document.activeElement` is never `body` after a keyboard step. This run found that `remove` and choosing an offered blocker by keyboard left focus on `body`; the PR fixes it (`u152_remove_by_keyboard_leaves_focus_off_the_body`).
- U153: computed row values equal the Tokens (background `--ground`, radius `--radius-row`, padding `--space-1`/`--space-2`, gap 4 px, list padding 8 px, button 28 px high, `--text-body`/`--text-caption`, `--grey`, no border or shadow); hover and selected are a `--hover` or `--selected` wash over `--ground`, so `background-color` stays `--ground`.
- U154: on `tree-40`'s long title no element of a Todo row has `scrollWidth` above `clientWidth`, no row is wider than the list, and no top edge moves when a row is hovered.
- U155: a real pointer drag of Todo 3 above Todo 1 calls `todo.reorder {id: 3, before: 1}` once and the rows read `[3, 1, 2]` with no Drawer opened; in a fresh load `⌥↑` twice reads `[3, 1, 2]` and the focused element is Todo 3's row button.
- U156: sampling every frame after `⌥↑`, the moved rows reach their final top 149 to 150 ms after the first moved frame, in steps that shrink toward the end (read by eye, not asserted), and rows 1 and 4 never move.

Not shown: a `rupd` restart and reload (the fake Daemon starts again from its seed), the Agent-created variant of U155 (`u155_` covers it), the interrupted second move, reduced motion, and Percy before and after (the Percy build of the PR holds them).
