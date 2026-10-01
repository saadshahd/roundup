# Scenarios

A scenario is the spec for a unit of work: given / when / then, in `CONTEXT.md` words. No scenario, no work.

- Ids are a letter and a number (`T3`). A test that proves a scenario has the lowercase id as the start of its name: `fn t3_completing_a_blocker_unblocks_the_todo`.
- A PR names the scenario ids it implements; rule 1 ("done") requires each to have a passing test whose name starts with its id. The one exception is W1. It changes only root config, and its scenario names the observer that proves it instead of a test.
- Methods and events are the ones in `crates/contracts`. Errors use the codes in `rpc::code`.
- Every write or read of a Todo or Pad is a Touch: `ctx.touch(verb, "todo:<id>" | "pad:<name>")`.
- Tests run without Claude, a network or a display. Anything that needs a real `claude` is replayed from `spikes/hooks-state/*.jsonl`.

| File | Module | Ids |
|---|---|---|
| `todos.md` | `crates/todos` | T1–T7 |
| `pads.md` | `crates/pads` | P1–P9 |
| `terminal.md` | `crates/terminal` | X1–X10 |
| `agents.md` | `crates/agents` (+ the `rup signal` subcommand in `crates/rup`) | A1–A13 |
| `loop.md` | `loop/` | L1–L15 |
| `mcp.md` | `crates/rup` (the `rup mcp` subcommand) | M1–M3 |
| `daemon.md` | `crates/rupd` (D1); end-to-end tests in `crates/rup/tests` (D2–D4) | D1–D4 |
| `app.md` | `crates/desktop` (the App: Tauri shell, App seam) | S1–S5 |
| `ui.md` | `apps/desktop` (the webview) | every `U` heading in the file; ids are reserved in `.work/queue.md` |
| `rpc.md` | `crates/rpc` | C1 |
| `workspace.md` | root files | W1 |
| `perf.md` | `crates/perf` | R1–R7 |

The order of work, the PR each scenario belongs to and what can start now are in `.work/queue.md`.
