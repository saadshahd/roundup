# roundup

roundup supervises coding Agents in each open Project from a macOS App. Names use these terms (`AGENTS.md` rule 6); _Avoid_ words are allowed only under `crates/agents/src/claude_code/`, where Claude Code's own terms live.

## Language

### Code terms and user labels

**Code term**:
The name used by identifiers, types and RPC methods. Rule 6 applies to public code names.

**User label**:
App-authored words describing an action or content. A label may differ from its code term; authored names, ids, paths, code, Adapter labels, Agent output and Terminal content stay as written. Other domain terms keep their current user spelling until explicitly mapped.

**App copy catalogue** (`copy`):
The single collection of App-authored labels and parameterised prose, separate from code terms and authored values.

| Code term | User label in App prose |
| --- | --- |
| Door | supervisor |
| Takeover | control |
| Shelf | work items |
| Handoff | summary |

| Existing action label | User label |
| --- | --- |
| Start Door | Start supervisor |
| Retry Door | Retry supervisor |
| End Takeover | Give control back |
| Take over | Take control |
| Show Shelf | Show work items |
| Hide Shelf | Hide work items |
| Clear without Handoff | Clear without a summary |
| Clear (prompt submit) | Clear and keep a summary |

Clear elsewhere remains Clear. The summary submit writes a Handoff; the fallback does not.

### Project and its nodes

**Project**:
A folder roundup is opened on; it owns its Workstreams, Agents, Terminals, Todos, Pads and Routes.

**Project data**:
The databases, launch files and Pad files roundup keeps for a Project, in `<app data>/projects/<key>/` and never in the Project folder (N1); only Worktrees go in the Project's `.roundup/` (G2).

**Agent**:
One supervised identity, with a Status, on the Rail under its Workstream; each launch of its program is an Attempt, and at most one runs at a time.
_Avoid_: session, bot, worker

**Terminal**:
A shell or program roundup started, with a screen the user can watch; no Status beyond running or exited. Never a Rail row and never stored: it ends with the Daemon, with its owner's removal or when the user closes it. An Agent's own Terminal is its pane; any other is shown as a Bubble.
_Avoid_: process, console

**Shell mark**:
An OSC 133 (prompt, command start and end with its exit code) or OSC 7 (directory) sequence a shell the user opened prints, as iTerm2, VS Code and WezTerm read them. The Daemon makes zsh, bash and fish print them by environment only (X15).
_Avoid_: shell integration (as a name; it is the means)

**Turn** (user label: command):
One command of a shell Terminal, from its prompt through its output, between the Shell marks `A` and `D`; its rows, exit code and directory, kept by xterm markers (U514).

**Excerpt**:
The text of a Terminal's selection that "Send to agent" carries to an Agent, cut to 6 000 bytes with its end kept (U515, U516). Sent as a Message `note` fenced with the Turn's facts when known.

**Terminal owner** (`TerminalOwner`, `TerminalOwners`, `with_terminal_owner`, internal `terminal_owner`):
The Agent or Door attached to a Terminal as its Message receiver, with its Attempt, Status revision and receiver ending facts. A Run or the user's shell has no Message receiver, even when an Agent or Workstream owns its Terminal.

**Cached rows** (`Connection::with_cached_rows`):
A scoped fresh SQL execution using a compiled statement in rusqlite's bounded cache. Its exclusive Connection borrow keeps rows inside the callback; reset and binding cleanup precede return. It retains no owner facts.

**Bubble**:
How the App shows a Terminal that is not an Agent's pane: a panel floating over the centre pane that the user moves, resizes, minimises to a chip. It ends with its Terminal.
_Avoid_: overlay, popup

**Pull**:
The user's right-button drag that brings the selected Workstream's Terminal out as a Bubble under the pointer, or opens one when it has none; a right-click that does not move is not a Pull.
_Avoid_: summon, fling

**Run**:
A Terminal an Agent started (`agent.run`), shown as a Bubble while that Agent is selected; it ends with its Agent's removal, never with its Status.

**Show request** (`terminal.show`, event `terminal.shown`):
A Door's ask that the App show a Terminal in its Reach: the Daemon emits `terminal.shown {id}` and the App selects the Terminal's owner. It reads, writes and starts nothing.

**Snapshot**:
A bounded copy of a Terminal's screen and output byte offset that restores its pane after a webview reload.

**Workstream**:
The persistent place for one body of work, named for the work, with exactly one Door and its Agents, Terminals, Todos and Sketches. Stopping its Door neither removes the Workstream nor moves its contents. Workstreams are flat: one never sits under another. Its Agents nest under the Door and under each other (Subagent). Cross-Workstream dependencies are open.
_Avoid_: room, folder

