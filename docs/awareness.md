# Agent awareness

Design for `scenarios/awareness.md` (E1 to E7). Term: Brief (`CONTEXT.md`). Series B (Messages, `docs/messages.md`) and the permission Cards (H) share the channel this uses; nothing here adds a second seam.

## What exists

- A4 starts Claude Code with a per-Agent `--settings` file (one `rup signal` hook) and A11 adds `--mcp-config` with a stdio server `rup mcp <id>`. That server is the user's own idea and the one seam from an Agent to the Daemon (A11): M1 gives it Todos and Pads, #160's B12 gives it Messages.
- `agent.signal`, the Rail tree (`parent`, `order`, `meta`), Todo `creator` (T8) and Messages (B1 to B12, in review) hold every fact an Agent needs about its team.
- Missing: telling the Agent at start, a way to ask again, and an Agent that never connects being visible.

## Injection points, from the vendor's docs

Read on 2026-10-02 (`code.claude.com/docs`): `--append-system-prompt-file` works in interactive mode (cli-reference); a `SessionStart` hook can return `additionalContext` that reaches the model (hooks); `--mcp-config` loads servers at spawn (cli-reference, mcp). Not documented, and so not relied on: `UserPromptSubmit` output fields, and when an interactive session connects its MCP servers. Mid-run pushes through "channels" are a research preview that needs Anthropic auth; roundup does not use them, since B2 types a Message at the next `idle`. E7 observes the three claims with the real `claude` before a Builder relies on them.

## Shape

| Need | Seam | Why |
|---|---|---|
| Who am I, what are my tools | the Brief, `--append-system-prompt-file` (E1) | static, so it never goes stale |
| Parent, peers, Todos at start | `SessionStart` hook, `rup context` printing `agent.brief` (E3) | the vendor's own injection point; live at the moment of start |
| The same again later | MCP tool `agent_context` (E4) | the Agent pulls when it wants; peers change (`rail.move`) |
| Ask the parent or a peer | `message_send` to `ask.to` or a peer id (E5) | B's channel; no second one |
| Never connected | `agent.channelUp`, `RailNode.channel` and the event `agent.channel` (E6); the Rail's `no channel` Chip is E8 | visible, never an interruption |

Not used: `CLAUDE.md`, because it is the user's file; screen scraping or typing the context into the Terminal, because `agent.context` is exact and typing would interleave with the user (P4's "black box" means the vendor is not parsed). One method serves all three readers: `agent.context`.

## Naming

"Team lead" is not a glossary term, and `CONTEXT.md` lists "lead" under Meta-agent's _Avoid_. The user decided: add no role word. An Agent's lead is the Agent at its parent **Meta-agent**, and when the parent is a plain Group the question goes to the user (E2's `ask`). One term is added, **Brief** (accepted by the user): the text roundup gives an Agent at start.

## Contract change (rule 4, with the first Builder PR)

`agent.context {id}` returns `{self, parent, ask, peers, todos}`; the MCP tool `agent_context` is its caller. `agent.brief {id}` returns `{stdout}`, the hook output built in `claude_code/`, which `rup context` prints verbatim, so `rup` holds no vendor string (P4), as `rup signal` forwards a vendor payload it does not parse. `agent.channelUp {id}` is called once by `rup mcp` at start (E6); `daemon.identify` is not used for it, because hooks identify too. E6 adds `RailNode.channel` (`pending`, `up`, `missing`, `null`) and the event `agent.channel`. The contract PR states every caller, including the webview's `RailNode` fixtures. E8, the Chip, is a webview scenario written after this lands. `rpc::code` gains nothing.

## The gates

- **P1.** An Agent can ask its Meta-agent (the Agent that serves the Thread about that Home, `CONTEXT.md`), a peer or the user, so asking the Meta-agent is how an Agent reaches the Thread. No Agent is reachable only through the Thread.
- **P3.** Nothing interrupts the user: `missing` changes no Kind and rings nothing, and a question to the user is B10's Inbox Message.
- **P4.** Everything Claude-specific (the flag, the hook JSON) sits in `claude_code/`: `rup context` prints a string the Daemon built and parses nothing. `agent.context` and the tool are vendor-neutral, and `AgentAdapter` gains no seam.

## Open

- "Its Todos" means Todos the Agent created (T8). A Todo has no assignee; if the user wants assignment, that is a Todo change, not this series.
- The 15 seconds of E6 is a guess until E7 shows when Claude Code connects.
