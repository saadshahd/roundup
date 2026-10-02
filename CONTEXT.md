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
- **Thread** — the one conversation the user has with roundup: it breaks down objectives, routes work to Agents, absorbs their chatter and gives one summary when the work settles. Served by a live Agent. _Avoid:_ chat, main agent.
- **Door** (name provisional) — the Thread as the default way in. Never the only way: the user can always open any Agent and type into it.
- **Takeover** — the user typing into an Agent directly. During a Takeover, the Thread sends that Agent nothing; its Messages wait as Held and deliver when the Takeover ends.
- **Card** — the one interruption: a single blocking question and the answers that unblock the Agent, shown in the Inbox and held in the Card store, not a Message. Only the user answers a permission Card. Every Agent with Kind `needs-you` has exactly one open Card; answering clears it. _Avoid:_ popup.
- **Steer** — send a prompt to a running Agent through the Adapter, not through the user's keyboard. **Interrupt** — end the Agent's current turn through the Adapter; the Agent stays alive and its Kind becomes `idle`. Both are the Daemon's own calls (`agent.prompt`, `agent.interrupt`); typing into the Terminal is a Takeover, never a Steer. _Avoid:_ inject, nudge, cancel, abort.
- **Token** — one named look value in `apps/desktop/src/tokens.css` (a colour, type step, space step, radius, shadow or duration); no other file holds such a literal. **Signifier** — what shows that something can be clicked: its hover surface, `pointer` cursor and focus ring. **Check** — one pass-or-fail rule of `docs/design-system.md` (`D1` to `D10`). **Held-out screen** — a screen the Design critic runs that a Builder's prompt does not list.
- **Todo** — an item with an optional blocker list, owned by the Project. _Avoid:_ task, ticket.
- **Stall** — a condition that stops the loop's work (`scenarios/loop.md` L28): main or a required check red, an approved PR unmerged, a third reject, a finished review with no verdict, too few VMs while a PR waits. It is recorded in `loop/out/stalls/` with one owner and a deadline. **Owner** — one loop role (`docs/development-loop.md`, and the Merger in `.work/queue.md`): Triage, Merger, Architect or Driver.  A Todo's blockers are other Todos and are not Stalls.
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
