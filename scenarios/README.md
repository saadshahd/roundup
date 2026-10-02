# Scenarios

A scenario is the spec for a unit of work: given / when / then, in `CONTEXT.md` words. No scenario, no work.

- Ids are a letter and a number (`T3`). A test that proves a scenario has the lowercase id as the start of its name: `fn t3_completing_a_blocker_unblocks_the_todo`.
- A PR names the scenario ids it implements; rule 1 ("done") requires each to have a passing test whose name starts with its id. Two exceptions: W1 changes only root config, and U55 is a diagnosis that changes no app file. Each names the observer that proves it instead of a test.
- Methods and events are the ones in `crates/contracts`. Errors use the codes in `rpc::code`.
- Every write or read of a Todo or Pad is a Touch: `ctx.touch(verb, "todo:<id>" | "pad:<name>")`.
- Tests run without Claude, a network or a display. Anything that needs a real `claude` is replayed from `spikes/hooks-state/*.jsonl`.

| File | Module | Ids |
|---|---|---|
| `todos.md` | `crates/todos` | every `T` heading in the file |
| `pads.md` | `crates/pads` | every `P` heading in the file |
| `terminal.md` | `crates/terminal` | X1–X10 |
| `agents.md` | `crates/agents` (+ the `rup signal` subcommand in `crates/rup`) | every `A` heading in the file |
| `loop.md` | `loop/` | every `L` heading in the file |
| `mcp.md` | `crates/rup` (the `rup mcp` subcommand) | M1–M3 |
| `daemon.md` | `crates/rupd` (D1, D7); end-to-end tests in `crates/rup/tests` (D2–D7) | every `D` heading in the file |
| `app.md` | `crates/desktop` (the App: Tauri shell, App seam) | every `S` heading in the file |
| `ui.md` | `apps/desktop` (the webview) | every `U` heading in the file; ids are reserved in `.work/queue.md` |
| `ui-persist.md` | `apps/desktop` (the webview) | U100 |
| `ui-daily.md` | `apps/desktop` (the webview) | U101, U102 |
| `rpc.md` | `crates/rpc` | C1 |
| `workspace.md` | root files | W1 |
| `perf.md` | `crates/perf`, and the keystroke probe in `apps/desktop` | every `R` and `K` heading in the file |

The order of work, the PR each scenario belongs to and what can start now are in `.work/queue.md`.
