# Architecture

roundup is a Claude Code workbench: Terminals, Agents (Claude Code only), Todos, Pads. macOS only for the MVP. No accounts, sync or telemetry. Terms are defined in `/CONTEXT.md`.

```
 ┌ Tauri UI (Solid) ┐  ┌ rup CLI ┐  ┌ Claude Code agents ┐  ┌ extensions ┐
 └────────┬─────────┘  └───┬────┘  └─────────┬──────────┘  └─────┬──────┘
          └────── JSON-RPC over unix socket / MCP (stdio) ─────────┘
                              │
                        rupd (Rust)
      ┌────────────┬─────────┴───────┬──────────────┐
   terminals     agents           todos         scratchpads
   (PTY mgr)  (adapter+state)  (SQLite+deps)  (md files+index)
                              │
                       event bus (typed, replayable)
```

Principle: every feature is a typed RPC method plus an event. The UI has zero privileged access. Client crates talk to `rupd` only through `contracts/`.

The App is a Rust crate under `crates/`, not under `apps/`. A Cargo workspace member glob that matches nothing is an error, so `apps/*/src-tauri` would force a root `Cargo.toml` edit into the first UI PR. This layout keeps each PR to one module and lets the App and the webview be built in parallel. They meet at the App seam in `scenarios/app.md`.

## Layout

| Path | Owner module | Contents |
|---|---|---|
| `crates/contracts` | Architect | RPC params, results and events as Rust types; generates `contracts/generated/*.ts` (`cargo test -p contracts`); `contracts/hooks.ts` holds the Hook types |
| `crates/rpc` | Architect | `Module` trait, `Ctx`, event `Bus`, error codes, `Client` |
| `crates/provenance` | Architect | the Touch log (append-only SQLite) |
| `crates/terminal` | terminal | `portable-pty`, screen state |
| `crates/agents` | agents | `AgentAdapter`, Status classifier; `claude_code/` adapter |
| `crates/todos` | todos | SQLite (WAL), blocker graph |
| `crates/pads` | pads | Pads as `.md` files, SQLite index |
| `crates/rupd` | core | Daemon: socket, Provenance log, bus, MCP |
| `crates/desktop` | ui | The App: the Tauri 2 shell that starts `rupd` for one Project and passes calls and events to the webview (`scenarios/app.md`) |
| `apps/desktop` | ui | The webview: Solid + xterm.js, a client of `rupd` through the App |
| `ext/` | ext | Extension host, SDK, example |

## Agent adapter contract

The adapter emits glossary Kinds and takes two kinds of input, because hook payloads arrive on a side channel, not in the PTY stream.

```rust
trait AgentAdapter {
    fn spawn(&self, cwd: &Path, prompt: Option<&str>) -> Result<AgentHandle>;
    /// Fold one Observation into the Agent's Status. None = no change.
    fn observe(&mut self, o: Observation) -> Option<Status>;
}
enum Observation { Title(String), Signal(Json), Exit { code: Option<i32> }, Stopped, Tick }
enum AdapterKind { NeedsYou, Error, Working, Idle, Done } // never Blocked
```

`blocked` is computed by `rupd` from Todo blockers and Routes. Signal for Claude Code = hook payloads, merged with the terminal title and exit status because some transitions fire no hook (ADR 0006, `spikes/hooks-state/REPORT.md`). Screen scraping is a last fallback behind the same trait.

## Data

- Todos, Pad index, Touch log, Routes, Agent tree (`parent` + `order`): SQLite in WAL mode, one DB per Project at `.roundup/roundup.db`.
- Pads: plain `.md` files. App-only by default (stored in the DB); per-Project setting stores them as files in `.roundup/pads/`.
- Touch log is append-only; `readBy` / `touchedBy` are derived, never maintained by hand.

## Message delivery

Typed Messages `{from, to, kind, body, replyTo}`. Delivery to a Claude Code Agent happens at its next safe point (its idle signal, or a prompt-submit injection), tagged with a sender header. Anything addressed to the user pre-fills and never submits. A Route's value comes from `bus.route`; a user's click wins.

## Perf budget (CI-measured; a regression above 10% fails)

Cold start < 300 ms; keystroke-to-render < 16 ms p95; 10 idle Agents < 150 MB extra RSS.

## Boundaries

Local only. The MVP targets macOS, so the shipped app, perf numbers and the macOS gate run locally and on GitHub macOS runners. boxd VMs are optional: unattended Builders and Linux or web-UI QA runs. See `docs/boxd.md` for what they can and cannot do.
