# Loop boxd

Module: `loop/boxd.sh`. Each id's cases are its `L<n>` tests in `loop/boxd.test.sh`; facts in `.agents/data/boxd.md`.

| Exit | Means |
|---|---|
| 0 | done |
| 1 | failed; the VM, if any, is destroyed |
| 2 | bad input, before any VM or file exists |
| 75 | paused on a usage limit (`loop/out/PAUSED`) |

**L4 pause.** A final `result` event reporting a 429 or a usage or rate limit writes `loop/out/PAUSED` and exits 75; while it exists no VM is made.

**L5 cleanup.** Every exit path destroys the VM the run made, keeps the exit code, and reports a failed removal as a leak.

**L6 secret.** Without the `CLAUDE_CODE_OAUTH_TOKEN` boxd secret no Claude VM is made.

**L7 isolation.** A Claude Builder's VM is `--isolated` from `ru-toolchain`.

**L8 cap.** At `BOXD_MAX_VMS` `ru-` VMs, or a held lock, no VM is made.

**L9 review.** `review <name> <prompt> [ref]` checks out the ref's real history on an isolated `ru-<name>` VM (`ru-<name>-r<pid>` when taken) and writes `loop/out/verdicts/<name>.md`.

**L10 input.** A bad name, a `-`-leading prompt or ref, or a `-r<digits>` name exits 2.

**L11 reboot.** A snapshot VM is rebooted and answers `exec` before anything runs on it.

**L12 swarm.** `swarm build|review <prompt>…` runs one VM per prompt within the cap, prints one result line per VM, and cleans up on a signal.

**L13 status.** `status` prints each `ru-` VM as `agent-running`, `idle` or `unreachable`; a missing boxd or a failed `boxd machine list` exits 1 naming it.

**L14 kill.** `kill <name>` removes `ru-<name>` and its `-r<digits>` twins; `kill all` removes only swarm VMs.

**L15 check.** `check <pr|branch>` merges the ref into `origin/main` on the laptop and runs `just check` on an isolated VM; `bake` refuses a snapshot holding a token.

**L16 empty approval.** A review checkout keeps an empty approval commit.

**L17 run logs.** Each run logs to its own `loop/out/runs/<name>-<UTC>-<pid>.*`.

**L18 slot wait.** `BOXD_SLOT_WAIT` seconds of waiting for a free slot before failing; `swarm` waits 900. Every wait polls each `BOXD_POLL` seconds (1), and the slot and lock waits count polls.

**L19 retry.** A VM that never answers, or an early upload failure, is retried once on a fresh VM before the agent starts; retries, silent runs and deadlines go to `loop/out/events.log`.

**L20 swarm ref.** `swarm review <prompt>@<ref>` reviews each prompt at its own ref.

**L21 streamed run.** Every agent event streams to the run's `.jsonl`; the verdict, cost and pause come from the final `result` event; `BOXD_AGENT_TIMEOUT` is a wall-clock limit.

**L22 trailers.** `trailers` on a review checkout gives the same result as on the real branch.

**L40 hermetic check.** `check` fails when a package relies on files outside the checkout.

**L64 Codex Builder.** `BOXD_AGENT=codex` moves only the Codex login to a normal `ru-toolchain` VM and proves no GitHub login is reachable before uploading source.

**L65 Codex Reviewer.** A Codex review uses L64's guards and writes the final completed answer as the verdict.

## Work

| Ids | Item | Owns | Keeps green | After |
|---|---|---|---|---|
| L25 | `critic <pr> <prompt>`: the Design critic's run on an isolated VM | `loop/boxd.sh` | `L4`–`L22` | — |
| L61 | VM start and upload checks | `loop/boxd.sh` | `L11`, `L19` | — |
