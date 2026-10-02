# Control channel

Status: provisional until the lead and the user confirm the recommendation and architect-b's steering series agrees on the seam. Scenarios: `scenarios/control.md` (H11 to H16). Evidence: `spikes/hooks-permission/REPORT.md` (Claude Code 2.1.287, macOS).

## Seams today

| Direction | Seam | State |
|---|---|---|
| Agent to Daemon | per-Agent `roundup` MCP server (`rup mcp`, A11, M1), plus `rup signal` hooks | exists; B12 adds Messages, #162 adds `agent_context` |
| Daemon to Agent, a prompt | `type_prompt` in `crates/agents/src/lib.rs`: paste, a 1 s `SUBMIT_DELAY`, then `\r` | the only keystroke writer in the adapter; fixed delay |
| Daemon to Agent, a permission answer | none (the user answers in the Terminal) | missing; `docs/permission-cards.md` |
| Daemon to Agent, end a turn | none | missing |

## Options compared

| Option | Prompt in | Permission answer | Interrupt | Verdict |
|---|---|---|---|---|
| MCP only (`--permission-prompt-tool`) | no: an MCP server cannot push to a running Claude Code | no: the flag is called in print mode only, the interactive UI accepts and ignores it (finding 6) | no | cannot cover the interactive Agent |
| Hooks only | no | yes: the hook's reply is the decision, in the interactive UI (finding 1) | no: no hook event fires on Esc (finding 5) | covers permissions only |
| Hooks plus MCP plus the existing keystroke path | paste plus `\r` in one write, sent when a hook says ready (H11) | the hook's reply (permission-cards.md) | Esc, acknowledged by the title (H13) | covers everything with the TUI kept |
| `claude -p` stream-json input | yes, structured | yes, `--permission-prompt-tool` works there | yes, a message | replaces the interactive UI, which is the product; out of scope without the user |
| Typed `y` or `n` into the dialog | | desync (the user can answer first, the hook is not signalled, findings 2, 4) | | rejected for permissions |

Recommendation: hooks plus the existing MCP server plus one keystroke path with an observable acknowledgement. Every Agent-to-user path goes through MCP (`ask_user`, H14); every user-to-Agent permission answer goes through the hook (Cards, H2 to H4); a Steer and an Interrupt are the only two writes to a Terminal on behalf of the Daemon, each with an acknowledgement and a typed error (H11, H13, H16).

## Seam under `contracts/` (rule 4)

`agent.prompt {id, text}`, `agent.interrupt {id}`, the errors `BUSY`, `NOT_ACCEPTED`, `NOT_RUNNING`, `NOT_ACKED`, and the Card store shared with permission Cards and `ask_user` (`card.list`, `card.answer`, `card.opened`, `card.cleared`). One store, one `Card` type, so the UI draws one thing. architect-b's steering series builds on `agent.prompt` and the Route rules; it owns when a Steer is sent and by whom. This document owns how it is delivered.

## Fork cost (measured)

`rup signal` costs 3.8 ms wall and 4.3 ms CPU per call on this Mac (measured by architect-swarm), about 0.9 s of CPU per 100 tool uses, each with a `PreToolUse` and a `PostToolUse` call. H1b or the H15 Builder repeats the measurement and commits the script. The cheap fix is H15: a single-threaded runtime in `rup`, and dropping an event only when a replay shows every Status unchanged without it. A long-lived transport (an `http` hook, an `mcp_tool` hook) works in the report (finding 7) but adds a second path into the Daemon and fails silently when the server is unreachable (the dialog appears with no message), so it is not chosen.

## Failure modes

- A lost `UserPromptSubmit`: `NOT_ACCEPTED`, text stays in the input line, no retry (H11, H16).
- A lost title: `NOT_ACKED`, Kind unchanged (H13).
- A hook that fails: Claude Code shows its own dialog without a message (finding 1); the Card is `answerable` false or the failure is the stderr line (H8).
- Startup: `text` plus `\r` in one write failed to submit once right after startup; a bracketed paste plus `\r` submitted 8 of 8 times (finding 4). H11 uses the paste form and verifies.
- Identity: a process inside an Agent's Terminal can reach the Daemon's socket, so answering a Card needs a secret the Agent cannot read (H4).

## Principles served

P1 (only the user answers a Card; steering follows the Thread rules of architect-b's series), P3 (one Card per `needs-you` Agent, `ask_user` included), P4 (the vendor strings, the paste markers and the Esc byte stay in `crates/agents/src/claude_code/`; `agent.prompt` and `agent.interrupt` are the existing inject seam with a result, not a fourth seam).

## Open

- Whether a Steer sent while the Agent is `working` is queued by Claude Code (not observed; H1b).
- `Takeover`: a Steer during a Takeover is held by the mailbox series; this series sends nothing then.
