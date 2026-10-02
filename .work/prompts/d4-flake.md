You are a Builder (`.agents/builder.md`). Fix the nondeterminism in scenario D4 (`scenarios/daemon.md`, "D4 ten idle Agents") and nothing else. Read `AGENTS.md`, `CONTEXT.md`, `scenarios/daemon.md` and `.claude/sound/` first.

Evidence: at 8acd9f3, `d4_ten_idle_agents_cost_the_daemon_less_than_150_mb` (crates/rup/tests/e2e.rs) failed once with "the event arrived: Elapsed(())" after 30.4 s from `next` in crates/rup/tests/e2e/daemon.rs, then passed on an identical rerun; it passed at 803c5f7. The commits between touch only apps/desktop and one agents launch test. The cause is unknown: either an `AgentStatus` Idle event was lost or the Daemon was slow under load. Do not guess and do not raise the 30 s.

You may edit only `crates/rup/tests/**`. If a lost event proves to be a Daemon bug, stop, do not fix it, and report the evidence to the Architect.

Do, in order:
1. Reproduce: run D4 in a loop (for example `cargo test -p rup --test e2e d4_ -- --nocapture` 30 times, once while another build keeps the machine busy) on a boxd VM and record how many fail. Say so in the PR if you cannot reproduce.
2. Make the wait follow D4's new text: one deadline of 30 s for all ten Agents counted from the first spawn (not 30 s per event), and on a miss panic with the Agents that were not Idle plus each Agent's last status seen. Also read `rail.tree` at the miss and print each Agent's status there: if the tree says Idle for an Agent whose event never arrived, that is a lost event and the Builder reports it as such.
3. If the reproduction shows a lost event, the test must wait on the Daemon's state (poll `rail.tree` until ten Agents are Idle, within the same single deadline) instead of on the event stream, and the report names the lost event for the Architect.
4. Name tests after the scenario (`d4_...`). Every existing test stays green. Run `just check`; no perf runs. Every commit carries `Author-Agent: <your id>`.

Report: how many of N runs failed before and after, the failure output if any, and which of "slow Daemon" or "lost event" the evidence supports.
