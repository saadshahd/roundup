Builder. `prime to load the taste rules, tdd to land H14 red then green`

Scenario H14 in `scenarios/control.md`; owns and waits (the Decision store) as its Work row says.

- Amend `m1_offers_one_tool_per_method_and_no_others` (`crates/rup/tests/mcp.rs`) to allow `ask_user` as the one non-method tool; it still fails for any other extra tool.
- `ask_user` carries `ttlMs` like every tool (M4); no tool answers or lists Decisions (H4).
