# U151, U157, U158 rendered observer

Headless agent-browser, one session per size and theme, on `just harness-real` (U144) over a real `rupd` of a fresh Project, so a reload reads the Daemon, not a fake. 1280×800 and 700×800, light and dark. `observe-real.sh W theme` is the run; `observer.txt` is its output for all four. The Project held Rooms `auth-refactor` and `docs` and Todos #1 to #3, #3 blocked by #1 and #2 (sent as `todo.setBlockers`).

U151: with the pointer parked outside `.shelf`, each row's row button, `data-todo-waits` and `data-todo-home` are read from `document`; #3 reads `waits on #1, #2` and every Home reads `in project root`.

U157: Todo 2 is moved to `auth-refactor` by the pointer from the row, and `[data-todo-home]` reads `in auth-refactor`, the Daemon's `todo.list` gives `home` `"1"`, and after reload it still reads `in auth-refactor`. It is moved back by the keyboard from the Drawer (the offers there are `project root` and `docs`); it reads `in project root`, `home` is `null`, and after reload it still does. `document.activeElement` is never `body` after a keyboard step, and after choosing it is the `move` word. Every row's top edge is the same at rest, hovered and with the list open (U112).

U158: Todo 2 moved to a Room made for the run reads `in <that Room>`, `in renamed-room` after `rail.rename`, and `in project root` after `rail.remove`, also after reload.

Not shown: the Esc and click-outside paths (`u157_` Vitest cases cover them).
