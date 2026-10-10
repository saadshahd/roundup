# U157, U158 rendered observer

Base: `f39130b` (main). Headless agent-browser, one session per size and theme, on `just harness tree-40 5199` (U26), 1280×800 and 700×800, light and dark. `observe.sh W theme` is the run; `observer.txt` is its output for all four.

Each run moves Todo 2 to `auth-refactor` by the pointer from the row (`todo.move {id: 2, home: "auth"}`), then back to `project root` by the keyboard from the Drawer (`{id: 2, home: null}`; the offers there are `project root` and the other Rooms, not the current Home). `[data-todo-home]` of Todo 2 reads `in auth-refactor`, then `in project root`; after `rail.rename` it reads `in renamed-room`, and after `rail.remove` of that Room `in project root` (U158). `document.activeElement` is never `body` after a keyboard step; after choosing it is the `move` word. Every row's top edge is the same at rest, with the pointer on the row, and with the list open (U112).

Not shown: the fake Daemon starts over on reload, so "after reload" is shown by the `todo.list` refetch that follows each `todo.move` in `app.calls`.
