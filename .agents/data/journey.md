# First complete journey

User-selected goal: first Project → start Agents → understand Status → intervene → review results. The product destination is `PRINCIPLES.md` P1/P3/P6 and the accepted Workstream/Door slice in `docs/wireframes.md`. Complete that journey before expanding into Sketch storage, Workstream nesting or cross-Workstream dependencies.

Priority and dependencies live in the GitHub work Issues (`.agents/data/work.md`). An open PR keeps its Issue regardless of priority.

## What must work

| Stage | Evidence required |
|---|---|
| Enter | From `first-run`, choose a Project, create a Workstream, start its Door and reach the Thread using keyboard and pointer. Empty, starting, stopped and failure states explain the next available action. A failed call preserves the user's input. |
| Request | Ask the Door for one concrete action. Observe that action through the real Daemon and the tools it actually offers, including the resulting Todo and outcome; a seeded success message is not evidence. Direct Agent access remains available. |
| Understand | With ten Agents, distinguish working, blocked, needs-you, done and error without relying on colour; identify the selected Workstream and the Agent requiring attention. Routine progress does not interrupt the user. |
| Intervene | Select the requesting Agent's Terminal, answer there with the vendor's own control (A3564a), or enter an Agent directly, then observe it resume. An error remains actionable; stopping a Door preserves its Workstream and contents. |
| Review | Read the action's outcome and inspect its resulting work from the Workstream. Reopen the Project and find that work again. Do not claim Workstream-scoped storage before its contract and behavior exist. |

