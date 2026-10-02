# Messages between Actors

Design for `scenarios/messages.md` (B1 to B23) and A30. Serves `PRINCIPLES.md` P1, P3 and P4 (the ids in `PRINCIPLES.md`, not `pads.md`'s). Terms are in `CONTEXT.md`; nothing here adds a term.

## What exists and what does not

Exists: the `Delivery` type and the `bus.route` Hook type (`contracts/hooks.ts`), the "Message delivery" paragraph of `docs/architecture.md`, Routes named in ADR 0004, the Inbox wireframe, and `type_prompt` in `crates/agents`, the only path that types into an Agent's Terminal. Missing: a Message type, a store, RPC methods, an MCP tool, and any delivery logic.

## Status of a Message

| Status | Meaning | Leaves it by |
|---|---|---|
| `pending` | to be typed at the receiver's next `idle` | typed (`delivered`), or the receiver exits (`dropped`) |
| `held` (`ask-first`, `takeover` or `escalated`) | waits for the user, or for a Takeover to end | `message.deliver`, `message.drop`, the end of a Takeover (only `takeover`), the user's answer (only `escalated`) |
| `delivered` | typed into the Terminal, or stored for the user | final |
| `dropped` | never typed, with a reason | final |

Default Route is `auto`. A Held Message never blocks a later one (B7). Delivery is at most once (B8). `blocked` is unchanged: this series gives Routes no say in Kind (`CONTEXT.md`, "Who decides Kind").

## Modules

| Module | Change |
|---|---|
| `crates/contracts` | `Message` (with `passedFrom`), `MessageKind`, `MessageStatus`, `Held` reason, `Route`, `Delivery` (moved out of `contracts/hooks.ts`, which imports it); methods `message.send`, `message.pass`, `message.get`, `message.list`, `message.deliver`, `message.drop`, `route.set`, `route.list`, `takeover.begin`, `takeover.end`; events `message.sent`, `message.held`, `message.delivered`, `message.dropped`, `route.changed`, `takeover.changed`; `generated/` is regenerated, never edited |
| `crates/messages` (new) | stores Messages and Routes in `.roundup/roundup.db` (ADR 0004), holds Takeovers in memory, listens to `agent.status` on the bus, and types through `Agents::prompt` |
| `crates/agents` | A30: `Agents::prompt`, the Adapter's input seam, made callable; no fourth seam (P4) |
| `crates/rupd` | registers the module |
| `crates/rup` | MCP tools `message_send`, `message_get`, `message_list`, `message_pass` (B12) |
| `crates/provenance` | none: `item` is a free string, `message:<id>` joins `todo:` and `pad:` (B11) |

Bounds, from the codes `rpc::code` already has: 32 `pending` or `held` Messages per receiver and 8192 bytes of body (`CONFLICT` and `INVALID_PARAMS`). No new error code.

## The gates

- **P1.** No new path makes the user address an Agent instead of the Thread, or makes an Agent reachable only through the Thread. During a Takeover nothing but the user's own Messages reaches the Agent (B6).
- **P3.** Nothing here interrupts the user. A Message to the user is stored, changes no Kind and carries no signal beyond the event (B10). The Inbox UI is not in this series; when it inks a badge for Held Messages it must answer this gate itself.
- **P4.** The `messages` crate reads no vendor string; typing goes through the Adapter seam that exists.

## Escalation

Serves P1 and P3. P1: the user can still address any Agent, and a question never reaches the user through an Agent that owns the Thread's place. P1 also: a hop `held` for a Takeover waits, so a Takeover is never cut short. P3: the landing is a Held Message, with no signal beyond the event, and a permission Card skips the chain, so only a valid Card interrupts. Added contract: `message.pass`, `passedFrom` on `Message`, and the Held reason `escalated`; the clock is injected, as B14's fake clock needs. A Meta-agent's hop uses the Meta-agent's own Agent, so nothing here adds a seam.

## Child summaries

Serves P1 and P3. P3: pushes go to a Meta-agent as `note` Messages, never to the user, and change no Kind. P1: a Meta-agent learns its children through a tool and Messages, so nothing makes the user address the Thread through it. P4: the envelope reads Status labels, Messages, Todos and Pads, never Terminal output or a vendor string. Added contract: `agent.summary` and the type `Summary`; the MCP tool `agent_summary` is offered to Meta-agents only. Tool set per kind of Agent, for this PR alone: M1's tools and B12's four for every Agent, plus `agent_summary` for a Meta-agent. The composer is `crates/messages`, which listens to `agent.status` and the Todo and Pad events; it reads no new input.

## Contract timing

The contract is not a separate PR: types and methods with no caller would be unused exports (`AGENTS.md` rule 2). It lands with the first Builder PR below, as A16's did, and rule 4 applies to that PR: an architect other than its author approves it. `docs/extension-interface.md` now says `to: Actor`; Group fan-out and the `bus.route` Hook are not in this series.

## Open

Two choices came from the user's ask and are settled in `scenarios/messages.md`: Takeover starts and ends by an explicit call from the webview (B6), and a Held Message does not block a later one (B7).
