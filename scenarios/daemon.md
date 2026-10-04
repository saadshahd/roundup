# Daemon

Module: `crates/rupd` (D1, D7); end-to-end tests in `crates/rup/tests` (D2–D7). Ids: every `D` heading in the file.

D1 and D7 are in `crates/rupd`. D2 to D7 are end-to-end tests of the whole Daemon. They live in `crates/rup/tests`, the only package whose tests can run both the Daemon (a dev-dependency) and the built `rup` binary that hooks and MCP servers call (`CARGO_BIN_EXE_rup`). They run without `claude`: a fake `claude` named by `ROUNDUP_CLAUDE_BIN` plays recorded hook payloads from `spikes/hooks-state/log*.jsonl`. `CLAUDE_CONFIG_DIR` points at a temp dir, so no test ever touches the user's `~/.claude.json`.

**D1 attached.** Given `rupd <project> --attached`, when its stdin closes, then it exits within a bound the test names, and every program it started in a Terminal is gone. Without `--attached`, a closed stdin changes nothing; CI starts `rupd &` with no stdin. The App passes `--attached`, so no Daemon and no Agent outlives the App, even when the App crashes.

**D2 Status end to end.** Given a Daemon whose fake `claude` feeds these hook payloads, in order, to the hook command in its `--settings` file: `SessionStart`, `UserPromptSubmit` (prompt "fix the refresh race"), `PermissionRequest`, `PostToolUse`, `Stop`. `ROUNDUP_RUP_BIN` is the built `rup`. When a subscribed client calls `agent.spawn`, then it receives that Agent's `agent.status` Kinds `idle, working, needs-you, working, idle` in order, and `rail.tree` names the Agent `fix-refresh-race` (A9).

**D3 Todos over MCP end to end.** Given the same Daemon with a fake `claude` that starts the `roundup` server from its `--mcp-config` file and calls `todo_create {title: "split checkout"}` over it, then `todo.list` holds that Todo and `provenance.history {item: "todo:1"}` names that Agent.

**D4 ten idle Agents.** Given ten Agents whose fake `claude` sends `SessionStart` and then sleeps, then the Daemon's resident memory grows by less than 150 MB. This is rule 7's budget for roundup's own share; the real `claude` programs are measured on the laptop at the MVP gate. The test prints the number it measured, for the gate report. Every Agent reaches Idle within one bound of 30 s for all ten together, measured from the first `agent.spawn`; a miss is not a pass, and its failure output names the Agents that were not Idle and each Agent's last status, so a lost event can be told from a slow Daemon. The bound covers only the ten `SessionStart` hooks reaching the Daemon, not the machine's load; a test that passes by waiting longer is not a fix.

**D5 no program outlives the Daemon, children included.** Given `rupd <project> --attached` with an Agent whose fake `claude` starts a background child that writes its pid to a file, and a Terminal whose shell does the same, when the Daemon's stdin closes, then the Daemon exits within D1's bound and the fake `claude`, the shell and both children are gone, checked by pid. Without `--attached` the same close changes nothing, as in D1. Owns `crates/rup/tests/e2e/**`, with fixes in `crates/rupd/**` or `crates/terminal/**` only if it fails today. Stacks after D1 and A17; it keeps `d1_attached` tests green and narrows none.

**D6 a Daemon killed without warning leaves a Project that reopens whole.** Given a Daemon with a Group, two Agents, a Terminal, three Todos where Todo 2 is blocked by 1, and two Pads (one owned by an Agent, one by the user), when its program is killed with SIGKILL and a new Daemon starts on the same Project folder, then `rail.tree` has the same node ids, names, parents, `order` values and `meta` flags, every Agent is `done` and every Terminal node has `terminal_id: null` (A8, A12), `todo.list` has the same three Todos with the same blockers and `done` state, `pad.list` the same Pads with the same owners and text, `provenance.history` still names the Actors of the earlier writes, and the next `todo.create` returns id 4. Owns `crates/rup/tests/e2e/**`. It keeps A8, T5, P7 and A12 green and narrows none; a failure here is fixed in the module that loses data, never by the test.

**D7 a store that cannot be read stops the Daemon, loudly.** Given a Project whose Rail, Todos or Pads file is truncated to a few bytes of garbage, when `rupd <project>` starts, then it exits with a non-zero status within a bound the test names, writes one line to stderr naming that file, accepts no connection, and leaves the file's bytes exactly as they were: it is never replaced with an empty store. A Project with none of the files starts as today. The App shows the Daemon's failure as it does for any exit (S3); D7 asserts only the Daemon's side. Owns `crates/rupd/**` and `crates/rup/tests/e2e/**`; if the failure sits in a module's `open`, that module's crate carries the fix and its test names D7.

## Work

Rows a Builder can take; `loop/rules.sh ready` prints each one's state.

| Ids | Item | Owns | Keeps green | After |
|---|---|---|---|---|
