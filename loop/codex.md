# Codex in the fleet

One local Builder uses the Mac's `codex login` (ChatGPT) and the shared `.agents/builder.md`. GitHub keeps scheduling three Claude Builders while `LOOP_CODEX_ENABLED=true`; the local service owns the fourth slot. It takes ready rows through `runs.sh queue` and `claim`, then delivers PRs to the existing Claude review and CI gates. A sleeping Mac leaves the Codex slot idle.

The controller is a dedicated clean clone, separate from interactive worktrees. Invoke `python3 loop/codex.py <private-state-directory>` there every five minutes with a user LaunchAgent, `RunAtLoad=true`, and an explicit PATH containing `codex`, `gh`, GNU `gtimeout`, git, jq, Python, pnpm, Rust and just. Install dependencies before enabling it; the service never installs account credentials. `codex login status` must report ChatGPT and `gh` must have repository write and Actions-variable access. Set `LOOP_CODEX_ENABLED=true` only after the slot reservation is on main. Keep one installation per repository.

Rust tests run serially (`NEXTEST_TEST_THREADS=1`) because the existing local A22/A23 resume observers race under concurrent tests; no assertions are skipped.

Each tick refreshes the controller from main, waits for that exact main's green check, then builds in its own clone. An OS lock and interrupted-run record keep one local Attempt. The two-hour timeout covers Codex and its subprocesses; a failed Attempt waits an hour, with two attempts per row and main SHA. Claims with a PR stay; other Claims are released. All clones and logs stay in the private state directory for recovery.

`last.json`, each Attempt's `record.json`, `result.md` and `events.jsonl` are the local evidence. `LOOP_CODEX_STATUS` gives the Status issue the last row, timestamp, result and token counts. These counts are separate from GitHub's Claude Ledger; neither is a measurement of subscription quota remaining. Raw local transcripts are not uploaded.

To stop: disable the LaunchAgent, let its running Attempt finish (or terminate its recorded timeout process group), and then set `LOOP_CODEX_ENABLED=false` to restore the fourth Claude slot. Never restore that slot while Codex is still running. If the controller exits during an Attempt, the next tick waits for its recorded process to end and recovers the Claim before taking another row.
