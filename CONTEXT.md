# roundup — domain language

Glossary only. No implementation. Rule 6 of the loop: every identifier, RPC method and UI string uses a term from this file or is added here in the same PR. A CI script greps public names against the _Avoid_ words, except inside `crates/agents/claude_code/**`, where Claude Code's own terms (session, hook, transcript) are allowed.

## Terms

- **Project** — a folder roundup is opened on; owns its Agents, Terminals, Todos, Pads and Routes.
- **Agent** — one Claude Code process that roundup started and supervises; has a Status. _Avoid:_ session, bot, worker.
- **Terminal** — a plain shell process roundup started; no Status beyond running/exited. _Avoid:_ process, console.
- **Group** — a named, process-less node that holds Agents, Terminals and Groups. _Avoid:_ folder.
- **Meta-agent** (the user's own term) — a Group promoted so that a live Agent sits at it and receives its children's events. _Avoid:_ parent agent, lead.
- **Rail** — the tree of Groups, Meta-agents, Agents and Terminals. **Shelf** — the Todo and Pad column. **Inbox** — the time-ordered drawer of Messages to the user.
- **Switcher** — the field `⌘K` opens over the centre pane; it lists every Rail node by name and selects one.
- **Status** — `{kind, label, since}`. **Kind** — one of `error | needs-you | blocked | working | idle | done`, listed most urgent first. **Chip** — extra label after a name; never changes the glyph.
- **Glyph** — the one mark a row shows for its Kind: `✕ ● ⏸ ○ · ✓`. **Ink** — bold plus colour, used only for `error` (red) and `needs-you` (amber). **Live line** — the second line under a Rail row: the Status label and how long it has held.
- **App** — the macOS window. It starts a Daemon for one Project and hosts the webview, which is a client of that Daemon like any other. **App seam** — the few Tauri commands and events between the App and its webview (`scenarios/app.md`); not a Daemon contract. **Drawer** — a panel that slides in from the right over the terminal and the Shelf (a Todo, a Pad, later the Inbox); it never resizes the terminal.
- **Observation** — one input an adapter reads about its Agent: a terminal title, a Signal, an exit, a stop roundup itself asked for, or a clock tick. **Signal** — a structured event the Agent's own tooling pushes (for Claude Code, a hook payload). _Avoid:_ notification, alert.
- **Todo** — an item with an optional blocker list, owned by the Project. _Avoid:_ task, ticket.
- **Pad** — a markdown note owned by an Agent or the user. _Avoid:_ scratchpad. "Scratchpad" is the product word in prose only; code says Pad.
- **Message** — a typed envelope `{from, to, kind, body}` between Actors. **Route** — a sender→receiver pair with a delivery value (`auto | ask-first | drop`). **Held** — a Message waiting on an ask-first Route.
- **Actor** — the user, an Agent or an Extension making a call. **Touch** — one logged read or write of a Todo or Pad by an Actor. **Provenance** — the append-only log of Touches.
- **Extension** — a directory with a manifest and one module. **Hook** — one `($, e, next)` function an Extension registers. **Daemon** (`rupd`) — the local process everything else is a client of. **Adapter** — the code that turns one agent vendor's process into an Agent (`AgentAdapter`).

## Who decides Kind

| Kind | Decided by |
|---|---|
| `needs-you`, `working`, `idle` | the Adapter, from Observations |
| `error`, `done` | the Adapter, from exit (error = non-zero exit or crash) |
| `blocked` | the Daemon, from Todos and Routes — never the Adapter |

Hooks may add labels and Chips, never change Kind. The glyph is the most urgent Kind.
