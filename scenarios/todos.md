# Todos

**T1 create.** Given an empty Project, when the user calls `todo.create {title}`, then the result is a Todo with id 1, `done: false`, `blocked: false`; `todo.create` again gives id 2; each call emits `todo.created` and logs a `wrote` Touch on `todo:<id>` by the caller.

**T2 read.** Given Todos 1 and 2, when `todo.list` is called, then both come back in id order; `todo.get {id: 2}` returns Todo 2 and logs a `read` Touch by the caller; an unknown id is `NOT_FOUND`.

**T3 blocked is derived.** Given Todo 2 created with `blockers: [1]` and Todo 1 open, then Todo 2 has `blocked: true`. When Todo 1 is completed (`todo.complete`), then Todo 2 has `blocked: false` and `todo.unblocked {id: 2}` is emitted once, after `todo.updated` for Todo 1.

**T4 bad blockers.** Given Todos 1 and 2 where 2 is blocked by 1, when `todo.setBlockers {id: 1, blockers: [2]}` is called, then it fails with `CONFLICT` and nothing changes; an unknown blocker id fails with `NOT_FOUND`; a Todo cannot block itself (`CONFLICT`).

**T5 persistence.** Given Todos created, when the module is dropped and `Todos::open` is called again on the same directory, then every Todo, `done` state and blocker list is intact and the next id continues the sequence.

**T6 delete.** Given Todo 2 blocked by 1, when Todo 1 is deleted, then `todo.deleted {id: 1}` is emitted, Todo 2's blocker list no longer contains 1, `blocked` is false, and `todo.unblocked {id: 2}` is emitted.

**T7 update.** Given Todo 1, when `todo.update {id: 1, title: "x"}` is called, then the title changes, the body is untouched, `todo.updated` is emitted, and a `wrote` Touch is logged; an update with neither field is `INVALID_PARAMS`.

**T9 a rejected call leaves no trace.** Given Todos 1 and 2 and a subscribed client, when `todo.create` names a blocker that does not exist (`NOT_FOUND`), `todo.setBlockers` would make a cycle (`CONFLICT`, T4), `todo.update` has neither field (`INVALID_PARAMS`) or `todo.complete` names an unknown id (`NOT_FOUND`), then after each call `todo.list` is unchanged, no event arrived, no Touch was logged, and the next `todo.create` returns id 3: a failed create never uses an id. After deleting the newest Todo and reopening with `Todos::open`, the next create still does not reuse its id. Owns `crates/todos/**`. It keeps T4 and T5 green and narrows none: T5 says the next id continues the sequence, and T9 says what continues it.