**Clear**:
The user's act of starting a Workstream's Door over on fresh context (`workstream.clear`): it asks why, the Door writes a Handoff, every Subagent stops and leaves the Rail, and the Door starts again from the Handoff (A32). It never removes the Workstream, its other Terminals or any file; a Subagent's Worktree and branch stay.

**Handoff**:
What a Door writes when its Workstream is cleared (`workstream.handoff`): what happened, the user's reason, and the Todos and Pads to keep. The rest is archived in the Pad `archive-<n>`, never deleted. The next Iteration starts from it.

**Iteration**:
One fresh conversation of a Workstream's Door, counted from 1; a Clear starts the next. The Thread spans all Iterations, and each Message carries its own.

**Home**:
Where an item sits: a Workstream, or the Project root. Each item has one Home the user can change. A Todo's Home changes with `todo.move`, a Pad's with `pad.move`. Removing a Workstream deletes its done Todos and its Pads, and moves its open Todos and the Pads of its kept Agents to the Project root (A34). The Shelf shows a selected Workstream's own items until the user asks for all (U347).

**Contract fingerprint**:
A hash of every RPC method's name with its params and result types, computed in `crates/contracts` and reported by the Daemon's `daemon.ping`. The App derives its own at load from the per-method hashes in `contracts/generated/contract.ts` and compares the two at connect (A38, S11).

**Door**:
The coordinating Agent role of one Workstream, with a system prompt tuned to that role. Its Rail id is the Workstream id; its program may be starting, running or stopped without changing the Workstream. Direct access to any Agent remains (P1).
_Avoid_: meta-agent, parent agent, lead

**Subagent**:
An Agent that another Agent, the Door or a Subagent, started. The Rail shows it nested under the Agent that started it, to any depth, within that Agent's Workstream. Claude Code's own in-process subagents are not on the Rail and are not Subagents.
_Avoid_: child agent, sub-agent, meta-agent, parent agent

**Digest**:
A Door's fixed-size envelope about one direct child, `{name, kind, last, todos, pads}`, asked for with `agent.digest`, or pushed to the Door as a Message when the child's Kind (an `idle` and `working` flip alone excepted, B35), Pads or open Todos change (B21), as plain text (B33), and never read from Terminal output (B19, B20). An `error` or `needs-you` child is the exception: its note goes to its parent at once, whoever the parent is (B43). The end note of a child carries its own summary, at most 600 bytes (B49).

**Wake request** (`agent.wake`):
An Agent's saved ask to be told, once, when all or any of named Agents finish (`idle`, `done`, `error` or `needs-you`): decided from `agent.status` Events and the Event log, never a clock, and gone after its one `[wake]` note (`scenarios/messages.md` B41 to B44). A request that fires nothing answers `why`, `already working` (B48).

**Mark**:
The Daemon’s durable record of one Agent at a Checkpoint: the end of its conversation and a snapshot of its tracked and untracked working files git does not ignore. A complete set belongs to one Iteration; invalidated Marks remain stored but unusable (A35).

**Rewind** (`workstream.rewindPlan`, `workstream.rewind`, `workstream.rewound`, `workstream_rewindPlan`, `workstream_rewind`, `HeldCall::Rewind`):
A Door's tool call that takes one child, or every child of its Workstream, back to one of its Checkpoints: each rewound Agent restarts on a copy of its conversation cut at its Mark, and the working files return to the Mark only when the call says `restore`. A child that started after the Checkpoint leaves the Rail. A child the user holds in Takeover waits as a Held action. It is not Claude Code's own `/rewind`, which the user types in a child's Terminal, and it never types one (A35).
_Avoid_: roll back

**Checkpoint**:
A short card of facts a Door writes when its Workstream's work reaches a point worth marking, `{id, workstream, at, title, blocks}` (M10). Not a Message: it has no Route, status or delivery. Written with `checkpoint.write`, listed with `checkpoint.list`.

**Block**:
One part of a Checkpoint that the Door chooses: `rows` (label and value), `table`, `diagram` (Mermaid source) or `text`, each with an optional title (M10).

**Rollup**:
The one Message that stands for the pushed Digests a window's cap held back (B22), or the merge of the oldest two when more than 20 are open. Each pushed Digest has a weight, the number of changes it stands for, so no cap loses a count.

**Attempt**:
One launch of an Agent's program, numbered per Agent from `1` in launch order (A20). It travels in the hook command, `SignalParams`, `StatusEvent` and `RailNode.attempt` as a canonical positive decimal string within SQLite's signed 64-bit range; a client may compare two of one Agent and reads nothing else from it. Never a Rail identity or a vendor conversation id; the last one survives stop, failure and reopen.

