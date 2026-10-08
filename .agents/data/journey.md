# First complete journey

User-selected goal: first Project → start Agents → understand Status → intervene → review results. The product destination is `PRINCIPLES.md` P1/P3/P6 and the accepted Room/Door slice in `docs/wireframes.md`. Complete that journey before expanding into Sketch storage, Room nesting or cross-Room dependencies.

Priority and dependencies live in the GitHub work Issues (`.agents/data/work.md`). A Claim or active PR keeps its ownership regardless of priority.

## What must work

| Stage | Evidence required |
|---|---|
| Enter | From `first-run`, choose a Project, create a Room, start its Door and reach the Thread using keyboard and pointer. Empty, starting, stopped and failure states explain the next available action. A failed call preserves the user's input. |
| Request | Ask the Door for one concrete action. Observe that action through the real Daemon and the tools it actually offers, including the resulting Todo and outcome; a seeded success message is not evidence. Direct Agent access remains available. |
| Understand | With ten Agents, distinguish working, blocked, needs-you, done and error without relying on colour; identify the selected Room and the Agent requiring attention. Routine progress does not interrupt the user. |
| Intervene | Answer one real Decision or enter an Agent directly, then observe it resume. An error remains actionable; stopping a Door preserves its Room and contents. |
| Review | Read the action's outcome and inspect its resulting work from the Room. Reopen the Project and find that work again. Do not claim Room-scoped storage before its contract and behavior exist. |

## How a Builder closes a gap

For a UI Issue or an Issue affecting the stages above, read the relevant stage and the existing scenarios before changing code. Use `emil-design-eng`; use `animate` for a motion clause. Inspect the rendered App with `agent-browser` at 1280×800 and 700×800, using the real harness and its failure injection. Capture the before/after interaction and its D1–D10 readings; name any pre-existing failing Check and introduce no regression. A mock proves the webview only; the Daemon observer above proves the actual action.

If the stage reveals missing behavior outside the owned Issue, first find the existing scenario and work Issue. If none covers it, add one bounded given/when/then scenario with a named observer and a work Issue with native blocking dependencies. Assign the next unused id in the relevant scenario file; the queue can take that Issue without changing this work order. Never replace the stage's acceptance condition with what the current code happens to do, and never create a test that merely checks a report contains the right words. Missing capability is unfinished work, not an approved visual difference.

The review run checks the stage from the scenario, checkout and evidence; it does not accept the author's assertion that the journey is complete. The goal is complete only after all five stages have behavioral and rendered evidence on the same main revision. Test-name presence and merged-PR counts are queue bookkeeping, not that evidence. A native rendering claim still needs the user's macOS observer.
