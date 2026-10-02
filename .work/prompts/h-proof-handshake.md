You are a Builder (`.agents/builder.md`). Build the answer proof: scenario H4 in `scenarios/decisions.md` plus the App seam row `daemon_proof` in `scenarios/app.md`, and nothing else. Read `AGENTS.md`, `CONTEXT.md`, `PRINCIPLES.md` (P1), `scenarios/decisions.md` (H4, H5), `scenarios/app.md` (the App seam table and S1), `docs/permission-decisions.md` and `.claude/sound/` first. This slice needs the Decision store slice merged first (`decision.answer {id, answer, proof}` and the Daemon's `Option<String>` proof exist on main); if they are not on main, stop and say so.

Edit only: `crates/rupd/src/**`, `crates/rupd/tests/**`, `crates/desktop/src/daemon.rs`, `crates/desktop/src/lib.rs`, `crates/desktop/tests/**`, `crates/agents/src/claude_code/launch.rs` (environment stripping), `apps/desktop/src/app/**` (the call and its test), `scenarios/app.md`. Do not touch the store or `rup permission`.

Build, test first, tests named `h4_...`:
1. The App creates a random proof when it starts and gives it to `rupd` on its stdin (never as an argument, an environment variable or a file); the Daemon holds it in memory. A Daemon started without it refuses every `decision.answer` with `FORBIDDEN`.
2. The Daemon removes the proof from every environment it builds for an Agent, a Terminal and an MCP server (it is never in one: a test reads the environment the fake `claude` and its Terminal receive and finds none).
3. The Tauri command `daemon_proof` (no arguments) returns the proof to the webview, which keeps it in memory only; add its row to the App seam table and change both sides in this PR (S1's rule). The webview's `decision.answer` call passes it.
4. H4's cases, each with the fake `claude` as a child inside an Agent's Terminal using the environment it inherited: `decision.answer` with a missing or wrong proof, with no `daemon.identify`, and as an Agent or Extension, all give `FORBIDDEN` and leave the Decision open; the `rup mcp` server lists no tool that answers or lists Decisions; a grep test finds no second caller of `decision.answer` in `crates/`.
5. Run `just check`; a Daemon-side change to the seam needs an architect other than the author to approve at the exact head, so name it in the PR body. Every commit carries `Author-Agent: <your id>`.

Report: the scenarios proven (one test name each) and the exact place the proof is read, held and removed.
