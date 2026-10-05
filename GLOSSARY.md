# roundup

roundup supervises coding Agents in one Project from a macOS App. Names use these terms (`AGENTS.md` rule 6); _Avoid_ words are allowed only under `crates/agents/claude_code/`, where Claude Code's own terms live.

## Language

### Project and its nodes

**Project**:
A folder roundup is opened on; it owns its Agents, Terminals, Todos, Pads and Routes.

**Agent**:
One Claude Code process that roundup started and supervises; it has a Status.
_Avoid_: session, bot, worker

**Terminal**:
A plain shell process roundup started; no Status beyond running or exited.
_Avoid_: process, console

**Snapshot**:
A bounded copy of a Terminal's screen and output byte offset that restores its pane after a webview reload.

**Group**:
A named, process-less node holding Agents, Terminals, Todos, Pads, views and Groups; the user places what is in it.
_Avoid_: folder

**Home**:
The one Group or Meta-agent an item sits in; the Project is the root Home.

**Meta-agent**:
A Group promoted so a live Agent sits at it and receives its children's events. The user's own term.
_Avoid_: parent agent, lead

**Worktree**:
The git worktree roundup makes for one Agent when the Project's `worktrees` setting is on: its own directory, branch and working directory (`docs/worktrees.md`).

**Base**:
The branch checked out in the Project when the Agent spawned; the Agent's branch starts there.

**Landing**:
Rebasing an Agent's branch onto its Base, running the Project's check, then fast-forwarding the Base.

### The App

**App**:
The macOS window; it starts a Daemon for one Project and hosts the webview, a client of that Daemon like any other.

**App seam**:
The few Tauri commands and events between the App and its webview (`scenarios/app.md`); not a Daemon contract.

**Rail**:
The tree of Groups, Meta-agents, Agents and Terminals.

**Shelf**:
The Todo and Pad column; it shows the selected node's Home, with a one-click view of the whole Project.

**Inbox**:
The time-ordered Drawer of Messages to the user.

**Drawer**:
A panel that slides in from the right over the terminal and the Shelf (a Todo, a Pad, later the Inbox); it never resizes the terminal.

**Switcher**:
The field `⌘K` opens over the centre pane, listing every Rail node by name to select one.

**Help**:
The panel `?` opens over the centre pane, listing each chord and what it does.

**Todo**:
An item with an optional blocker list of other Todos, with a Home.
_Avoid_: task, ticket

**Pad**:
A markdown note owned by an Agent or the user. "Scratchpad" is the product word in prose only.
_Avoid_: scratchpad

**Editor**:
A Pad's formatted Markdown editing surface.

**Source**:
A Pad's syntax-highlighted Markdown field for direct text editing.

**Preview**:
The rendered view of a Pad's draft while the user edits; the reading view uses the same rules.

**Attachment**:
An image pasted or dropped into the Thread's input, shown inline as its name and size with a hover preview; not a Chip.

**Quote**:
Selected text attached to the Thread's input as a block with its source.

### Status

**Status**:
`{kind, label, since}`.

**Kind**:
One of `error | needs-you | blocked | working | idle | done`, most urgent first; the Adapter decides all but `blocked`, which the Daemon decides from Todos and Routes. Hooks never change it.

**Glyph**:
The one mark a row shows for its most urgent Kind: `✕ ● ⏸ ○ · ✓`.

**Ink**:
Bold plus colour, used only for `error` (red) and `needs-you` (amber).

**Chip**:
An extra label after a name; it never changes the Glyph.

**Live line**:
The Status label and how long it has held, on a Rail row's own line.

### Agents at work

**Adapter**:
The code that turns one agent vendor's process into an Agent (`AgentAdapter`).

**Observation**:
One input an Adapter reads about its Agent: a terminal title, a Signal, an exit, a stop roundup asked for, or a clock tick.

**Signal**:
A structured event the Agent's own tooling pushes (for Claude Code, a hook payload).
_Avoid_: notification, alert

**Steer**:
Sending a prompt to a running Agent through the Adapter (`agent.prompt`), never through the user's keyboard.
_Avoid_: inject, nudge

**Interrupt**:
Ending the Agent's current turn through the Adapter (`agent.interrupt`); the Agent stays alive and becomes `idle`.
_Avoid_: cancel, abort

**Takeover**:
The user typing into an Agent directly; meanwhile no other Actor sends it anything.

**Brief**:
The text roundup gives an Agent at start about itself, its Home, its peers and its open Todos (`docs/awareness.md`).

**Channel**:
Whether one Agent's `roundup` MCP server (`rup mcp <id>`) has reported to the Daemon: `pending`, `up` or `missing`. Not the vendor's own channels feature.

**Evaluator**:
A small model run inside the Daemon for a routine judgement at no frontier-model cost; a spike (B27), not yet built.

### Messages and Cards

**Thread**:
The conversation the user has with one Meta-agent about its Home.
_Avoid_: chat, main agent

**Door**:
The Thread as the default way in, never the only one: the user can always open any Agent and type into it. Name provisional.

**Message**:
A typed envelope `{from, to, kind, body, replyTo}` between Actors, with a status of `pending`, `held`, `delivered` or `dropped`.

**Digest**:
The fixed, bounded entry `{name, kind, last, todos, pads}` telling a Meta-agent about one child.

**Route**:
A sender→receiver pair with a delivery value: `auto | ask-first | drop`.

**Held**:
A Message waiting on an ask-first Route, the end of a Takeover, or the user's answer to an escalated question.

**Reason**:
The word on a held or dropped Message saying why: `ask-first`, `takeover` or `escalated` (held); `receiver gone` or `not accepted` (dropped).

**Card**:
Anything the user sees in the app (a permission request, a question, an Inbox Message, a status, a summary); it has a kind.
_Avoid_: popup

**Decision**:
The kind of Card that blocks an Agent on a live hook or tool call; only the user answers it.

**Actor**:
The user, an Agent, an Extension or the Daemon (as `rupd`) making a call.

**Touch**:
One logged read or write of a Todo, a Pad or a Message by an Actor.

**Provenance**:
The append-only log of Touches.

### Daemon and Extensions

**Daemon**:
`rupd`, the local process everything else is a client of.

**Extension**:
A directory with a manifest and one module.

**Hook**:
One `($, e, next)` function an Extension registers.

### Design system

**Token**:
One named look value in `apps/desktop/src/tokens.css` (a colour, type step, space step, radius, shadow or duration); no other file holds such a literal.

**Signifier**:
What shows that something can be clicked: its hover surface, `pointer` cursor and focus ring.

**Check**:
One pass-or-fail rule of `docs/design-system.md` (`D1` to `D10`).

**Held-out screen**:
A screen the Design critic runs that no Builder's prompt lists.

### The loop

**Stall**:
A condition that stops the loop's work, with one Owner and a deadline; specified by L28 and not yet built. A Todo's blockers are not Stalls.

**Owner**:
The one loop role answering for a Stall: Triage, Merger, Architect or Driver.

**Merger**:
The Driver while it merges a PR.

**VM tool**:
A Claude Code plugin, skill, hook or MCP server the agent uses on a boxd VM; not an Extension.
_Avoid_: plugin
