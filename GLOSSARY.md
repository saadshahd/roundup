# roundup

roundup supervises coding Agents in one Project from a macOS App. Names use these terms (`AGENTS.md` rule 6); _Avoid_ words are allowed only under `crates/agents/src/claude_code/`, where Claude Code's own terms live.

## Language

### Project and its nodes

**Project**:
A folder roundup is opened on; it owns its Rooms, Agents, Terminals, Todos, Pads and Routes.

**Agent**:
One supervised identity on the Rail, with a Status; each launch of its program is an Attempt, and at most one runs at a time.
_Avoid_: session, bot, worker

**Terminal**:
A plain shell process roundup started; no Status beyond running or exited.
_Avoid_: process, console

**Snapshot**:
A bounded copy of a Terminal's screen and output byte offset that restores its pane after a webview reload.

**Room**:
The persistent place for one body of work, with exactly one Door and its Agents, Terminals, Todos and Sketches. Stopping its Door neither removes the Room nor moves its contents. Room nesting and cross-Room dependencies are open.
_Avoid_: folder

**Home**:
Where an item sits: a Room, or the Project root. Each item has one Home the user can change. The Shelf is still Project-wide; Room-scoped Todos and Sketches are follow-up work.

**Door**:
The coordinating Agent role of one Room, with a system prompt tuned to that role. Its Rail id is the Room id; its program may be starting, running or stopped without changing the Room. Direct access to any Agent remains (P1).
_Avoid_: meta-agent, parent agent, lead

**Attempt**:
One launch of an Agent's program, numbered per Agent from `1` in launch order (A20). It travels in the hook command, `SignalParams`, `StatusEvent` and `RailNode.attempt` as a canonical positive decimal string within SQLite's signed 64-bit range; a client may compare two of one Agent and reads nothing else from it. Never a Rail identity or a vendor conversation id; the last one survives stop, failure and reopen.

**Status revision**:
The strictly increasing ordinal of published Status transitions within one Attempt, `1` at Starting. A canonical positive decimal string on the wire; absent after reopen. Never stands in for an Attempt or Terminal liveness.

**Resume**:
Starting an exited Agent's next Attempt in its saved conversation.

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
The tree of Rooms, Agents and Terminals.

**Shelf**:
The Todo and Pad column, Project-wide for now; Room scoping is follow-up work.

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

**Sketch**:
The accepted destination for user- or Agent-made rich Markdown holding Mermaid diagrams or tldraw visuals; storage, embedding and ownership are open, and Pad contracts are unchanged.

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
The conversation with a Room's Door about that Room's work, not a second container; today it is the Door's Terminal.
_Avoid_: chat, main agent

**Message**:
A typed envelope `{from, to, kind, body, replyTo}` between Actors, with a status of `pending`, `held`, `delivered` or `dropped`.

**Digest**:
The fixed, bounded entry `{name, kind, last, todos, pads}` telling a Door about one child.

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

**Builder**:
The one kind of agent run that builds roundup and reviews it, each run fresh, from `.agents/builder.md`: a build run takes a Work row, a fix run answers a reject or a failure, and a review run judges a PR's head, never its author's rationale (L23, L24).

**Code PR**:
A PR touching `apps/`, `crates/`, `contracts/` or a Build file; it needs a `Scenarios:` line (L46).

**Build file**:
What `just check` runs and builds with: `justfile`, `Cargo.toml`, `Cargo.lock`, `package.json`, `pnpm-lock.yaml`, `pnpm-workspace.yaml`, `rust-toolchain.toml`, `tsconfig.json`, `.oxlintrc.json`, `.fallowrc.json`, `.cargo/` and `tools/` (L46).

**Loop machinery**:
`loop/`, `.github/`, `.agents/`, `.claude/`, and every `AGENTS.md` or `CLAUDE.md`; a review run reads `main`'s copy, never the PR's, and a PR touching `.github/` waits for the user's merge (L24, L46).

**Claim**:
The branch `build/<slug>` the build queue pushes at `main` to hold one Work row for its Builder, whose PR grows from it; a Builder that ends with no PR frees it (L23).

**Ledger**:
One row per agent run (role, subject, model, turns, tokens, exit), kept as that run's `ledger-*` artifact (L29); what a Retro reads.

**Status issue**:
The one open issue labelled `loop:status`, pinned and assigned to the user: what merged, the tokens per merged product PR, the PRs waiting on the user, the PRs stuck and what to watch. `status.yml` rewrites it each hour and posts it as a comment at 08:03 UTC (L80).

**Retro**:
One run, after every 20 merged PRs, that reads the Ledger and the rejects and opens one PR cutting what does not pay (L27); the user merges it.

**Percy build**:
Percy's Chromium render of every harness seed for one PR head, made by the `percy` job (L36); merge-ready needs it green on a PR touching `apps/desktop/src/` (L46). Its images are not Snapshots.
_Avoid_: proof branch
