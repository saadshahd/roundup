# Permission Decisions

Status: provisional. H1 is recorded (`spikes/hooks-permission/REPORT.md`); the design is final only after H1b and the control-channel comparison report back.

A Claude Code permission prompt becomes a Decision (`GLOSSARY.md`). The user answers it in the App; the answer reaches Claude Code through its `PermissionRequest` hook. Scenarios: `scenarios/decisions.md` (H1 to H10).

## What exists

- The hook already reaches the Daemon: `rup signal` delivers each hook payload as `agent.signal`, and `PermissionRequest` gives the Kind `needs-you` (ADR 0006, A1).
- The hook cannot reply: `rup signal` prints nothing and returns at once, with a 5-second `timeout` on every event (`crates/agents/src/claude_code/launch.rs`).
- Missing: the handlers for `agent.permission` and `decision.*` (their types are in `crates/contracts/src/decision.rs`), a Decision store, `rup permission`, and any UI.

## Flow

1. Claude Code runs `rup permission <agent-id>` for `PermissionRequest`, payload on stdin.
2. `rup` calls `agent.permission {id, payload}` and waits.
3. The Daemon's Claude Code adapter turns the payload into a Decision (tool, args) and opens it; the Agent's Kind is `needs-you` while it is open.
4. The user calls `decision.answer {id, answer}`. The adapter builds the reply text; the call from step 2 returns `{output}`; `rup` prints it unchanged and exits 0.
5. Anything else clears the Decision or makes `rup` exit 1 with a line on stderr, and Claude Code then draws its own dialog (H1: it does so silently, so the line is the only trace). Nothing ever allows by default. The Kind after each outcome: `working` for an answer, `working` or `idle` for a Terminal answer (H7), the Agent's own for `agent-gone`, and `needs-you` for `replaced`.
6. The user can answer the dialog in the Terminal first. H1 shows the hook then keeps running, so the Decision is cleared by the next Signal from the Agent (`PostToolUse`, `Stop` and the like) and by the hook's SIGTERM after Esc (H7), never by a timer.

## Seam (contract change, rule 4)

New under `crates/contracts`: type `Decision {id, agent, tool, args, opened_at}`; methods `agent.permission {id, payload} -> {output}`, `decision.list -> [Decision]`, `decision.answer {id, answer} -> null`; events `decision.opened`, `decision.cleared {id, outcome}`. `answer` is `allow` or `deny`. `outcome` is `allow`, `deny`, `replaced`, `agent-gone` or `terminal`. `decision.answer {id, answer, proof}` needs the `proof` of H4, else `FORBIDDEN`: the Actor of a connection comes from the connection and an Agent's Terminal can claim `user`, so identity is a secret the App holds and the Daemon strips from every Agent environment. `Decision` also has `answerable` (false when the Daemon opened it without being able to reply, H8). Every caller changes in the same PR: `rup permission`, the adapter, the webview's fake App, `contracts/generated/`.

## Modules

| Module | Holds |
|---|---|
| `crates/contracts` | the types, methods and events above |
| `crates/agents/src/decision.rs` | the Decision store: one open Decision per Agent, cleared on exit and stop, memory only |
| `crates/agents/src/claude_code/` | payload to Decision, answer to reply text, the settings hook line; the only place that names a vendor string |
| `crates/rupd`, `crates/desktop` | the `proof` handshake: the App makes it, gives it to the Daemon on stdin, and the Daemon removes it from child environments |
| `crates/rup` | the `permission` subcommand: vendor-blind, prints what the Daemon returns |
| `apps/desktop` | the Decision in the UI (written by architect-b, ids from the `U` ranges) |
| `spikes/hooks-permission` | H1 and H1b: the real reply strings and behaviour |

## Principles served

- P3 Cards over chatter (the user's title; the blocking kind is a Decision): one valid Decision per `needs-you` Agent; no bell, no alert. Gate: does anything interrupt except a valid Decision? No. Does routine progress yield `needs-you`? No: only a permission request opens a Decision.
- P4 Black-box harness: the vendor payload and reply live in `crates/agents/src/claude_code/`; `rup` and the contract carry opaque text. Gate: does code outside that directory read a vendor string? No.
- P1 Single front door: the user answers the Decision directly. The Thread never answers one (H4), and a permission Decision never climbs the escalation chain (H10).

## Limits and open questions

- An App quit ends the Daemon and its Agents (D1), so a Decision cannot survive it. H5 covers a webview reload.
- H1 to H1d recorded the reply shape, the timeout, Esc, No, always-allow, Tab-amend, concurrent requests, late replies, `AskUserQuestion` and the `Notification` timing (report findings 1 to 23). Still unobserved: whether "always allow" persists, two dialogs open at once, what `AskUserQuestion` returns after an answer, whether Esc on its choice UI changes the title, and macOS.
- A Daemon that outlives the App (so a Decision survives a quit) is out of scope: the user ruled that a Decision survives a webview reload and not an App quit (D1, S3).
- Out of scope: "always allow" rules, edited tool input, deny messages beyond what H1 shows works.
