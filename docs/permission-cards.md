# Permission Cards

A Claude Code permission prompt becomes a Card (`CONTEXT.md`). The user answers it in the App; the answer reaches Claude Code through its `PermissionRequest` hook. Scenarios: `scenarios/cards.md` (H1 to H9).

## What exists

- The hook already reaches the Daemon: `rup signal` delivers each hook payload as `agent.signal`, and `PermissionRequest` gives the Kind `needs-you` (ADR 0006, A1).
- The hook cannot reply: `rup signal` prints nothing and returns at once, with a 5-second `timeout` on every event (`crates/agents/src/claude_code/launch.rs`).
- Missing: a Card type, `card.*` methods and events, a Card store, a hook that waits for an answer, and any UI.

## Flow

1. Claude Code runs `rup permission <agent-id>` for `PermissionRequest`, payload on stdin.
2. `rup` calls `agent.permission {id, payload}` and waits.
3. The Daemon's Claude Code adapter turns the payload into a Card (tool, args) and opens it; the Agent's Kind is `needs-you` while it is open.
4. The user calls `card.answer {id, answer}`. The adapter builds the reply text; the call from step 2 returns `{output}`; `rup` prints it unchanged and exits 0.
5. Anything else (the Agent ends, the hook's connection closes, a newer request replaces it, the Daemon is unreachable) clears the Card or makes `rup` exit 1 with a line on stderr. Claude Code then draws its own dialog. Nothing ever allows by default.

## Seam (contract change, rule 4)

New under `crates/contracts`: type `Card {id, agent, tool, args, opened_at}`; methods `agent.permission {id, payload} -> {output}`, `card.list -> [Card]`, `card.answer {id, answer} -> null`; events `card.opened`, `card.cleared {id, outcome}`. `answer` is `allow` or `deny`. `outcome` is `allow`, `deny`, `replaced`, `agent-gone` or `terminal`. `card.answer` accepts only an Actor of kind `user`, else `FORBIDDEN`. Every caller changes in the same PR: `rup permission`, the adapter, the webview's fake App, `contracts/generated/`.

## Modules

| Module | Holds |
|---|---|
| `crates/contracts` | the types, methods and events above |
| `crates/agents/src/card.rs` | the Card store: one open Card per Agent, cleared on exit and stop, memory only |
| `crates/agents/src/claude_code/` | payload to Card, answer to reply text, the settings hook line; the only place that names a vendor string |
| `crates/rup` | the `permission` subcommand: vendor-blind, prints what the Daemon returns |
| `apps/desktop` | the Card in the UI (written by architect-b, ids from the `U` ranges) |
| `spikes/hooks-permission` | H1: the real reply strings, recorded before any code |

## Principles served

`PRINCIPLES.md` is not on main yet; ids are from the user's draft.

- P3 Cards over chatter: one valid Card per `needs-you` Agent; no bell, no alert. Gate: does anything interrupt except a valid Card? No. Does routine progress yield `needs-you`? No: only a permission request opens a Card.
- P4 Black-box harness: the vendor payload and reply live in `crates/agents/src/claude_code/`; `rup` and the contract carry opaque text. Gate: does code outside that directory read a vendor string? No.
- P1 Single front door: the user answers the Card directly. The Thread never answers one (H4).

## Limits and open questions

- An App quit ends the Daemon and its Agents (D1), so a Card cannot survive it. H5 covers a webview reload.
- Claude Code's reply shape, the longest `timeout`, and what happens when the user answers the dialog first are unverified. H1 records them against the installed `claude`; nothing else starts before it.
- Out of scope: "always allow" rules, edited tool input, deny messages beyond what H1 shows works.
