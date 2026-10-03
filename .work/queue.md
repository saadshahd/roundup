# Work queue

This is the queue until roundup can hold its own Todos (`docs/development-loop.md`). One line per item: scenario ids, module, owned files and the observer that says the item is done (`AGENTS.md` rule 1). An item without scenario ids and an observer is not ready and is not dispatched. Scenarios with the same PR key ship as one PR in one worktree. Line counts are estimates, sized to the guide of about 2000 lines (#39).

Volatile state is not kept here (it goes stale within hours): no PR numbers, verdicts or merge status. Find a PR with `gh pr list --search "<id> in:title"`; `gh pr list --state merged` and `git log origin/main` are the record of what is done. A row names ids, an owner, owned files and an observer. Text for an id is its heading in `scenarios/` (`grep -n '^\*\*<id>[ .]' scenarios/*.md`); an id with no heading there is reserved in the id table below and its text arrives with its own PR, so check before dispatching it.

A Builder edits only the files its row owns, never `scenarios/`, the README or this file. Observer for every row: `just check` green and the item's own tests (named by its id prefix) pass; UI rows also have QA drive a `just harness <seed>` page.

## In flight

A Builder holds each row and its PR is in review. Nothing below is dispatched again.

| Item | Ids | Owns | Observer | Claimed by |
|---|---|---|---|---|
| spawn with a prompt (`⇧⌘N`) | U33 | `apps/desktop/src/rail/spawn*`, `Rail.tsx`, `u32_` and `u33_` tests | `u33_` tests pass; the U32 case "cmd and shift" keeps `⇧⌘T` doing nothing | boxd-agents |
| reopen, App half | S5 | `crates/desktop/**` | `s5_` tests pass; `s1_` and `s3_` tests stay green | boxd-agents |
| D4 flake: one deadline and a readable failure | D4 | `crates/rup/tests/**` | the Builder reproduces first; `just check` | boxd-agents |
| hold an early Signal until the Agent is registered | A14 | `crates/agents/**` | `a14_` tests fail before the fix and pass after | boxd-agents |
| `review` keeps the branch's trailers across a merge | L22 | `loop/boxd.sh` and its tests | `loop/boxd.test.sh` | boxd-agents |
| the boxd check does not depend on files outside the checkout | L40 | `loop/boxd.sh` (shared with L22: merges after it), the tsc setup and their tests | `loop/boxd.test.sh` | boxd-agents |
| empty states | U38 | one small file per region in `src/rail`, `src/todos`, `src/pads`, `src/terminal` (`src/rail` shares `Rail.tsx` with U33: merges after it) | U2's tests; narrows U14's test | boxd-agents |
| the Drawer takes and gives back focus | U40 | `apps/desktop/src/drawer/**` | `u27_focus_already_in_another_field_is_left_alone`, the `u28_` tests | boxd-agents |
| MVP gate | U2-U21 screenshots; cold start, keystroke-to-render p95, RSS with 10 idle Agents | QA and Driver, no Builder | the Phase 3 screenshot pack and the gate report | QA |

## Ready now

Scenario text is on main and nothing it needs is unmerged. Every row owns a directory no ready or in-flight row owns, so all start at once. Start the first row you have a free Builder for.

| Id | Item | Owns | Keeps green |
|---|---|---|---|
| audit-todos | coverage audit: T1, T2, T3, T5, T7 | `crates/todos/**` | existing tests |
| audit-pads | coverage audit: P2, P3, P7 | `crates/pads/**` | existing tests |
| audit-perf | coverage audit: R1-R7 | `crates/perf/**` | existing tests |
| U41 | Rail by keyboard, the rest (Moves: none) | `apps/desktop/src/keys/**`, one line in `src/App.tsx` | U31's and U32's tests |

## Waiting

Scenario text is on main. The row starts when what it waits on has merged. A UI row ends its Item with `(Moves: <D ids>)`, the design-system checks it moves (`docs/design-system.md`, baseline protocol); the Builder's PR body repeats the line and corrects it if the diff moves other checks.

| Id | Item | Owns | Keeps green | Waits on |
|---|---|---|---|---|
| L44–L49 | merge policy: `class`, `rounds`, `merge-ready`, `revert-due`, `dispatch` in `loop/rules.sh`, stall kind f in `loop/stalls.sh`, the `Scenarios:` check in `loop/boxd.sh build`, the new `loop/rules.sh` steps in `.github/workflows/loop.yml` | `loop/rules.sh`, `loop/rules.test.sh`, `loop/stalls.sh`, `loop/boxd.sh` (shared with L22 and L40: merges after them), `loop/boxd.test.sh`, `.github/workflows/loop.yml` | `loop/rules.test.sh`, `loop/boxd.test.sh` | L28 and L33 implemented (stall kinds, `base`), the U130 and L41 row merged first (it owns the same files) (the user opened the post lane on 2026-10-02) |
| L50 | split the queue's rows into `.work/queue/<squad>.md`, put a `Module:` line in each scenario file, delete the README table, add `loop/rules.sh queue`, and name `.work/queue/*.md` as a `post` path in L44 and AGENTS.md rule 1 | `.work/**`, `docs/squads.md` (the "Moving the queue" section only), `scenarios/*.md` (first lines only), `scenarios/README.md`, `loop/rules.sh`, `loop/rules.test.sh`, `loop/stalls.sh` is not touched | `loop/rules.test.sh` | the L50 text merged, and no docs PR in the merge lane when it runs (the Driver announces it) |
| L51 | stall kind g in `loop/stalls.sh` (a rejected PR with no push for 60 minutes) | `loop/stalls.sh`, `loop/stalls.test.sh` (or the file L28's tests live in) | `l51_` tests | after the L44–L49 row's `loop/stalls.sh` PR merges (kind f; one writer of `loop/stalls.sh` at a time) |
| L54 | `loop/rules.sh carry <pr>`, its call in `merge-ready` (L46) and `trailers` | `loop/rules.sh`, `loop/rules.test.sh`, `docs/development-loop.md` | `l54_` tests | after the L44–L49 row's `loop/rules.sh` PR merges (it owns `merge-ready` and `trailers`); #207 (per-squad queue files) removes most queue conflicts and is independent |
| L53 | `loop/rules.sh proof <pr>`, its call in `merge-ready` (L46), the `proof/pr-<n>` branch convention, AGENTS.md rules 1 and 8 (already edited here), the Builder and Reviewer prompts | `loop/rules.sh`, `loop/rules.test.sh`, `docs/development-loop.md` | `l53_` tests | after the L44–L49 row's `loop/rules.sh` PR merges (it owns `merge-ready`); until it lands the rule is by hand: no visible PR approves without screenshots in its body |
| U44, U46, U47, U49 | rail polish: rows under the pinned bar, hover jitter, `⌘T` selects the Terminal, Live line title (Moves: D1, D6) | `apps/desktop/src/rail/**` (one PR, so the four do not collide) | `u6_`–`u10_`, `u9_` tests, `u30_`, `u32_`, `u22_` | after U31 and U33 merge |
| U103 | a reloaded Terminal pane shows its screen (webview half) | `apps/desktop/src/terminal/**` | U104's Builder PR |
| U104 | `terminal.snapshot` and the `offset` on `terminal.output` (Daemon half; contract, rule 4) | `crates/terminal/**`, `crates/contracts/**` and generated files, `crates/rupd/src/**` | the `vt100` parser in `crates/terminal` on main |
| U105–U107, U109, U110 | the Thread's input and feed: dim, folded lines, jump, Attachment, Quote (Moves: D3) | `apps/desktop/src/thread/**` (new); U107 also `apps/desktop/src/rail/**`; U106 and U107 add chords to U54's help, which ui-surfaces owns | after the Door scenarios (not yet written); U107 after B6's `takeover.begin` Builder PR; narrows U12 for U107 |
| B24 | a Meta-agent's Brief carries three sentences | `crates/agents/src/claude_code/**` (E1's Brief file) and its `e1_` test | E1 on main; `awareness.md`'s architect approves |
| U101, U102 | Terminal copy and paste (Moves: D3); Terminal text size (Moves: D2) | `apps/desktop/src/terminal/**` | `u11_`–`u13_` tests | U101 after U57 merges; U102 needs U100 (merged); ids U101–U129 are reserved |
| A15 | clean environment for every program the Daemon starts | `crates/terminal/**`, `crates/agents/src/claude_code/**`, `crates/rupd/src/lib.rs`, `crates/rup/tests/**`; no `SpawnParams` field; no vendor name in `crates/terminal` (P4) | the `a` and `x` tests | after #114 lands |
| B1-B18 | Messages and Routes: contract (rule 4, an architect other than the author approves), `crates/messages`, the registration in `crates/rupd`, the MCP tools in `crates/rup` | `crates/contracts/**` and generated files, `crates/messages/**`, `crates/rupd/src/**`, `crates/rup/src/**`, `crates/rup/tests/mcp.rs` (the amended M1 test), `scenarios/mcp.md`'s M1 text | `a`, `t`, `d`, `m` tests; narrows M1 (`m1_offers_one_tool_per_method_and_no_others`) | after the H11–H17 row (`Agents::prompt`) merges for B2, and after A16 for `crates/contracts/src/methods.rs` |
| B19-B23 | child digests for Meta-agents: `agent.digest` (contract, rule 4), the composer and caps in `crates/messages`, the tool `agent_digest` in `crates/rup` | `crates/messages/**`, `crates/contracts/**` and generated files, `crates/rup/src/**`, `crates/rup/tests/mcp.rs` (the amended M1 test) | `a`, `t`, `m` tests; B1-B18's tests | after the B1-B18 row merges (same crates) |
| A16 | `rail.remove` (contract change, committee approved) | `crates/contracts/src/methods.rs` and `contracts/generated/methods.ts`, `crates/agents/src/lib.rs` and `rail.rs`, `apps/desktop/src/testing/seeds.ts`; nothing in `crates/desktop` or `rupd`; its `a16_` tests in `crates/agents/tests/scenarios/**` and `crates/rup/tests/**` | the `a` tests | after #114 lands (both edit `crates/agents/src/lib.rs`) |
| U56 | right-click menu: stop, remove (Moves: D1, D6, D7, D8) | `apps/desktop/src/rail/menu/**`, one line each in `Rail.tsx`, `RailRow.tsx`, `src/rail/keys.ts` (created by U31) | `u9_`, `u31_`, `u33_` tests | after A16, U31, U33 and the rail polish PR (U44, U46, U47, U49); stacking allowed |
| U57 | no pane header (Moves: D1) | `apps/desktop/src/terminal/**` | `u14_with_nothing_selected_the_pane_is_empty`; replaces the other `u14_` header tests; the U4 computed-colour test drops the pane header | after U56 merges (or in its PR) |
| U58 | an Agent's Pads under it, on demand (Moves: D1, D2, D4, D6) | `apps/desktop/src/rail/pads/**`, one line in `RailRow.tsx`, `src/pads/**`, one case in `src/keys/**` | `u20_` tests; narrows two named `u18_` tests | after U31, U33, U36, U41, U50 and U56; stacking allowed |
| U59 | the terminal blends with the app (Moves: D1, D5) | `apps/desktop/src/terminal/**` and its stylesheet | `u11_` to `u13_` tests; QA screenshots in `artifacts/ux/U59/` (not gating) | after U57 and the contrast tokens merge |
| U37 | reopen, webview half (Moves: D3, D5, D6, D7) | `apps/desktop/src/app/**` | `u25_daemon_gone.test.tsx` before a reopen | after S5 merges |
| sweep-rupd-harness | one shared way for tests to start a `rupd` and wait for `daemon.ping` | tests under `crates/rup/tests/**` and `crates/rupd/tests/**` | every existing test | after D4 merges |
| U50 | the Pad's text fills its Drawer (Moves: D1, D2, D5) | `apps/desktop/src/pads/**` | `u20_` tests | after U38 merges (it adds a pads file) |
| T8 | `Todo.creator` (contract change approved) | `crates/todos/**`, `crates/contracts/**` and generated files, `crates/rup/tests/e2e.rs`, the webview Todo fixtures T8 lists | `t` tests, `u15_`, `u17_` tests | after A15 and A16 merge (A15 edits `crates/rup/tests/e2e.rs`, A16 edits `src/testing/seeds.ts`) |
| U60 | an Agent's Todos under it (Moves: D1, D2, D4) | `apps/desktop/src/rail/pads/**` (shared with U58), `apps/desktop/src/todos/**` | `u15_`, `u17_`, `u35_`, U58's tests | after T8 on main; may stack on U58's branch |
| contrast | Live lines and Ink text read at 4.5:1 (U4, U25) (Moves: D1, D5) | `src/styles.css`, `src/rail/styles.css`, the contrast tests, `u5_drawer.test.tsx` | U4, U25, U50 | go from the user; starts when this text is on main; observer: the computed-colour and stylesheet tests, `just check` |
| notch | U55: report which element draws the empty-Terminal notch | no app file (a report and screenshots under `artifacts/ux/U55/`) | U55 | go from the user; observer: the report |
| U100 | the Rail's selection and collapse survive a restart (webview storage, no seam) (Moves: none) | `apps/desktop/src/rail/persist/**`, one mount line in `Rail.tsx` | `u3_`, `u8_`, `u30_` tests | after U31 and U33 merge |
| U130, U131, L41 | visual pass 1, Tokens: `tokens.css` holds every look value (starting values in `docs/design-system.md`), the four stylesheets read `var(--…)`, type and space steps; `loop/rules.sh tokens` with its tests, run by a `tokens` step in `.github/workflows/loop.yml` | `apps/desktop/src/tokens.css`, `apps/desktop/src/**/*.css`, `loop/rules.sh`, `loop/rules.test.sh`, `.github/workflows/loop.yml` (the scenario says so) | every existing `apps/desktop` test; each `u4_`, `u5_` or `u20_` test that asserts a colour U130 replaces is changed and named in the PR | now, unless the `contrast` row is already in flight, then after it (D5 carries its ratios); one PR, so the check and the migration land together |
| U137 | one module computes D1 to D10; `window.__checks()` on the harness page | `apps/desktop/src/testing/**`, `apps/desktop/harness.html` | `u26_` tests | after U130 and U131 merge |
| U132, U133 | visual pass 2, colour and click targets: Kind tones, `--accent`, hover, pressed and focus states, 24 px hit areas | `apps/desktop/src/rail/**`, `apps/desktop/src/drawer/**`, `apps/desktop/src/todos/**`, `apps/desktop/src/pads/**` | every test of those folders | after U130, U137, U35 and U36 merge |
| U134, U135 | visual pass 3, surfaces and schemes: `--sunken` and `--ground`, the one hairline, the Drawer's shadow, dark, more contrast, reduced transparency | `apps/desktop/src/**/*.css`, `apps/desktop/src/drawer/**` | every existing test | after U132 and U133 merge |
| U136 | the finish line: D1 to D10 pass on `first-run`, `agents-10`, `tree-40`, `daemon-exits`, `conflict`; QA and the critic's baseline shows each check `fixed`, none `regressed` | no Builder; QA, Design critic and Driver | `u136_` | after U134 and U135 merge |
| G contract | `project.setWorktrees`, `project.get`, `agent.worktreeState`, `agent.land`, `agent.discard`, `RailNode.worktree` (docs/worktrees.md) | `crates/contracts/**` with `contracts/generated/**`, and every `RailNode` literal (the webview fixtures, `seeds.ts`, the e2e tests) | `cargo test -p contracts`, `just check` | after the worktrees docs PR, A16 (the contract PR adds `worktree_unlanded` to `rail.remove`, which A16 introduces) and T8's implementation merge, because all three edit `crates/contracts/**` and `seeds.ts`; committee approval (rule 4) |
| G1, G2, G3, G7 | the `worktrees` setting, spawn provisions a Worktree, status, a Rail move leaves the Worktree alone | `crates/agents/src/worktree.rs`, `crates/agents/src/rail.rs` and `lib.rs` (shared with A16: merges after it), `agents.db`'s schema, `crates/rup/tests/**` e2e (D4 owns it and `sweep-rupd-harness` changes it: merges after both) | the `a4_` tests, `just check` | after G contract, A16, D4 and `sweep-rupd-harness` merge |
| G4, G5, G6 | Landing, safe removal and discard, restart | the same files as G1 to G3 | the `g1_` to `g3_` tests and `g7_` | after G1, G2, G3 merge |
| H1b | permission spike follow-up: always-allow persistence, `AskUserQuestion` after an answer, Esc on its UI, and macOS recorded (H1 itself is the committed `spikes/hooks-permission/REPORT.md`) | `spikes/hooks-permission/**` | `finding` lines in the report | none (needs a login and network; not in `just check`) |
| H2, H3, H5, H6, H7, H10 | permission Decision store and contract: `Decision`, `agent.permission`, `decision.list`, `decision.answer`, `decision.opened`, `decision.cleared`, the Observations `Answered` and `Dismissed` (contract change; prompt `.work/prompts/h-decision-store.md`) | `crates/contracts/**` and generated files, `crates/agents/src/decision.rs` (new), `crates/agents/src/lib.rs`, `crates/agents/src/claude_code/**`, `crates/agents/tests/**`, `crates/rupd/src/**` (register the methods), `apps/desktop/src/testing/seeds.ts` | `h2_`, `h3_`, `h5_`–`h7_`, `h10_` tests, and the `a1_`, `a2_`, `d2_` tests H9 names | after the A15 and A16 Builder PRs (they edit `crates/agents/src/lib.rs`) |
| H8, H9 | `rup permission` and the settings hook line (prompt `.work/prompts/h-rup-permission.md`) | `crates/rup/src/main.rs`, `crates/rup/tests/**`, `crates/agents/src/claude_code/launch.rs`, `crates/agents/tests/launch.rs`, `docs/adr/0006-claude-code-hooks-for-state.md`, A4 in `scenarios/agents.md` | `h8_`, `h9_` tests, the `a4_` launch tests | after the H2 row merges |
| H4 | the answer proof: the App makes it, `rupd` reads it from stdin and strips it from child environments, `daemon_proof` (App seam; prompt `.work/prompts/h-proof-handshake.md`) | `crates/rupd/**`, `crates/desktop/src/daemon.rs`, `crates/desktop/src/lib.rs`, `crates/desktop/tests/**`, `apps/desktop/src/app/**`, `scenarios/app.md` (the seam row) | `h4_` tests | after the H2 row merges; also edits `crates/agents/src/claude_code/launch.rs`, which the H8 and H15 rows own, so it waits for both to merge |
| H11, H12, H16 | the Steer: `agent.prompt`, `Agents::prompt`, first prompt through it, no timed delay (contract change; prompt `.work/prompts/h-agent-prompt.md`; the error codes `BUSY`, `NOT_ACCEPTED`, `NOT_RUNNING`, `NOT_ACKED` get wire codes -32005 to -32008 in `crates/rpc/src/error.rs`) | `crates/contracts/**` and generated files, `crates/rpc/src/error.rs`, `crates/agents/src/lib.rs`, `crates/agents/src/claude_code/**`, `crates/agents/tests/**`, `crates/rupd/src/**`, A4 in `scenarios/agents.md` | `h11_`, `h12_`, `h16_` tests | after the H2 row and the A15/A16 PRs merge; this row owns `Agents::prompt` (variants `Busy`, `NotAccepted`, `NotFound`) and supersedes the A30 row (architect-b drops A30 and B2 calls this function)
| H13, H17 | the Interrupt: `agent.interrupt`, the title acknowledgement, `NOT_ACKED` (contract change; prompt `.work/prompts/h-agent-interrupt.md`) | `crates/contracts/**` and generated files, `crates/agents/src/lib.rs`, `crates/agents/src/claude_code/**`, `crates/agents/tests/**`, `crates/rupd/src/**` | `h13_`, `h17_` tests | after the H11 row merges (error codes, `lib.rs`)
| M4 | `ttlMs` on every `tools/list` reply, a live defect (the real Claude Code client rejects the reply); no Decision store needed (prompt `.work/prompts/m-ttl.md`) | `crates/rup/src/mcp.rs`, `crates/rup/tests/mcp.rs`, `scenarios/mcp.md` (line 3 excepts M4's VM run) | `m4_` tests (the empty list and a second page built from a fake Daemon) and the VM run M4 names | may start now; the H14 row also owns `crates/rup/src/**` and `crates/rup/tests/mcp.rs`, so it waits for this one to merge |
| H14 | the `ask_user` MCP tool and the amended `m1_` closed-list test (contract change; prompt `.work/prompts/h-ask-user.md`) | `crates/rup/src/**`, `crates/rup/tests/mcp.rs`, `crates/agents/src/decision.rs`, `crates/agents/src/claude_code/launch.rs`, `crates/contracts/**` and generated files | `h14_` tests, `m1_` | after the H2 row and the M4 row merge; the escalation chain before the user's Inbox is architect-b's B-series
| H15 | hook fork cost: current-thread runtime for `rup signal`, `STATE_EVENTS` cut by replay, a perf metric (prompt `.work/prompts/h-hook-cost.md`) | `crates/rup/src/main.rs`, `crates/rup/tests/**`, `crates/agents/src/claude_code/launch.rs`, `crates/agents/tests/**`, `docs/perf.md`, `scenarios/perf.md` | `h15_` tests, a `just perf` metric | may start now; the H8 row also owns `crates/rup/src/main.rs` and `launch.rs`, so whichever starts second waits for the first to merge
| (UI half of Decisions) | the Decision in the App: its text, `allow` and `deny`, clearing | architect-b writes the scenario; ids from `U100` to `U129` | | after H2–H10 merge |
| L43 | `loop/rules.sh ranges` | `loop/rules.sh`, `loop/rules.test.sh` | `l43_` tests | after the L41 row's `loop/rules.sh` PR and the L44–L49 row's PR merge (all three own `loop/rules.sh` and `loop/rules.test.sh`, one at a time); `docs/squads.md` is on main (#126) |
| E7 | spike: does the real Claude Code take a Brief, a `SessionStart` context and an MCP server at start | `spikes/context-injection/**` | the report; no app code | now; needs the installed `claude` |
| E1-E6 | Brief, `agent.context`, `rup context`, `agent_context`, `rup mcp` connecting at start, the `RailNode.channel` field (the Chip is E8, reserved); the contract rides in this PR (committee approval) | `crates/agents/src/**`, `crates/agents/tests/**` and `claude_code/`, `crates/rupd/src/**` (it composes the Rail and the Todos for E2), `crates/rup/src/**`, `crates/contracts/**` with `contracts/generated/**`, `crates/rup/tests/**`, every `RailNode` literal (the webview fixtures, `seeds.ts`) | the `e1_` to `e6_` tests, with the `a4_` and `m1_` tests kept green; `just check` | after E7, after B12 and T8's implementation merge |
| F2, F3, F4, F5 | a Meta-agent has no shell, `agent_spawn`, the PATH shim, stray detection; the contract rides in this PR (committee approval) | `crates/agents/src/**`, `crates/agents/tests/**` and `claude_code/`, `crates/rup/src/**`, `crates/rup/tests/**`, `crates/contracts/**` with `contracts/generated/**`, every `RailNode` literal | the `f2_` to `f5_` tests, with the `a4_`, `a7_` and `m1_` tests kept green; `just check` | after #159 (G2, Home), #162 (E1) and #160 (B12) merge, and after E1 to E6 or in the same PR order |
| F7 re-run | run `spikes/spawn-boundary/run-interactive.sh` on a `ru-spike-<x>` VM whenever the pinned `claude` version changes and update the report's interactive rows; a result that contradicts F2 goes to the Architect | `spikes/spawn-boundary/**` | the report's interactive rows, dated | F2 implemented |

The coverage audit: every scenario id in `scenarios/*.md` has a test with its lowercase id as a prefix, except the `L` ids, whose tests are shell scripts that name them `L<n>` (`loop/*.test.sh`), W1 (its observer is `just check`), F1 and F7 (records of runs of `spikes/spawn-boundary`), and the ids in flight. The ids above have exactly one test each. An audit Builder reads each of its scenarios clause by clause, adds one test per clause that no test asserts, adds none for a clause already covered, and puts a table of id, clause and test name in the PR. It adds no scenario text and changes no behavior; a clause that the code does not satisfy is a defect to report to the Architect, not to fix in the audit PR.


## Deferred past the MVP (no scenario yet)

- motion.md rows other than the Drawer slide and drag;
- Terminals named from their first command;
- the `● 1 below` line (U30 covers jumping, not the pinned line), provenance letters, the Todo `on` field and the Pad storage switch;
- Inbox, Messages, Routes, Extensions and history;
- packaging (a signed `.app`).

## Swarm protocol

**Stacking.** A Builder may branch from an unmerged base PR's branch when its scenario says "after X" or "may stack", opens its PR against `main` as `AGENTS.md` says, and merges only after its base has merged. Its own commits touch no file its base's commits touch, so the two stay reviewable apart.

**Writers.** A writer is an agent that writes scenarios and no code: the Architect gives it one scenario file and one id range, and records both in the id table below. It owns the scenario file and the id range the Architect gave it, edits no other scenario file and not this file, and never decides a contract change, a new RPC method or an App seam command; those and any change to a merged scenario go to the Architect.

Agents coordinate only through this repo: this file, `scenarios/`, PRs and their commit trailers. The WIP limit, the `Claimed-by:` line and the one-day claim expiry are queue policy added by the Architect, not `AGENTS.md` rules; change them here.

- **Claiming.** An item is claimed by a `Claimed-by: <agent-id> <date>` line under its row. Only the Architect pushes it to main (`AGENTS.md`: nobody else pushes there). A Builder requests a claim by being named in a dispatch; a Builder never claims by editing this file in its own PR, and does not start an item that has no claim. A claim with no branch or PR after a day is released by the Architect.
- **WIP limit.** At most 12 Builders work at once (matching `BOXD_MAX_VMS`), counting every open PR that is not waiting on review. The Architect dispatches no thirteenth. `boxd` VMs are a further limit, `BOXD_MAX_VMS`, default 12 (`AGENTS.md`).
- **One id per author, a different id per reviewer.** Every authored commit carries `Author-Agent: <id>`. The Reviewer's approval is an empty commit that carries only `Reviewed-by-Agent: <id>`, and its id differs from every `Author-Agent` in the PR. It is the newest commit: anything pushed after it needs a new approval. `loop/rules.sh trailers` reads `git log --no-merges`, so it does not see a merge of main made after the approval; the Merger checks that by hand (`git log --first-parent` shows no merge above the approval). A merge of main that touches only a lockfile gets a fresh empty `Reviewed-by-Agent` commit and no more. A Reviewer is never given the Builder's rationale (rule 5).
- **How the Merger gates.** The Merger merges a PR only when rule 1 holds, all at once: CI `check` green; the scenario ids named in the PR have passing tests, in e2e where the scenario is end to end, and the item's named observer passes as well, never instead; an approval from a different id (for a PR of class `post`, `loop/rules.sh merge-ready` instead, and the independent verdict follows within 60 minutes, `AGENTS.md` rule 1 and L44 to L49); `loop/rules.sh trailers`, `size origin/main` and `vocab` clean; zero anti-slop findings. PRs merge in the order of their "merge after" column, and a stacked PR merges after the PR it stacks on. If main is red, the Merger stops and reverts; it never fixes forward (rule in `AGENTS.md`).
- **Stale branches.** A Builder whose branch is behind main merges `origin/main` into its branch (the repo's practice, for example on `builder/mcp`), runs `just check`, pushes, and asks for a new approval. A stacked branch merges its base's head first, then main. It never rebases a branch with an approval or a review in flight.
- **`Cargo.lock` and `pnpm-lock.yaml` conflicts.** Two PRs that add packages both rewrite the lockfile, and the merge leaves conflict markers inside it. Never edit them by hand. Take main's file and let the tool regenerate the branch's additions: `git checkout origin/main -- Cargo.lock && cargo update -w`, and `git checkout origin/main -- pnpm-lock.yaml && pnpm install`. Then commit the result with the merge.
- **Scenario ids are reserved, not taken.** When the Architect dispatches an item, it reserves that item's ids in a line under its row, `ids: U27–U28`, pushed to main like a claim. A Builder writes scenarios and tests only under its reserved ids and asks the Architect for more rather than picking the next free number. A Reviewer blocks a PR whose ids are not reserved to it, or that clash with an id in another open PR or on main. This is queue policy added by the Architect, like the claim line. Current allocation:

  | Ids | Holder |
  |---|---|
  | U23–U24 | #60 `ux-layout` |
  | U25 | #61 `ux-daemon` |
  | U26 | #62 `chrome-harness` (renumbered from U23) |
  | U27–U28 | `ux-drawer` (focus on close, Esc, the Drawer edge) |
  | U29 | `ux-pads` (the Pad row) |
  | U30 | #74, jump to what needs you (merged) |
  | U31 | Rail keyboard, builder-ux-drawer |
  | U32 | #75, spawn shortcuts (merged) |
  | U33 | spawn with a prompt, boxd-agents |
  | S5 | reopen, App half, boxd-agents |
  | U34 | scrollback, boxd-agents |
  | U35 | Todo triage, boxd-agents |
  | U36 | Pad edit safety, boxd-agents |
  | U37 | reopen, webview half |
  | U38 | empty states |
  | U39 | retired: superseded by #93's build-time probe (`just perf-keystroke`, K1 to K3, R11); #90 closed. Rule 7's keystroke-to-render number is #93's real-window p95 in WKWebView, one probe and one pairing rule (the first render after the emulator parsed the typed character itself); no second in-app hook that pairs a keystroke with any later output, because it can end early on unrelated output and so bounds nothing. The Daemon-side cheap bound already exists as `just perf`'s write-to-output limit (R8) |
  | U40 | the Drawer's focus |
  | U41 | Rail by keyboard, the rest |
  | U42 | dropped: a false positive (a stuck key in the test driver), never an app defect |
  | U43 | dropped: not a bug (the automated double-click hit the Live line, not the name) |
  | U44, U46–U47, U49 | rail polish |
  | U45, U48 | `⌘J` rules and the chip |
  | U50 | Pad text fills its Drawer |
  | U51 | quick switcher |
  | U52 | first run: recent folders and the `claude` line |
  | U53 | Todos list motion |
  | U54 | keyboard help on `?` |
  | U55 | diagnose the empty-Terminal notch |
  | U56 | right-click menu: stop, remove |
  | U57 | no pane header |
  | U58 | an Agent's Pads under it |
  | U59 | the terminal blends with the app |
  | U60 | an Agent's Todos under it |
  | R11, R12 | #93, keystroke probe (merged) |
  | R13 | load-aware perf comparison |
  | A13 | symlinked agents directory |
  | A14 | hold an early Signal until the Agent is registered |
  | A15 | clean environment for every program the Daemon starts |
  | A16 | `rail.remove` |
  | T8 | `Todo.creator` |
  | S6 | first-run facts in the App seam (`recent_projects`, `claude_binary`) |
  | S7 | the smallest window (640 by 400) |
  | A17, A18, D5–D7, T9, P10 | #125, daemon squad robustness (A18 stacks after A14, #114) |
  | A19 | a removed node's id is never reused: written after A16 (#120) merges; reserved to the daemon squad |
  | C1 | dropped-call outcome (`scenarios/rpc.md`) |
  | L9–L14 | `boxd-swarm` (#73): L9 review, L10 input, L11 reboot, L12 swarm, L13 status, L14 kill; it also rewords L6 (secret) and L8 (cap) |
  | L15 | #66, `loop/boxd.sh check` and `bake` (merged) |
  | L16 | `review` replays an empty approval commit |
  | L17–L20 | loop hardening: run logs, swarm throttle, run events and retry, ref per prompt |
  | L21 | streamed agent run |
  | L22 | `review` keeps the branch's trailers across a merge |
  | L23 | VM tools |
  | L24 | agent search servers |
  | L25 | design critic run |
  | L26–L27 | `loop/wake.sh`: L26 `watch`, L27 `check`; architect-b's split of #133 |
  | L40 | `boxd check` does not depend on files outside the checkout (round cap on #140: amend; one more reject on the same clause retires it) |
  | U61–U79 | `scenarios/ui-rail.md`, writer-ui-rail |
  | U80–U99 | `scenarios/ui-surfaces.md`, writer-ui-surfaces |
  | U100–U129 | `scenarios/ui-*.md`, architect-b (U100 `ui-persist.md`, U101–U102 `ui-daily.md`, U103–U104 `ui-reload.md`, U105–U110 `ui-attention.md`) |
  | U130–U137 | `scenarios/ui-visual.md`, architect visual-system |
  | G1–G9 | `scenarios/worktrees.md`, architect-c (G1–G7 written; G8–G9 reserved for the UI half) |
  | B1–B29 | `scenarios/messages.md`, architect-b (B24–B27 from the anchor rules; B25, B26, B27 are spikes) |
  | L44–L49 | `scenarios/loop.md`: merge policy, `loop/rules.sh` `class`, `rounds`, `merge-ready`, `revert-due`, `dispatch` and stall kind f (rows below) |
  | L41–L42 | `scenarios/loop.md`: L41 `loop/rules.sh tokens` (row U130), L42 `loop/rules.sh delta` (done, with this reservation) |
  | H1–H29 | `scenarios/decisions.md` (H1–H10) and `scenarios/control.md` (H11–H17), permission Decisions and the control channel, architect-swarm (written; the UI half is architect-b's, from the `U` ranges) |
  | L43 | `loop/rules.sh ranges`: squad ranges hold only unused ids |
  | E1–E9 | `scenarios/awareness.md`, architect-c (E1–E7 written; E8–E9 reserved) |
  | F1–F9 | `scenarios/spawn-boundary.md`, architect-c (F1–F5 and F7 written; F6 the Rail's `stray` badge; F8–F9 reserved) |

  Every Builder has ids; the Architect reserves more on request. `scenarios/ux.md` is gone: every candidate in it now has a `U` id above.
- **Every item names scenarios and an observer.** An item with no scenario ids, or with no observer a Reviewer can run, is returned to the Architect. A new idea starts as a scenario in `scenarios/` (an Architect PR), then becomes a row here.
- **Stop conditions.** If `loop/out/PAUSED` exists, a limit was hit: stop and tell the user. A defect that needs a change in `contracts/`, `CONTEXT.md` or the App seam stops the Builder and goes to the Architect.

## Linux VMs and CI

Once `crates/desktop` is a workspace member, every `just check` compiles Tauri, which makes macOS CI slower. On Linux, Tauri needs WebKitGTK and its dev packages. Unverified here: the `ru-toolchain` snapshot is believed to have none of them, so a `boxd` `just check` would fail in every module until the snapshot is rebaked (`loop/boxd.sh bake`). Default until the user decides (open question): run Builders in local worktrees.
