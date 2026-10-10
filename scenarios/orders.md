# Orders

Module: `crates/agents` (the order on each Agent, set at spawn and at a Door's start, persisted with the Rail), `crates/contracts` (`Order`, `RailNode.order` (named `work` in the wire type if `order` collides with the sibling position field), `SpawnParams.order`, `CreateWorkstreamParams.order`, `ContextSelf.order`, `agent.setOrder`), `crates/rupd` (registration), `crates/rup` (the `agent_set_order` MCP tool), `apps/desktop` (O6 only). Terms: Work order, Clarification order, Workstream, Door, Steer, Decision, Provenance (`GLOSSARY.md`). Principles served: P1, P3, P6. Contract change (rule 4): the fields and method above, with every caller in the same PR. Reported in #389. Workstream becomes Workstream with #369, and these clauses rename with it. Ids: O1–O6.

**O1 every Agent holds one order.** Given any node in `rail.tree`, then each Agent node and each Workstream node (its Door) has an order that is exactly one of `{kind: "work", ask, limits}` or `{kind: "clarification", question}`. `ask` and `question` are non-empty after trimming. `limits` is a list, possibly empty, of non-empty strings saying what the Agent must not do. A Terminal node has no order (`null`). Test: `o1_`, over a real Daemon after each path of O2 and O3.

**O2 a spawn sets the order.** Given `agent.spawn {cwd, prompt?, parent?, order?}`:
- With `order`, that order is stored.
- With `prompt` and no `order`, the order is `{kind: "work", ask: prompt, limits: []}`.
- With neither, it is `{kind: "clarification", question: "What should this Agent do?"}`.
- Both `prompt` and `order`, or an order that breaks O1's shape, is `INVALID_PARAMS` and changes nothing (A18).

Once H19's gate opens (the first `SessionStart` and the first star title), the Agent gets one Steer in place of A4's prompt Steer:
- For a Work order, the ask, then `Must not:` and one line per limit (the section is left out when `limits` is empty).
- For a Clarification order, the question and an instruction to find the intent, elicit what is missing (from whom `agent.context` says to ask) and record a Work order with `agent_set_order`.

A Door's `agent_spawn` (F3) takes the same `order`; B24's policy (fixed limits, not decisions) is what its Brief tells the Door to put there. Narrowing: A9's no-prompt spawn now also holds a Clarification order; its naming rule is unchanged. Test: `o2_`, with a fake `claude` recording the first Steer.

**O3 a Door's order is its Workstream's.** Given `rail.createWorkstream {name, parent, order?}`, then the Workstream's Door holds `order` when given, else `{kind: "clarification", question: "What is this Workstream for?"}`. `rail.startDoor` (A7) sends the Door O2's Steer for that order. Stopping the Door keeps the order with the Workstream. Test: `o3_`.

**O4 settling an order.** Given `agent.setOrder {id, order}`, then the Agent itself, the Door of its Workstream, or the user may replace its order with any order of O1's shape. The change emits `rail.changed` and records a Touch with the Actor (Provenance).

If a caller other than the Agent changes the order, the new order is sent to the Agent as O2's Steer, except during a Takeover (P1), when it is not sent. In both cases `agent.context` returns it as `this.order`.

Errors:
- Another Agent: `FORBIDDEN`.
- A Terminal: `CONFLICT`.
- An unknown id: `NOT_FOUND`.
- A malformed order: `INVALID_PARAMS`, with nothing changed.

A Clarification order whose question only the user can answer reaches the user only as a Decision (P3, `agent.ask`); holding the order alone never yields `needs-you`. `rup mcp <id>` offers `agent_set_order`, with `id` filled with its own Agent's id. Test: `o4_`.

**O5 an order survives.** Given an Agent with an order, when the Daemon reopens (A8, A12), the Agent is Resumed (A22), or it is moved to another Home (`rail.move`), then `rail.tree` shows the same order. A Resume sends no new order Steer. Discard and `rail.remove` drop it with the node. Test: `o5_`.

**O6 the user reads the order in the Workstream.** Given a selected Agent or Workstream in the App, then one line above its Terminal shows the order. For a Work order, the line is the ask, then `must not:` and the limits count. For a Clarification order, it is `clarifying:` and the question. The line is cut to one line, and activating it opens the full order in the Drawer under U111's layout rules.

The line is present for every selected Agent, so moving between Agents never changes the Terminal's size (U5). It sits with U113's Card, which stays above it. A Terminal shows no line. Text meets U4, and the line uses only Tokens. Observers: `o6_` in Vitest and the U26 harness, and a native 1280×800 capture of a Door, a Work-order Agent and a Clarification-order Agent in one Workstream. The PR names the Checks it moves.
