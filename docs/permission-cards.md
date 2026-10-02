# Permission Cards

Status: provisional. H1 is recorded (`spikes/hooks-permission/REPORT.md`); the design is final only after H1b and the control-channel comparison report back.

A Claude Code permission prompt becomes a Card (`CONTEXT.md`). The user answers it in the App; the answer reaches Claude Code through its `PermissionRequest` hook. Scenarios: `scenarios/cards.md` (H1 to H10).

## What exists

- The hook already reaches the Daemon: `rup signal` delivers each hook payload as `agent.signal`, and `PermissionRequest` gives the Kind `needs-you` (ADR 0006, A1).
- The hook cannot reply: `rup signal` prints nothing and returns at once, with a 5-second `timeout` on every event (`crates/agents/src/claude_code/launch.rs`).
- Missing: a Card type, `card.*` methods and events, a Card store, a hook that waits for an answer, and any UI.

## Flow

1. Claude Code runs `rup permission <agent-id>` for `PermissionRequest`, payload on stdin.
2. `rup` calls `agent.permission {id, payload}` and waits.
3. The Daemon's Claude Code adapter turns the payload into a Card (tool, args) and opens it; the Agent's Kind is `needs-you` while it is open.
4. The user calls `card.answer {id, answer}`. The adapter builds the reply text; the call from step 2 returns `{output}`; `rup` prints it unchanged and exits 0.
5. Anything else clears the Card or makes `rup` exit 1 with a line on stderr, and Claude Code then draws its own dialog (H1: it does so silently, so the line is the only trace). Nothing ever allows by default. The Kind after each outcome: `working` for an answer, `working` or `idle` for a Terminal answer (H7), the Agent's own for `agent-gone`, and `needs-you` for `replaced`.
6. The user can answer the dialog in the Terminal first. H1 shows the hook then keeps running, so the Card is cleared by the next Signal from the Agent (`PostToolUse`, `Stop` and the like) and by the hook's SIGTERM after Esc (H7), never by a timer.

## Seam (contract change, rule 4)

New under `crates/contracts`: type `Card {id, agent, tool, args, opened_at}`; methods `agent.permission {id, payload} -> {output}`, `card.list -> [Card]`, `card.answer {id, answer} -> null`; events `card.opened`, `card.cleared {id, outcome}`. `answer` is `allow` or `deny`. `outcome` is `allow`, `deny`, `replaced`, `agent-gone` or `terminal`. `card.answer {id, answer, proof}` needs the `proof` of H4, else `FORBIDDEN`: the Actor of a connection comes from the connection and an Agent's Terminal can claim `user`, so identity is a secret the App holds and the Daemon strips from every Agent environment. `Card` also has `answerable` (false when the Daemon opened it without being able to reply, H8). Every caller changes in the same PR: `rup permission`, the adapter, the webview's fake App, `contracts/generated/`.

## Modules

| Module | Holds |
|---|---|
| `crates/contracts` | the types, methods and events above |
| `crates/agents/src/card.rs` | the Card store: one open Card per Agent, cleared on exit and stop, memory only |
| `crates/agents/src/claude_code/` | payload to Card, answer to reply text, the settings hook line; the only place that names a vendor string |
| `crates/rupd`, `crates/desktop` | the `proof` handshake: the App makes it, gives it to the Daemon on stdin, and the Daemon removes it from child environments |
| `crates/rup` | the `permission` subcommand: vendor-blind, prints what the Daemon returns |
| `apps/desktop` | the Card in the UI (written by architect-b, ids from the `U` ranges) |
| `spikes/hooks-permission` | H1 and H1b: the real reply strings and behaviour |

## Principles served

- P3 Cards over chatter: one valid Card per `needs-you` Agent; no bell, no alert. Gate: does anything interrupt except a valid Card? No. Does routine progress yield `needs-you`? No: only a permission request opens a Card.
- P4 Black-box harness: the vendor payload and reply live in `crates/agents/src/claude_code/`; `rup` and the contract carry opaque text. Gate: does code outside that directory read a vendor string? No.
- P1 Single front door: the user answers the Card directly. The Thread never answers one (H4), and a permission Card never climbs the escalation chain (H10).

## Limits and open questions

- An App quit ends the Daemon and its Agents (D1), so a Card cannot survive it. H5 covers a webview reload.
- H1 and H1b recorded the reply shape, the timeout, Esc, concurrent requests and the `Notification` timing. Still unobserved: option "No", Tab-amend, two dialogs open at once, and macOS.
- A Daemon that outlives the App (so a Card survives a quit) is out of scope: the user ruled that a Card survives a webview reload and not an App quit (D1, S3).
- Out of scope: "always allow" rules, edited tool input, deny messages beyond what H1 shows works.
