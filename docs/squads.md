# Squads and the architect committee

No single agent writes every scenario. A squad owns a module and everything up to a Builder starting; a committee of three architects owns what several squads share.

## Squads

A squad is one scenario writer, its Builders and its reviewer pool. It writes scenarios inside its own files and id range, starts Builders from them and merges through rule 1, and needs no architect for any of that. Its rows live in `.work/queue/<squad>.md`, and its Builders edit only the files those rows own.

| Squad | Module directories | Scenario files | New ids it allocates |
|---|---|---|---|
| ui-rail | `apps/desktop/src/rail`, `drawer`, `todos`, `pads` | `ui-rail.md` | U61 to U79 |
| ui-surfaces | `apps/desktop/src` outside those four (switcher, first run, help, pane, empty and error states) | `ui-surfaces.md` | U80 to U99 |
| daemon | `crates/agents`, `crates/terminal`, `crates/todos`, `crates/pads`, `crates/rupd`, `crates/rup`, `crates/rpc`, `crates/desktop` | `agents.md`, `terminal.md`, `todos.md`, `pads.md`, `daemon.md`, `mcp.md`, `rpc.md`, `app.md` | A17 to A29, X11 to X20, T9 to T15, P10 to P14, D5 to D9, M4 to M6, C2 to C9, S7 to S12 |
| loop | `loop/`, `.agents/`, `docs/boxd.md` | `loop.md` | L26 to L39 |
| perf | `crates/perf`, the keystroke probe, the workspace files | `perf.md`, `workspace.md` | R14 to R29, K4 to K9, W2 to W5 |

Ids already in `.work/queue.md` stay with their holders. A squad that runs out of ids asks the committee for the next block. Two squads never edit one scenario file. A scenario that needs a file another squad owns names it, and the owning squad's writer adds that part.

## Architect committee

Three architects, each with its own id. The committee approves what no single squad owns:

- any edit under `contracts/`, including a new RPC method, an event or a field;
- any App seam command (a Tauri command or capability);
- any change to the shared design tokens (U4's colours and type);
- `CONTEXT.md` terms that two squads use, and ADRs;
- a change to a scenario that a merged test asserts, when it crosses squads;
- a conflict between squads.

An approval is one architect whose id differs from the author of the PR or scenario. If a second architect disagrees, the third decides, and the decision is recorded as a PR comment. The approval is a comment `ARCHITECT: approve <full sha>` or `ARCHITECT: reject <full sha>`, naming what was checked, and it counts for the head it names. A squad does not wait for the committee on anything outside this list. The committee is not a gate on scenarios inside a squad's range, and it does not read every PR.

## Unchanged

Rules 1, 2, 3, 5, 6 and 7 of `AGENTS.md`: a reviewer's id differs from the author's, the checks and the scenario test are required, the Reviewer's input excludes the author's rationale, and vocabulary and the perf budget apply. A PR still goes through a different agent's review and a Merger. Red main is still stop-the-line and a revert.

## Moving the queue

`.work/queue.md` keeps the swarm protocol and the id table. Each squad's rows move to `.work/queue/<squad>.md` in one PR after the open queue PRs have merged, so no row is edited in two places meanwhile. Until that PR merges, rows stay where they are.