The latest audit evidence is [the J14 result summary](../../docs/evidence.md#j14-first-journey-audit) (J14, main `b5b51f34`). Four isolated headless passes drive all five stages on that revision: keyboard and pointer first-run choice at both 1280×800 and 1440×800, a real Door tool action and resulting Todo, a rejected send retaining its exact draft and one delivered retry, all six Kinds among ten Agents in two Workstreams, a real native question answered in the Agent's Terminal with the Agent resuming, stopped-Door preservation, Todo inspection and reopen. All 68 screens pass D1–D14 and D16, including D3/D10 on rejected calls. Main's behavioral observers also cover starting/cancellation, routine-progress silence and jump wrap. No uncovered journey gap or successor audit remains within this observer: the Daemon is real, its `claude` is fake and its chooser returns the configured isolated Project. No native chooser, native App, real `claude` or settled-work summary is claimed; the user reports what they meet on the Mac.

## How a Builder closes a gap

For a UI Issue or an Issue affecting the stages above, read the relevant stage and the existing scenarios before changing code. Use `emil-design-eng`; use `animate` for a motion clause. Inspect the rendered App with `agent-browser` at 1512×950 (`set viewport 1512 950`), using the real harness and its failure injection. Capture the before/after interaction and its D1–D10 readings; name any pre-existing failing Check and introduce no regression. A mock proves the webview only; the Daemon observer above proves the actual action.

If the stage reveals missing behavior outside the owned Issue, first find the existing scenario and work Issue. If none covers it, add one bounded given/when/then scenario with a named observer and a work Issue with native blocking dependencies. Assign the next unused id in the relevant scenario file; the queue can take that Issue without changing this work order. Never replace the stage's acceptance condition with what the current code happens to do, and never create a test that merely checks a report contains the right words. Missing capability is unfinished work, not an approved visual difference.

The review run checks the stage from the scenario, checkout and evidence; it does not accept the author's assertion that the journey is complete. The goal is complete only after all five stages have behavioral and rendered evidence on the same main revision. Test-name presence and merged-PR counts are queue bookkeeping, not that evidence. A native rendering claim still needs the user's macOS observer.

## The viewport

One viewport: 1512×950, the fullscreen 14-inch MacBook Pro. Everything fits it with no clipping or overflow. No other width is checked.

## The owner's visual taste

The owner wants a native macOS App, not a web page. A UI PR that breaks one of these is rejected by the critic and the review run, whatever else it proves:

- No outlines or boxes around controls or tabs.
- No hover background boxes; hover only changes text or icon colour (#2173). The owner: "i don't like these bg hovers boxes it - its bad - annoying in spaces - radiuses - and fully annoying not native feels weby".
- Radius only on floating panels: the bubble, menus, popovers (#2176).
- Tabs: one flat Zed-style bar for Project and Terminal tabs (#2210). The owner: "the borders and design of the tabs is too annoying not elegant and not native - too bad".
- Floating panels are opaque with a shadow; no opacity slider (#2176). The owner: "we should remove the opacity feature remove it and add proper box shadow and padding and radius to the bubble".
- One Terminal bubble shows at a time, centred over the centre pane; roundup builds no splitting, tiling or bubble manager, since the Terminal itself (tmux or similar) handles splits (#2211). The owner: "we only need a resizer - terminals them selves could manage splitting and tiling not us".
- The Terminal bubble is a transient panel: open, it is the topmost layer and owns focus, and a press outside it minimises it (#2307). Its bar is a 28 px macOS panel bar: close and minimise dots at the left, the title centred, a status dot after it, ink only. The owner: "the open bubble should have the highest zIndex".
- State is not an action (#2469). State tells what a thing is doing; actions are what you can do to it. State rides with the name (gap at most `--space-2`, no action between them). Actions group at the trailing edge, right of every state, at least twice the gap between two actions and never under `--space-3` from any state. The exceptions are the leading disclosure chevron and a native tick box whose value is the state.
- State never looks or behaves like a control: no button role, no tab stop, no pointer cursor, no hover wash, a box size unlike the row's icon buttons. Hover and focus never move, hide or swap it (a Terminal tab's status dot never turns into its ×). Every non-decorative KindGlyph and state count carries a state mark (`data-state`). This supersedes the bubble bar's status dot: its status is a Kind glyph beside the title. The owner: "the status indicator is too close to the actions - it's a state not an action".
- Every string answers the user (#3021). A rendered text node is one of: text the user typed; a name the user gave, or one that names the item elsewhere in the App; a time; a `GLOSSARY.md` user-label state with its KindGlyph; an error that names its cause and its fix; a control's label; a section head. No other text renders: no id, revision or storage key, no path of the open Project, no fallback label for an unmapped field (`Stored value`), no statement that roundup did not check something (`Original availability not verified`). The critic names each text node's kind on the seed; a node of no kind is a block.
- Every control changes something. Pressing a control in the seed changes what is on screen, calls an RPC or writes the clipboard. A control that would change nothing in that state does not render (with one revision, no `Inspect`). An item offers at most one `Copy`, which copies all its text.
- Typing moves nothing else. While the user types into a field whose saves succeed, no element outside that field mounts, unmounts or changes its text. Observer: type 20 characters with acknowledged saves; a MutationObserver on the App root, outside the field, records nothing.
- History is opt-in. Past changes, revisions and logs open collapsed, as one line with their count and a disclosure; entries render only after the user opens it. Each entry reads `<past-tense verb> <value> · <time>`, plus the actor when it is not the user (`Renamed to jbkhnj`, `Marked done`), never `Field: value`.
- A state is never a bare control word. No text node outside a control equals a control label from the App copy catalogue (`Done`, `Open`, `Copy`, `Discard`); section heads are exempt.
- Choosing from other items is a menu. Picking one item from a list of others (a blocker, an Agent, a Model) opens the shared `apps/desktop/src/ui/Combobox.tsx`: floating, anchored to its trigger, filtered by typing, closed by Escape with focus back on the trigger. It never expands the list inline in the page.
- Actions sit on their row. A row's text actions sit on the baseline of the row's first line, within 1 px.
- A modal's centre sits at the window's centre within 1 px on both axes; only `ui/Dialog.tsx` positions a modal (#3083).


## Calm screens (#3786)

At rest means actions and motion have settled, the pointer is outside rows, and menus/pickers/tooltips are closed; selection and requested panels may remain open. These owner taste lines are acceptance for U2539b in `scenarios/ui-worktree-changes.md`; this specification does not claim they are implemented. Owner taste D6 supplements the existing design-system D6 target/focus Check; it does not replace it.

- **D6:** a resting header/footer item mounts only for existing changes, current native attention/error, or recovery from a current failure. Zero changed files mounts neither changes opener nor view; nonzero mounts one counted icon, opening the view on demand. Requested navigation/inspection is available without keeping its contents mounted at rest; retain the Terminal portal host and lifetime.
- **F1:** each of the ten tasks below completes in at most three presses and has its working chord shown beside its action in the native menu bar. Count through the endpoint, including confirmation and selection.
- **F2:** each Agent's current Kind appears in at most three resting places in the active window, including separate aggregate summaries containing that Agent. A row's glyph/label/count together is one place. Different Agents remain separate facts; hidden DOM, closed menus and vendor output are excluded.
- **F4:** with an open Project, nothing selected and no requested surface, only Ask and Needs you mount as product content, including an empty Project. Current failure/recovery, window controls and the native menu bar remain available. Everything else opens on demand; direct Agent access and native attention continuation remain intact.

### Top ten tasks

Press counting: one chord, unmodified key or pointer activation is one press; `⌘K` followed by a letter is two. Text content and an exact unique target-name query are payload entry, recorded separately from navigation; every arrow, Tab, submit/confirm and menu activation counts. Do not pre-open a surface unless the Start column says it is open. Count until the acknowledged RPC or focus/selection/view endpoint, not merely until a picker opens. Each picker row below requires filtering by the exact unique authored name and Enter, no intervening navigation. More than nine possible targets must work; arbitrary targeting is not restricted to digit shortcuts. Repeat with Terminal focus where the action is App-scoped; respect modal/editor ownership.

| Task | Start → endpoint | Main baseline | Required path | Presses |
|---|---|---|---|---|
| Ask for work | open Project, draft text supplied → one acknowledged `ask.send` | `⌘/`, Enter: 2 | `⌘/`, Enter | 2 |
| Start an Agent | open Project, no modal → returned Agent selected, Terminal reachable | `⌘N`: 1 | `⌘N` | 1 |
| Open a Terminal | Workstream selected → new shell Bubble focused | `⌘T`: 1 | `⌘T` | 1 |
| Capture a Todo | open Project, title supplied → one acknowledged `todo.create` | configured quick-add, Enter: 2 | configured quick-add (initially `⇧⌘T`), Enter | 2 |
| Reach what needs you | Project with native attention → requesting Agent Terminal focused | `⌘K`, J: 2 | `⌘K`, J | 2 |
| Open work items | work items closed → current scope's list reachable | `⌘K`, S: 2 | `⌘K`, S | 2 |
| Open Inbox | Inbox closed → its attention list reachable | `⌘K`, I: 2 | `⌘K`, I | 2 |
| Hand a Todo to an Agent | Todo selected/focused → acknowledged `todo.assign` for chosen Agent | O, target click: 2; landing path below: 5 | `⌘K`, A, Enter after target query | 3 |
| Switch Project | eight open Projects → chosen Project active, its prior selection retained | tab click: 1; no Project-switch chord | `⌘K`, P, Enter after target query | 3 |
| Review changes | Agent selected, nonempty changed-file set → changed-file tree open | branch-control click: 1; no review chord | `⌘K`, C | 2 |

Baseline: main `a284e11af`, isolated headless U26 App at 1512×950 on 2026-10-24, `tree-40`, `resources` and `tabs-8`, using the fake Daemon seam. Calls, selected rows, focused input and mounted roles establish these paths; they do not establish native menu behavior or real Daemon execution. The five-press landing assignment path is Unassigned disclosure, Todo click (opens Drawer), close Drawer (focus returns to Todo), O, target click. From an already focused Todo it is two, so “5+” is not a universal count. Text entry was separate. The `resources` seed needs a supplied `agent.changedFiles` fixture; a successful empty fixture leaves the branch opener and `No changes` panel on main. No such empty view is allowed by this specification.

For `migrate-db` at the root on `tree-40`, the same needs-you fact appears in five presentations: its Rail row, backend's rollup, the Project-root count, the overview heading and its overview attention entry. The Project tab shows Error on this seed and is not a sixth needs-you presentation. At a selected Agent the root overview is absent. A settled open Inbox also counts toward the limit, including its section count and Agent entry; opening it must suppress redundant background presentations if necessary. The implementing observer must enumerate presentations by Agent and include aggregates, not count literal text occurrences or all Agents as one fact.

Understand/Intervene/Review remain the journey: submit an Ask, reach its requesting Terminal, inspect the resulting changes, close the optional surfaces, reopen the Project and recover the same work. Use U2539b's real-Daemon and rendered observers before claiming completion. Keep drag (#3540) and calm Todo/Pad/Rail row behavior in their existing work orders.
