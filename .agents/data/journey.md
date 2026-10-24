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

- D1 (#3789, row interaction): a primary click selects a Todo, Pad or Rail row; Return or its Open button opens; double-click/F2 renames. A drag or rename gesture never opens. Apply U3789a to Shelf rows and owned-item leaves alike.
- D2 (#3789, Todo add): one key or click focuses one line; text plus Return files into the selected scope. Opening/typing never expands an owner group; only the field, accepted new row and newly nonempty destination group mount, with existing counts/empty feedback updated (U3789b).
- D3 (#3789, empty groups): an item group with zero items renders nothing; one scoped-list empty line is the only empty state, replaced by add while editing. Empty Rail destinations remain (U3789c).
- D5 (#3789, row rest): name plus at most one state glyph, failed > blocked > running > done; preserve native attention's non-colour distinction. Secondary facts/actions show on focus or selection, never hover; opacity in reserved space moves no row, with no display swap or !important in row action styles. Actionable failure feedback stays visible (U3789d).
- F3 (#3789, row budget): a focused/selected row exposes at most two controls including Open, checkbox and disclosure; at rest at most four state marks, with the one-glyph row rule stricter. Keep other actions in the native menu or opened destination (U3789d). These Issue labels do not rename the numbered design Checks.
