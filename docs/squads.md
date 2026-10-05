# Squads and the architect committee

A squad owns a module and everything up to a Builder starting; a committee of three architects owns what several squads share.

## Squads

A squad is one scenario writer, its Builders and its reviewer pool. It writes scenarios in its own files and id range, starts Builders from them and merges through rule 1, with no architect. Only it writes the `## Work` tables of its files (a row no squad owns goes in the file of its first id), and its Builders edit only the files those rows own.

| Squad | Module directories | Scenario files | New ids it allocates |
|---|---|---|---|
| ui-rail | `apps/desktop/src/rail`, `drawer`, `todos`, `pads` | `ui-rail.md` | U61 to U79 |
| ui-surfaces | `apps/desktop/src` outside those four (switcher, first run, help, pane, empty and error states, and the keystroke probe) | `ui-surfaces.md` | U80 to U99 |
| daemon | `crates/agents`, `crates/terminal`, `crates/todos`, `crates/pads`, `crates/rupd`, `crates/rup`, `crates/rpc`, `crates/desktop` | `agents.md`, `terminal.md`, `todos.md`, `pads.md`, `daemon.md`, `mcp.md`, `rpc.md`, `app.md` | A20 to A29, X11 to X20, T10 to T15, P11 to P14, D8 to D9, M4 to M6, C2 to C9, S8 to S12 |
| loop | `loop/`, `.agents/` | `loop-rules.md`, `loop-boxd.md`, `loop-qa.md` | L35 to L39 |
| perf | `crates/perf`, the workspace files | `perf.md`, `workspace.md` | R14 to R29, K4 to K9, W2 to W5 |

Architects allocate A30 to A49, D10 to D19, C10 to C19, T16 to T25 and L40 to L59. A squad out of ids asks the committee. Two squads never edit one scenario file: `ui.md` (U1 to U60) takes no new ids, and an edit to one of its scenarios belongs to the squad owning that scenario's module; a scenario needing another squad's file names it, and that squad's writer adds the part.

## Architect committee

Three architects: `architect-swarm` (contracts and ids), `architect-b` (UI and product) and `architect-c` (daemon, rpc and loop). The committee approves what no single squad owns:

- any edit under `contracts/`, including a new RPC method, an event or a field;
- any App seam command (a Tauri command or capability);
- any change to the shared design tokens (U4's colours and type);
- `GLOSSARY.md` terms that two squads use, and ADRs;
- a change to a scenario that a merged test asserts, when it crosses squads;
- a conflict between squads.

An approval is a comment `ARCHITECT: approve <full sha>` (or `reject`) from one architect other than the author, naming what was checked; it counts for the head it names. When a second architect disagrees the third decides in a PR comment, or the user when the third is the author. Nothing outside this list waits on the committee.
