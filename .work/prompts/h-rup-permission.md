You are a Builder (`.agents/builder.md`). Build `rup permission` and the settings hook line: scenarios H8 and H9 in `scenarios/decisions.md`, and nothing else. Read `AGENTS.md`, `GLOSSARY.md`, `PRINCIPLES.md` (P4), `scenarios/decisions.md`, `docs/permission-decisions.md`, `spikes/hooks-permission/REPORT.md` and `.claude/sound/` first. This slice needs the Decision store slice merged first (`agent.permission` and its contract exist on main); if they are not on main, stop and say so.

Edit only: `crates/rup/src/main.rs`, `crates/rup/tests/**`, `crates/agents/src/claude_code/launch.rs` and `crates/agents/tests/launch.rs`, `docs/adr/0006-claude-code-hooks-for-state.md`, and the A4 sentence in `scenarios/agents.md` that H9 amends. Do not touch the store, the contract or `crates/desktop`.

Build, test first, tests named `h8_...` and `h9_...`:
1. `rup permission <agent-id>`: read one hook payload from stdin, identify as that Agent (as `rup signal` does), call `agent.permission`, and print the returned `output` on stdout unchanged, exit 0. It is vendor-blind: no vendor string in `crates/rup`.
2. H8, each case run against the built `rup`: no `RUPD_SOCKET` or no Daemon, stdin not JSON, `NOT_FOUND` for the id, and a Daemon that stops answering: one line on stderr naming the cause, nothing on stdout, exit 1; never exit 2; never print a decision it did not receive. `AskUserQuestion` returns an empty `output`: print nothing on stdout, write `answer in the Terminal` to stderr, exit 0 (H2).
3. H9: the per-Agent `--settings` file runs `<rup> permission <agent-id>` for `PermissionRequest` with `timeout` 86400, and `rup signal <agent-id>` with the 5-second timeout for every other event in `STATE_EVENTS`; amend A4's wording and the ADR 0006 sentences exactly as H9 says. Existing launch tests change only where they assert the old single hook line.
4. Run `just check`. Every commit carries `Author-Agent: <your id>`.

Report: the scenarios proven (one test name each) and any case of H8 that needed a decision from the Architect.
