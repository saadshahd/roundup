# Scenarios

A scenario is the spec for a unit of work: given / when / then, in `CONTEXT.md` words. No scenario, no work.

- Ids are a letter and a number (`T3`). A test that proves a scenario has the lowercase id as the start of its name: `fn t3_completing_a_blocker_unblocks_the_todo`.
- A PR names the scenario ids it implements; rule 1 ("done") requires each to have a passing test whose name starts with its id.
- Methods and events are the ones in `crates/contracts`. Errors use the codes in `rpc::code`.
- Every write or read of a Todo or Pad is a Touch: `ctx.touch(verb, "todo:<id>" | "pad:<name>")`.
- Tests run without Claude, a network or a display. Anything that needs a real `claude` is replayed from `spikes/hooks-state/*.jsonl`.

| File | Module | Ids |
|---|---|---|
| `todos.md` | `crates/todos` | T1–T7 |
| `pads.md` | `crates/pads` | P1–P9 |
| `terminal.md` | `crates/terminal` | X1–X8 |
| `agents.md` | `crates/agents` (+ the `rup hook` subcommand in `crates/rup`) | A1–A8 |