**Status revision**:
The strictly increasing ordinal of published Status transitions within one Attempt, `1` at Starting. A canonical positive decimal string on the wire; absent after reopen. Never stands in for an Attempt or Terminal liveness.

**Resume**:
Starting an exited Agent's next Attempt in its saved conversation.

**Worktree**:
The git worktree roundup makes for one Agent when the Project's `worktrees` setting is on: its own directory, branch and working directory (`docs/worktrees.md`).

**Worktree state**:
How far an Agent's Worktree is from its Base: `ahead` and `behind` in commits, and `dirty` when it has changes outside `.roundup/` (`agent.worktreeState`).

**Base**:
The branch checked out in the Project when the Agent spawned; the Agent's branch starts there.

**Resource use**:
The memory, CPU and disk an Agent uses, sampled only when `project.resources` is called (D8): memory and CPU over its program and every program that program started, disk as its Worktree's size. The App shows only the selected running Agent’s values at the right of the shared bottom bar below the Terminal, never on the Rail (U408, U444, U469).

**Landing**:
Rebasing an Agent's branch onto its Base, running the Project's check, then fast-forwarding the Base (`agent.land`, which returns `Landed`: the Base's new commit).

**Discard**:
Deliberately dropping an Agent with its Worktree and branch whatever their state (`agent.discard`); `rail.remove` refuses instead when work would be lost.

### The App

**App**:
The macOS window; it starts a Daemon for each open Project and hosts the webview, a client of that Daemon like any other.

**App seam**:
The few Tauri commands and events between the App and its webview (`scenarios/app.md`); not a Daemon contract.

**File drop**:
The window's native drop of files, which the Tauri shell takes before the webview sees a DOM `drop`; the App seam reports it as `onFileDrop` and a Terminal pane pastes the dropped paths (U162); while one is dragged the Rail's rows and the Project tabs are its drop targets (U348).

**Message image**:
An absolute path to an image file in a Message body, which the Thread shows under the body's text; the App seam gives the webview its URL as `imageUrl` and the shell serves only image files (U352).

**Window bar**:
The App’s shared row for native traffic lights, Project tabs and the centred Project — Workstream title.

**Project tab**:
One entry of the header strip for an open Project: its name (the folder's until the user renames it) and one Tab mark; closing it stops that Project's Daemon (`scenarios/project-tabs.md`).

**Tab mark**:
The small dot just after a Project tab's name: red if an Agent has Kind `error`, else amber if one has `needs-you`, else none (`scenarios/ui-project-tab-box.md`).

**Rail**:
The tree of Workstreams and Agents.

**Hint line**:
The first line of the bottom bar, naming the two to four chords that act where keyboard focus is; each is a `CHORDS` row of `keys/chords.ts` (`scenarios/ui-shortcut-hints.md`).

**Shelf**:
The full-height Todo and Pad panel at the window's trailing edge, with two tabs, `Todos` and `Pads`, that the user opens and closes. It reserves its width for the centre; below the minimum centre width, it overlays a scrim. An open Drawer reserves width instead. With a Workstream selected it shows that Workstream's own Todos and Pads until the user chooses `all`; otherwise it is Project-wide.

**Scrim**:
The darkened surface over the centre while the Shelf overlays it.

**Resize edge**:
The 1 px `--hairline` `role="separator"` line between two regions, the Rail and the centre pane or the Shelf and the centre pane, that the user drags or keys to change a width (U389, U390). Not a button.

**Inbox**:
The time-ordered Drawer of Messages to the user.

**Drawer**:
A panel that slides in from the right over the Shelf (a Todo, a Pad, later the Inbox); the centre narrows to end where it begins (U430), so it covers none of the centre.

**Switcher**:
The field `⌘K` opens over the centre pane, listing every Rail node by name to select one.

**Help**:
The panel `?` opens over the centre pane, listing each chord and what it does.

**Todo**:
An item with an optional blocker list of other Todos, with a Home, in one order, the user's.
_Avoid_: task, ticket

**Sketch**:
A Pad holding Mermaid or drawing blocks; the Pad contracts are unchanged. Workstream-scoped storage is still open.

**Drawing**:
A fenced ```excalidraw block in a Pad whose JSON body is a diagram the user edits in place and an Agent writes as text. It is Pad text, so the Pad contracts are unchanged.

**Pad**:
A markdown note owned by an Agent or the user, with a Home (P12). Removing its Workstream deletes it unless a kept Agent owns it (A34). It has a Revision (P15): a number that starts at 1 and rises with each change to its text, which `pad.appendSection` can require (`expectedRevision`) so two writers never interleave blindly. "Scratchpad" is the product word in prose only.
_Avoid_: scratchpad

**Editor**:
A Pad's single formatted Markdown surface the user reads and types in.

**Attachment**:
An image pasted or dropped into a Message's draft, shown inline as its name and size with a hover preview; not a Chip. (U467 removed the Thread's input, so the App has no such draft today.)

**Attachment file**:
The copy of a pasted image the Daemon keeps under its data folder (`attachment.save`) so a path can name it in a Message; a native-dropped Attachment needs none.
_Avoid_: upload, temp image

**Pasted image**:
An image `⌘V` read from the clipboard into a Terminal: the App seam command `save_pasted_image` saves its bytes as a file in the OS cache folder, and the Terminal pastes that file's path as a File drop does (U418). Not an **Attachment**, which is a Message's draft.

**Paste placeholder**:
The short `[Pasted text #N +M lines]` the Thread's input shows for a long paste; the full text is what the Door receives.

**Quote**:
Selected text attached to the Thread's input as a block with its source.

**setWindowTitle**:
The App seam write (`set_window_title`) of the full native title of the current window (U450); failures are RpcError.

**dragWindow**:
The App seam request (`drag_window`) to start dragging the current native window (U451).

**zoomWindow**:
The App seam request (`zoom_window`) to toggle zoom of the current native window (U451).

**windowOperations**:
The fake App's ordered record of title writes, drag requests and zoom requests (U450/U451).

**windowTitleHandler**:
The fake App's optional title-write response, which can reject or hold writes pending (U450).

**window-bar**:
The native App plugin for current-window title writes, drag and zoom. Its permissions are `window-bar:allow-set-window-title`, `window-bar:allow-drag-window` and `window-bar:allow-zoom-window` (U449–U451); none accepts a target window.

### Status

**Status**:
`{kind, label, since}`.

**Kind**:
One of `error | needs-you | blocked | working | idle | done`, most urgent first; the Adapter decides all but `blocked`, which the Daemon decides from Todos and Routes. Hooks never change it.

**Glyph**:
The one ring-fill mark a row shows for its most urgent Kind: `⊗ ◉ ◌ ◔ ○ ✓`.

**Ink**:
Bold plus colour, used only for `error` (red) and `needs-you` (amber).

**Chip**:
An extra label after a name; it never changes the Glyph.

**Live line**:
The Status label, in a Rail row's `title` and the Agent's pane; the Rail draws none.

### Agents at work

**Adapter**:
The code that turns one agent vendor's process into an Agent (`AgentAdapter`).

**Observation**:
One input an Adapter reads about its Agent: a terminal title, a Signal, an exit, a stop roundup asked for, or a clock tick.

**Signal**:
A structured event the Agent's own tooling pushes (for Claude Code, a hook payload).
_Avoid_: notification

**Steer**:
Sending a prompt to a running Agent through the Adapter (`agent.prompt`), never through the user's keyboard.
_Avoid_: inject, nudge

**First Steer completion** (`FirstSteerCompletion`, `Agents::first_steer_completion`):
The Agent's Attempt-bound observation of its initial Steer, covering H19's gate, queued or running Terminal input and submission. Submitted, refused and uncertain outcomes follow physical completion or an unwritten refusal; none authorizes retry. A lookup error establishes no completion. Cancelling the observation leaves the Steer accounted for.

**Interrupt**:
Ending the Agent's current turn through the Adapter (`agent.interrupt`); the Agent stays alive and becomes `idle`.
_Avoid_: cancel, abort

**Toast**:
A small card with an Agent's Glyph, its name and one line, shown for a moment when that Agent needs the user, fails or changes the user's work; it hides by itself and is never kept in a list.

**Takeover**:
The user typing into an Agent directly; meanwhile no other Actor sends it anything.

**Watch**:
The user reading an Agent's Terminal pane without typing into it; no Takeover, so nothing is held and no work stops.

**Work order**:
The ask an Agent works on, optionally its Done condition, and its limits, rules that each read on their own, such as "Change no file in the repo", held on the Agent's Rail node. Not a Work Issue.

**Done condition**:
The sentence in a Work order that says how the Agent knows it is done (`order.done`).

**Order title**:
The short name of a Work order, at most six words, which the Door writes beside the ask (`order.title`). Absent, the Drawer leads with the ask.

**Spawn prompt**:
The working detail an Agent wrote for the child it started, kept as written beside the child's order and read with `agent.spawnPrompt`. A prompt the user types has none: it is the ask.

**Clarification order**:
The open question an Agent holds instead of a Work order; the Agent finds the intent, elicits what is missing and shapes a Work order.

**Skill**:
A named instruction the vendor program can run when the user types `/<name>`: a folder or file the user, the Project or a plugin holds. `agent.skills` lists them; reading the vendor's folders is the Adapter's (P4). Not an Extension.

**Brief**:
The text roundup gives an Agent at start (`docs/awareness.md`), in two parts. Its file (E1) says roundup supervises the Agent and lists its tools; the file keeps the same bytes for the Agent's session (P9). Its start text (E3, `agent.brief`) gives the Agent's Context in words.

**Context**:
What `agent.context` returns for one Agent: its own Status, its parent, whom to ask (a Door's id, or the user's), its peers and its open Todos. The Brief's start text says the same in words.

**Channel**:
Whether one Agent's `roundup` MCP server (`rup mcp <id>`) has reported to the Daemon: `pending`, `up` or `missing`. Not the vendor's own channels feature.

**Claude inbox**:
Claude Code's own messaging socket for a message from another Claude Code instance, which the adapter sends a `note` over instead of typing it (B50). The Rail fact `inbox` is `ok`, `missing` or `mismatch` (B51). Not the user's Inbox.

**Mcp**:
The Project setting `mcp {on}` (`project.setMcp`, M2219): with `on` false, or the Daemon's `ROUNDUP_MCP=off`, an Agent starts with no `roundup` MCP server and its Brief names `rup` commands. Only Agents started afterward change.

**Stray**:
A process under a Door's Terminal whose program name is one an Adapter supplies and that the Daemon did not register, found by name (F5). `RailNode.stray` lists them and the event `agent.stray` announces a change; it only detects.

**Shim**:
The executable a Door's `PATH` finds first for each vendor program name; it prints that the Door starts an Agent with `agent_spawn` and exits 1 (F4). It stops a lookup by name and nothing else.

**Model**:
The vendor model an Agent runs on, which the user picks (P8). Outside its Adapter, which lists the Models it offers, a Model is an opaque id.

**Effort**:
How much an Agent's Model reasons per turn, picked with the Model (P8); an opaque id outside its Adapter.

**Role**:
The kind of Agent roundup starts, which a Project's Model and Effort are set for: `door` or `subagent`, one list in `crates/agents` that every surface reads. A **scout** is a Subagent started with `scout: true` for reading and searching only (LC4); it takes the Subagent's Model and Effort and is no third entry. Not a Kind (a Status).

**Usage**:
The share of the user's plan limits that an Agent's turns spend (P9, P10).

**Evaluator**:
A small model run inside the Daemon for a routine judgement at no frontier-model cost; a spike (B27), not yet built.

### Messages and Cards

**Thread**:
The conversation with a Workstream's Door about that Workstream's work, not a second container; today it is the Door's Terminal.
_Avoid_: chat, main agent

**Message**:
A typed envelope `{from, to, kind, body, replyTo}` between Actors, with a status of `pending`, `held`, `delivered` or `dropped`. `delivered` means the Agent's Signal confirmed the prompt that carried it, not that it was typed (B39).

**In-flight record** (`message_flight`, `SignalOutcome`, `Agents::observe_prompts`, `Messages::confirm_prompt`):
The saved mark that a pending Message was typed in a prompt and its `UserPromptSubmit` Signal has not confirmed it; a Message is `delivered` only when that Signal arrives, and a prompt that only repeats delivered Messages is dropped by the hook (B39, B40).

**Restart reservation**:
The Daemon coordinator's durable hold on Message delivery to a retained Agent during its conversation restart (A35), separate from a Takeover. Pending and held Messages keep their states and Reasons until release after SessionStart and the reserved first prompt, which the Door's Thread records once without another Steer.

**Retirement reservation**:
The Daemon coordinator's durable protection of a selected later Agent's existing Messages during A35 removal from the Rail. Completion keeps those records unchanged and permanently ineligible for typing, separate from a retained Agent's Restart reservation and from Takeover consent.

**Route**:
A sender→receiver pair with a delivery value: `auto | ask-first | drop`.

**Held**:
A Message waiting on an ask-first Route, the end of a Takeover, the submit or clear of unsent input, or the user's answer to an escalated question.

**Unsent input**:
The bytes the user wrote to an Agent's Terminal since the last submit or clear. While there are any, no Message is typed there (B32).

**Reason**:
The word on a held or dropped Message saying why: `ask-first`, `takeover` or `escalated` or `inbox` (held); `receiver gone`, `not accepted` or `passed` (dropped).

**Hop**:
One Message of a bubbling question: sent to an Agent, then to each Agent above it in its Workstream, nearest first, ending with the Door, each linked to the one before by `passedFrom`. A hop passes when its Agent answers nothing within 60 000 ms or calls `message.pass`; the chain ends at the Escalated question.

**Escalated question**:
The Message to the user that ends a bubbling question: `held` with the reason `escalated`, never `delivered` until the user answers it.

**Card**:
Anything the user sees in the app (a permission request, a question, an Inbox Message, a status, a summary); it has a kind.
_Avoid_: popup

**Markdown**:
The text format an Agent writes; `renderMarkdown` draws it as elements on every surface of generated text (`scenarios/plain-text.md` Q10), and text the user typed is not drawn.

**Decision**:
The kind of Card that blocks an Agent on a live hook or tool call; the user answers any of them, and an Agent answers only those of Agents in its Reach, never its own (`scenarios/decisions.md` H21). An Agent's `ask_user` call is one, answered with one of its listed answers (outcome `answered`).

**Answered Decision** (`AnsweredDecision`, `AnswersParams`, `decision.answers`):
The Project's durable record of one accepted Decision answer: the full original Decision,
complete prompt, request kind (`RequestKind`: question or permission), requesting Attempt, exact answer and note, authenticated answering Actor and answer time.
Its Decision id distinguishes consecutive requests across Daemon runs. The user reads any
Agent's records; an Agent reads only those in its Reach. Open Decisions stay ephemeral.

**Failed answer** (`FailedAnswer`, `Decision.failure`):
The latest refused authenticated Agent answer on one original live Decision,
read through `decision.list` with its Reach filtering: requesting Attempt,
answering Actor, failure time and exact RPC error code and message. It is not a
successful answer record; replacement, clearing or successful delivery discards it.

**Terminal question** (`AskCall`, `TerminalAnswer`, `answer_terminal_question`):
An `ask_user` question the user reads and answers in its Agent's primary Terminal.
The answer belongs to the original Decision and Attempt and retains its authenticated
answering Actor. `AskCall.terminal` correlates that waiting call with its form and
grants no answering authority.

**Prompt identity** (`PromptIdentity`, `CaptureParams`, `decision.capture`):
The original Decision's identity within one Daemon run, Agent and Attempt. A held answer or answer-like Terminal write carries it into the effect; an answered, dismissed, replaced or unavailable original refuses with an error the Daemon can record as dropped. Equal text and a reused Decision id never revive it. It grants no Reach or Proof.

**Prompt effect**:
An answer or Terminal write applied to its captured Prompt identity. The check and effect exclude a concurrent answer, replacement or Attempt change; refusal writes no bytes, and a successful effect consumes the identity. It does not store a held action or acquire a Takeover.

**Actor**:
The user, an Agent, an Extension or the Daemon (as `rupd`) making a call.

**Held action** (`HeldAction`, `HeldCall`, `HeldCall::Stop`, `HeldCall::Interrupt`, `HeldCall::Prompt`, `HeldCall::SetOrder`, `HeldCall::Resume`, `HeldCall::Answer`, `HeldCall::Close`, `HeldCall::Kill`, `HeldCall::Write`, `Agents::prompt_attempt`, `Agents::stop_attempt`, `Agents::interrupt_attempt`, `agent.actionHistory`, `agent_actionHistory`):
A durably accepted Door action waiting for a child's Takeover to end, with its
original Actor, target, method and parameters retained. Its outcome records
waiting, in-progress, completion or a dropped reason; acceptance is not execution.

**Resize action** (`ResizeAction`, `ResizeOutcome`, `terminal.resizeHistory`, `terminal_resizeHistory`):
An Agent's accepted Terminal resize and its retained outcome: waiting for the child's Takeover, in-progress, completed or dropped with a reason.

**Output stream** (`Output`, `OutputStream`, `Terminals::subscribe_output`, `terminal.output`, `terminal.outputStop`, `OutputStarted`, `OutputEvent`, `OutputGap`, `terminal.gap`):
One Terminal's bytes with their offsets, delivered only to a connection that selected that Terminal (`scenarios/terminal-stream.md` X14). `terminal.output` selects it and answers the offset of the first byte; the Daemon then pushes `terminal.output` notifications with the bytes and, when the connection fell behind, a `terminal.gap` naming the offsets it dropped, so the App recovers with `terminal.snapshot`. `terminal.outputStop` or closing the connection releases it. Output never enters the Bus.

**RPC output intake** (`Client::take_output`, `OutputFrame`):
The Client's optional bounded receiver of the Output streams its connection selected, taken before selecting. It reports lag separately from replies and control events; retained frames keep their Terminal ids and byte offsets for Snapshot recovery.

**Event log**:
The Daemon's table in `project.db` of every Event but Terminal output, kept to the last 10 000 (`EVENT_WINDOW`); `events.subscribe` replays from it (`scenarios/daemon.md` D10, D11).

**Event number** (`seq`):
The positive integer the Event log gives an Event as its row id: it rises by one per saved Event, never repeats across a Daemon restart, and rides on the Event every subscriber sees. Terminal output has none (D10).

**Event head** (`head`):
The highest Event number saved when an `events.subscribe` was accepted, 0 for an empty log: its answer is `{head}`, and `rup events` without `--follow` stops once it has printed that Event (`scenarios/cli.md` I9).

**Output log**:
The last 1 MiB of a Terminal's output bytes, kept so an Agent can read what is new from an offset or search it (`scenarios/terminal.md` X13); it ends with its Terminal.

**Read cursor** (`reader`):
The offset in a Terminal's Output log where one reader's next `terminal.read` without `after` begins, set to the reply's `next`; each (Agent id, Terminal) pair has its own, it is Daemon state only, and it ends with its Terminal (`scenarios/terminal-stream.md` X16).

**Reach** (`Agents::guard_reach`):
The Rail nodes an Agent may act on through its MCP server: its descendants, and the Workstreams it created with theirs (`scenarios/mcp.md` M7); a call outside it is `FORBIDDEN`. It also bounds the Decisions an Agent lists and answers (H21).

**project.requested**:
The Event an Agent's `project_open` or `project_rename` emits, because a Daemon cannot reach the App; the webview answers it with `open_project` or `rename_project` (`scenarios/mcp.md` M8).

**Touch**:
One logged read or write of a Todo, a Pad, a Message or a Decision (`decision:<id>`) by an Actor.

**Provenance**:
The append-only log of Touches.

### Daemon and Extensions

**Daemon**:
`rupd`, the local process everything else is a client of.

**Proof**:
The random secret the App gives its attached Daemon on stdin and the webview reads through `daemon_proof`; the user's `decision.answer` needs it; an Agent's is checked by Reach instead (H4, H18, H21).

**openTarget**:
The App seam command (`open_target`) that opens a web link in the browser, or an absolute path in its default app (a folder in Finder), when the user Cmd+clicks one printed in a Terminal; a missing path is `NOT_FOUND`, a scheme but http and https is `FORBIDDEN` (U365).

**open_in_editor**:
The App seam command that opens an absolute directory in the user's code editor: `ROUNDUP_EDITOR`, else VS Code, else Cursor; with none installed it fails `NOT_FOUND` with a message saying so (U429).

**Extension**:
A directory with a manifest and one module.

**Hook**:
One `($, e, next)` function an Extension registers.

### Design system

**Token**:
One named look value in `apps/desktop/src/tokens.css` (a colour, type step, space step, radius, shadow or duration); no other file holds such a literal.

**Section head**:
The icon, capital label and trailing rule that name a section of the Rail or Shelf.

**Signifier**:
What shows that something can be clicked: its hover surface, `pointer` cursor and focus ring.

**Button kind**:
One of five looks every button of the App takes, named by its `data-button`: `primary`, `quiet`, `add`, `row-action`, `icon`; each is defined once, from Tokens, in `apps/desktop/src/buttons.css` (`docs/design-system.md`, "Click targets"). A row is not a button.

**Check**:
One pass-or-fail rule of `docs/design-system.md` (`D1` to `D10`).

**Held-out screen**:
A screen the Design critic runs that no Builder's prompt lists.

### The loop

**Work Issue**:
An open GitHub Issue that the user or a loop App filed, holding a scoped outcome, acceptance references, immutable Key, priority and native dependencies, or a request without them that a build run specifies first. Who filed it authorizes execution; `flag:needs-user` or `flag:needs-mac` holds it; its delivery evidence determines completion (L34).

**Builder**:
The one kind of agent run that builds roundup and reviews it, each run fresh, from `.agents/builder.md`: a build run takes a GitHub work Issue, a fix run answers a reject or a failure, and a review run judges a PR's head, never its author's rationale (L23, L24).

**Design critic**:
The run that judges a UI PR's head against the user's bar, "elegant, well designed and tasteful" (#400), and `docs/design-system.md`; its reject is the `critic` status and blocks merge (L103).

**Screen**:
The Haiku run in a build or fix job that reads what the run asks the user, a build run's non-empty `ask` or a draft's `Stopped:` paragraph, against `.agents/screen.md` and sends it to the user (`flag:needs-user`), to an agent on the user's Mac (`flag:needs-mac`) or back to the loop: an `ask` as a strike, a draft as an `unfinished` fix run (L79, L81).

**Code PR**:
A PR touching `apps/`, `crates/`, `contracts/` or a Build file; it needs a `Scenarios:` line (L46).

**Ready PR**:
A Builder PR out of draft with auto-merge armed: its run's `gh pr ready`, then `gh pr merge --auto --merge` (`.agents/builder.md` step 3). An Issue's ready row (L34) is another thing.

**Build file**:
What `just check` runs and builds with: `justfile`, `Cargo.toml`, `Cargo.lock`, `package.json`, `pnpm-lock.yaml`, `pnpm-workspace.yaml`, `rust-toolchain.toml`, `tsconfig.json`, `.oxlintrc.json`, `.fallowrc.json`, `.cargo/` and `tools/` (L46).

**Loop machinery**:
`loop/`, `.github/`, `.agents/`, `.claude/`, and every `AGENTS.md` or `CLAUDE.md`; review runs read `main`'s copy, never the PR's, and a review run rejects a change that lets a PR pass its own gate (L24, L46).

**Capture**:
A PNG or checks JSON under the artifact-relative `artifacts/ux/<id>/` (journey: `artifacts/ux/journey/<id>/`) that a test writes only when `ROUNDUP_CAPTURES` names its id; `just capture <ids>` selects only the PR's `Scenarios:` ids and `just check` writes none. `rules` rejects tracked evidence additions or edits and permits deletion-only retirement (L112).

### Todo history

**History** (`History`): the ordered durable Changes of one Todo, fetched only by its open Drawer. Reading History does not log a Touch.

**Get History** (`todo.getHistory`): fetch one Todo's History and its previous read boundary without logging a read Touch.

**Values** (`Values`): a Todo's title, body, done state and blocker identities, as held by a Change.

**Change** (`Change`): initial Values or the before and after Values of one successful Todo mutation. `seq` orders Changes; lists and the Rail do not carry them.

**Read History** (`todo.readHistory`, `ReadHistoryParams`): log the calling Actor's read Touch with `through`, the last fetched Change's sequence. `unread` names the first Change after the previous read. Equal clock times do not merge reads and Changes.

**Tick box** (`TickBox`): the shared native Todo completion control on its row and before its Drawer title; a done Todo is checked and cannot be completed again.

### App component layer

**Dialog**:
A Token-styled question or panel in the App that holds focus until dismissed.

**AlertDialog**:
A Dialog asking permission before a destructive action; dismissal cancels the action.

**Menu**:
A Token-styled list of App actions opened from a control.

**Tooltip**:
A Token-styled explanation shown on hover or focus of an App control.

**Tabs**:
A Token-styled set of named App views, with one selected view and its panel visible at a time.

**IconButton**:
An App button of the icon kind, named for its action for assistive technology.

**TreeView**:
A shared navigable hierarchy preserving caller-rendered rows.

**Sortable**:
A shared reorderable region with named drag handles and caller-owned committed order. With its `tree` contract it reports a drop as a **Sortable drop**.

**Sortable drop**:
Where a dropped Sortable row lands: `{id, parent, index}`, with `index` counting the parent's other children; the Rail's `rail.move` takes the same three (U535).

### Keyboard registration

**KeyScope**:
The owner of a shortcut: the App, a region containing keyboard focus, or an active layer.

**registerKeys**:
Register a KeyScope's chords, guards and actions for its owner's lifetime.

**chordsFor**:
The matching spellings of one displayed Help chord.

**Leader**:
`⌘K`: it waits for one more key from the `LEADER` table (`LeaderKey`, `leaderChord`), or ends on any other key (U2239). The `leader` KeyScope holds those keys' actions; `armLeader`, `endLeader`, `leaderArmed` and `modalOpen` are its state.

### Ask placement

**Ask** (`ask::Ask`, `Asks`, `SendParams`, `PickParams`):
One message the user sends from the Ask bar; distinct from an Agent's Decision. `Asks` stores and delivers it; `SendParams` sends it and `PickParams` chooses one part's destination. `Steer::User` carries its exact words through `Agents::prompt`; ordinary `Steer::Instruction` keeps H20's lead.

**Ask part** (`AskPart`, `ask::State`):
A verbatim, non-overlapping span of an Ask, with its Placement.

**Placement** (`Placement`, `Destination`, `Target`):
An Ask part's proposed destination: routed to a Workstream, new with an Order title, or unclear with a question and up to three candidate Workstreams.

**Placement request** (`placement::Request`, `EarlierAsk`):
An Ask's text and its context: the five newest earlier Asks' parts and destinations, and every Workstream's name, Order and Status Kind.

**Placement call** (`PlacementCall`, `Agents::placement_call`):
The Adapter's one-shot judgement that splits an Ask into checked parts and proposes their Placements. It is not an Agent and has no Terminal or Rail node.
