# First complete journey

User-selected goal: first Project → start Agents → understand Status → intervene → review results. The product destination is `PRINCIPLES.md` P1/P3/P6 and the accepted Room/Door slice in `docs/wireframes.md`. Complete that journey before expanding into Sketch storage, Room nesting or cross-Room dependencies.

Priority: H18 U113 J1 U143 E1 B24 H2 H3 H5 H6 H7 H10 H8 H9 H14 U132 U133 U136

`loop/runs.sh queue` ranks eligible Work rows by the first matching id above, then keeps the other rows in their existing order. Priority never overrides an `After` dependency, an open PR, a Claim or the Builder limit. An id with no eligible row consumes no slot. U143 is the first-project UI slice being developed; the other ids reuse the existing Brief, Door, Decision and visual work rather than duplicating it.

## What must work

| Stage | Evidence required |
|---|---|
| Enter | From `first-run`, choose a Project, create a Room, start its Door and reach the Thread using keyboard and pointer. Empty, starting, stopped and failure states explain the next available action. A failed call preserves the user's input. |
| Request | Ask the Door for one concrete action. Observe that action through the real Daemon and the tools it actually offers, including the resulting Todo and outcome; a seeded success message is not evidence. Direct Agent access remains available. |
| Understand | With ten Agents, distinguish working, blocked, needs-you, done and error without relying on colour; identify the selected Room and the Agent requiring attention. Routine progress does not interrupt the user. |
| Intervene | Answer one real Decision or enter an Agent directly, then observe it resume. An error remains actionable; stopping a Door preserves its Room and contents. |
| Review | Read the action's outcome and inspect its resulting work from the Room. Reopen the Project and find that work again. Do not claim Room-scoped storage before its contract and behavior exist. |

## How a Builder closes a gap

For a UI row or a row affecting the stages above, read the relevant stage and the existing scenarios before changing code. Use `emil-design-eng`; use `animate` for a motion clause. Inspect the rendered App with `agent-browser` at 1280×800 and 700×800, using the real harness and its failure injection. Capture the before/after interaction and its D1–D10 readings; name any pre-existing failing Check and introduce no regression. A mock proves the webview only; the Daemon observer above proves the actual action.

If the stage reveals missing behavior outside the owned row, first find the existing scenario/Work row. If none covers it, add one bounded given/when/then scenario with a named observer and a Work row with real `After` ids. Assign the next unused id in the relevant scenario file; the queue can take that row without changing this work order. Never replace the stage's acceptance condition with what the current code happens to do, and never create a test that merely checks a report contains the right words. Missing capability is unfinished work, not an approved visual difference.

The review run checks the stage from the scenario, checkout and evidence; it does not accept the author's assertion that the journey is complete. The goal is complete only after all five stages have behavioral and rendered evidence on the same main revision. Test-name presence and merged-PR counts are queue bookkeeping, not that evidence. A native rendering claim still needs the user's macOS observer.
