# roundup — domain language

Glossary only. No implementation. Rule 6 of the loop: every identifier, RPC method and UI string uses a term from this file or is added here in the same PR. A CI script greps public names against the _Avoid_ words, except inside `crates/agents/claude_code/**`, where Claude Code's own terms (session, hook, transcript) are allowed.

## Terms

- **Project** — a folder roundup is opened on; owns its Agents, Terminals, Todos, Pads and Routes.
- **Agent** — one Claude Code process that roundup started and supervises; has a Status. _Avoid:_ session, bot, worker.
- **Terminal** — a plain shell process roundup started; no Status beyond running/exited. _Avoid:_ process, console.
- **Group** — a named, process-less node that holds Agents, Terminals, Todos, Pads, views and Groups; the user places what is in it. _Avoid:_ folder.
- **Home** — the Group or Meta-agent an item (an Agent, Terminal, Todo, Pad or any later view or tool) sits in; the Project is the root Home. Each item has exactly one Home, and the user can change any item's Home.
- **Meta-agent** (the user's own term) — a Group promoted so that a live Agent sits at it and receives its children's events. The Agent places the children it makes; the user's own move always wins. Any Group can be promoted. _Avoid:_ parent agent, lead.
- **Rail** — the tree of Groups, Meta-agents, Agents and Terminals. **Shelf** — the Todo and Pad column; it shows the selected node's Home, with a one-click view of the whole Project. **Inbox** — the time-ordered drawer of Messages to the user.
- **Switcher** — the field `⌘K` opens over the centre pane; it lists every Rail node by name and selects one.
- **Status** — `{kind, label, since}`. **Kind** — one of `error | needs-you | blocked | working | idle | done`, listed most urgent first. **Chip** — extra label after a name; never changes the glyph.
- **Glyph** — the one mark a row shows for its Kind: `✕ ● ⏸ ○ · ✓`. **Ink** — bold plus colour, used only for `error` (red) and `needs-you` (amber). **Live line** — the second line under a Rail row: the Status label and how long it has held.
- **App** — the macOS window. It starts a Daemon for one Project and hosts the webview, which is a client of that Daemon like any other. **App seam** — the few Tauri commands and events between the App and its webview (`scenarios/app.md`); not a Daemon contract. **Drawer** — a panel that slides in from the right over the terminal and the Shelf (a Todo, a Pad, later the Inbox); it never resizes the terminal.
- **Observation** — one input an adapter reads about its Agent: a terminal title, a Signal, an exit, a stop roundup itself asked for, or a clock tick. **Signal** — a structured event the Agent's own tooling pushes (for Claude Code, a hook payload). _Avoid:_ notification, alert.
- **Thread** — the conversation the user has with one Meta-agent about its Home: its Agent breaks down objectives, routes work to its children, absorbs their chatter and gives one summary when the work settles. _Avoid:_ chat, main agent.
- **Door** (name provisional) — the Thread as the default way in. Never the only way: the user can always open any Agent and type into it.
- **Takeover** — the user typing into an Agent directly. During a Takeover every Actor but the user sends that Agent nothing; its auto Messages wait as Held and deliver when the Takeover ends; ask-first Messages stay Held for the user; drop-route Messages are dropped.
- **Card** — the one interruption: a Message of kind `card` in the Inbox holding a single blocking question and the answers that unblock the Agent. Every Agent with Kind `needs-you` has exactly one open Card; answering clears it. _Avoid:_ popup.
- **Token** — one named look value in `apps/desktop/src/tokens.css` (a colour, type step, space step, radius, shadow or duration); no other file holds such a literal. **Signifier** — what shows that something can be clicked: its hover surface, `pointer` cursor and focus ring. **Check** — one pass-or-fail rule of `docs/design-system.md` (`D1` to `D10`). **Held-out screen** — a screen the Design critic runs that a Builder's prompt does not list.
- **Todo** — an item with an optional blocker list, with a Home. _Avoid:_ task, ticket.
- **Stall** — a condition that stops the loop's work (`scenarios/loop.md` L28): main or a required check red, an approved PR unmerged, a third reject, a finished review with no verdict, too few VMs while a PR waits. It is recorded in `loop/out/stalls/` with one owner and a deadline. **Owner** — one loop role (`docs/development-loop.md`, and the Merger in `.work/queue.md`): Triage, Merger, Architect or Driver.  A Todo's blockers are other Todos and are not Stalls.
- **Channel** — whether `rup mcp <id>` (the `roundup` MCP server, A11) has reported to the Daemon for one Agent, by `agent.channelUp` after its `daemon.identify`. It holds no standing connection: each tool call connects again. Its state is `pending`, `up` or `missing`. Not the vendor's own "channels" feature, which roundup does not use.
- **Brief** — the text roundup gives an Agent when it starts, in two parts: a file holding its id, that roundup supervises it and which tools reach roundup; and a start text holding its Home, who to ask, its peers and its open Todos, which the Agent can ask for again (`scenarios/awareness.md`, `docs/awareness.md`).
- **Pad** — a markdown note owned by an Agent or the user. _Avoid:_ scratchpad. "Scratchpad" is the product word in prose only; code says Pad.
- **Message** — a typed envelope `{from, to, kind, body, replyTo}` between Actors (a `passedFrom` field names the Message it was passed on from; `message.pass` passes a question to the next Meta-agent), with a status of `pending`, `held`, `delivered` or `dropped`. **Route** — a sender→receiver pair with a delivery value (`auto | ask-first | drop`). **Held** — a Message waiting on an ask-first Route, on the end of a Takeover, or on the user's answer to an escalated question.
- **Actor** — the user, an Agent, an Extension or the Daemon (as `rupd`, when it passes a question on) making a call. **Touch** — one logged read or write of a Todo, a Pad or a Message by an Actor. **Provenance** — the append-only log of Touches.
- **Extension** — a directory with a manifest and one module. **Hook** — one `($, e, next)` function an Extension registers. **Daemon** (`rupd`) — the local process everything else is a client of. **Adapter** — the code that turns one agent vendor's process into an Agent (`AgentAdapter`).

## Who decides Kind

| Kind | Decided by |
|---|---|
| `needs-you`, `working`, `idle` | the Adapter, from Observations |
| `error`, `done` | the Adapter, from exit (error = non-zero exit or crash) |
| `blocked` | the Daemon, from Todos and Routes — never the Adapter |

Hooks may add labels and Chips, never change Kind. The glyph is the most urgent Kind.
