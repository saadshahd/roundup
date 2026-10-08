# Harness

Data for the QA sweep, the Design critic and the Percy build: `loop/percy.sh` (L36) snapshots every seed in the table below.

`just harness <seed> <port>` (defaults `tree-40`, `5199`, strict port) serves the App on a fake Daemon at `http://localhost:<port>/harness.html?seed=<seed>` (U26). It checks no Route, Block or Provenance and says nothing about macOS rendering or timing: read the DOM, never the numbers.

| Seed | Starts with |
|---|---|
| `first-run` | no open Project |
| `empty-project` | open Project with no rows; U143’s first Room action |
| `door-stopped` | selected Room whose Door exited 0; U143’s state and restart action |
| `decisions` | `agents-10` with a permission, an `ask_user` and an unanswerable Decision on `agent-1` to `agent-3`; answering clears it |
| `agents-10` | ten Agents, one per Kind in turn |
| `tree-40` | forty nodes (nested Rooms, a Door, Terminals), eight Todos, four Pads |
| `daemon-exits` | `agents-10`, then `daemon-exited` with code 1 |
| `conflict` | `agents-10`; the first call fails with `CONFLICT` |

`window.__fake` (drive with `agent-browser eval`):

| Call | Sends |
|---|---|
| `setStatus("agent-3", "needs-you", "asks: keep v1?")` | `agent.status` |
| `writeOutput("t-agent-3", "hello\r\n")` | `terminal.output`; visible only once that Agent is selected |
| `emit({actor: {kind: "user", id: "you", parent: null}, name: "rail.changed"})` | any Event |
| `failNext(-32003, "name already taken")` | the next call rejects |
| `app.exitDaemon({code: 1})` | `daemon-exited`; `app.calls` lists every call |

`window.__checks()` (U137) runs D1 to D10 on the screen. `agent-browser`: one session name per agent, `set viewport W H`, single key events via `eval` (after `press Enter` a key may repeat; count keydowns first).

## Held-out screens

No Builder prompt lists these.

- `tree-40`, 1280x800, dark, the Todo Drawer open on `#5`.
- `agents-10`, 700x800, light, `prefers-contrast: more`.
- `tree-40` with a 60-character Agent name (U29), 1280x800, dark.
- `first-run`, 700x800, dark.
- `conflict`, 1280x800, light, a Pad Drawer open.

## Rules the Checks do not cover

| Rule | Passes when |
|---|---|
| R1 | the Live line shows only for blocked, needs-you, error, hover or selection |
| R2 | the provenance letter shows only on hover or selection |
| R3 | each duration is within 25% of `docs/motion.md`; a Status Glyph change is instant; nothing snaps (covers its whole displacement between two `requestAnimationFrame` samples, so a clause of 120 ms or more needs two frames between); reduced motion leaves only the instant ones; drag unscored |
| R4 | no string the app itself writes holds an _Avoid_ word, and each domain noun is its `GLOSSARY.md` term; `scenarios/ui.md`'s strings are exempt |

## Captures

`artifacts/ux/<id>/<step>.png` and `<step>.checks.json`; motion: `<step>-frames.json` and at most five `<step>-NN.png`; baselines `artifacts/ux/<id>/base/` and `head/`, compared by `loop/rules.sh delta base head` (L42; exit 2 is a failed run). Linux and macOS captures are never compared.
