Builder. `prime to load the taste rules, tdd to land H3, H5, H6, H7 and H10 red then green`

Scenarios H2–H10 in `scenarios/decisions.md` (H2 has tests); owns as its Work row says. Also read `docs/permission-decisions.md` and `spikes/hooks-permission/REPORT.md`. Leave `crates/rup` and `crates/desktop` to the H8/H9 and H4 slices.

- `agent.permission` also hands the payload to the adapter as A1's Signal, so the A1 and A2 replays pass unchanged; `agent.signal` ignores a `PermissionRequest` payload (H9).
- The `allow` and `deny` reply text is built only in `crates/agents/src/claude_code/`, from the report's exact strings (P4).
- `decision.answer` checks `proof` against an `Option<String>` set at construction (`None` refuses all with `FORBIDDEN`); add no stdin, environment or file handling.
- Amend `a1_`, `a2_`, `a4_`, `d2_` tests only where H9 names them.
