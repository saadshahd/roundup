# Scheduled QA coordinator (L66)

The coordinator serves P2 and P5: a disposable isolated VM checks only public
trusted main, and never writes a branch. Install only the reviewed coordinator
files after they merge to trusted main, on one normal VM; do not run a Builder checkout there. The Builder snapshot
has no Run installation and must not register this job. This implementation is
fake-CLI tested; standby wake and the in-VM CLI integration still need a normal-VM
observation before claiming live verification.

Run's platform schedule wakes an auto-suspended coordinator. The only Run SDK
imports are `every` and `object`; VM operations use `Bun.spawn` and the installed
`boxd machine` CLI. The coordinator needs Bun, GNU timeout, bash, jq, curl, boxd
and Run. Fallow lists `loop/*.run.ts` as entrypoints and excludes the
platform-provided `@boxd/run` from dependency accounting; it is not an npm
dependency shipped with roundup. The QA snapshot needs git, pnpm, just, python3, jq, curl and agent-browser.
The remote command sources the snapshot’s `~/.cargo/env` to put just on PATH.
The existing sweep starts `just harness tree-40 5199` itself and owns its cleanup;
the coordinator starts no second harness. It does not run native tsc or Claude,
so does not use the snapshot reboot workaround from `docs/boxd.md`. A harness or
exec hang is red at the deadline, not reported as a successful sweep.

From the reviewed, merged main checkout on the normal VM, set the actual trusted public owner
(the Builder checkout deliberately has no git remote):

```sh
cd ~/roundup
export ROUNDUP_QA_REPO='<owner>/roundup'
TMPDIR=/tmp bun loop/qa-coordinator.run.ts dry-run
run loop/qa-coordinator.run.ts
run jobs --json
```

The dry run executes L66's fake command tests with no VM, network call, durable
state change or registration. Registration is exactly the same file path on
every deploy: Run replaces that path's job. Keep only one coordinator VM and
one registration. `object(name)` provides persistence, not distributed compare
and swap; the directory lease additionally fences overlapping replacement jobs
on this VM. Never register a copy on another VM. `ru-qa-scheduled` is reserved for
this job. Do not create or remove it while a tick is active.

Copy the returned job id into `QA_JOB` for these exact inspection commands:

```sh
export QA_JOB='<id from run jobs>'
run logs "$QA_JOB"
bun loop/qa-coordinator.run.ts check
cat loop/out/qa-scheduled/state.json
boxd machine list --json
```

`check` returns nonzero on missing state, a failed result, a stale lease, or a tick
more than 60 seconds overdue, including when the job never woke. It uses the
atomic local mirror; the Run object `roundup-qa-scheduled` is authoritative. Every
tick prints the full state to `run logs`; `failure` includes the next action.
Start/end are UTC, `head` is the exact API SHA (null if unresolved), and `latest`
contains status, output directory and cleanup outcome. The archive `sweep.tgz`
holds L52's JSON and screenshots; command/copy/cleanup logs are beside it. Only
the latest 144 output directories are retained. Archive evidence elsewhere
before it ages out. Run owns its own log retention.

The API read is unauthenticated and read-only. A rate limit is red. The isolated
QA VM clones public main and fetches the recorded SHA even if main advances
between the API read and clone; HEAD and origin/main are pinned to that exact
SHA before L52's guard runs. No PR ref, credential file, GitHub login or SDK
credential is copied in. VM counts respect `BOXD_MAX_VMS` (default 12) and
`loop/out/PAUSED`. Work gets 420 seconds, output copy 30 seconds, removal 30
seconds, and GNU timeout allows two seconds for forced termination per command.
The VM has a 540-second auto-destroy guard. Remote timeouts are unknown outcomes;
creation is followed by removal even when its reply was lost. Failed cleanup
retains the lease and retries removal after it becomes stale, without creating
another VM. All failures latch red until explicit recovery.

Stop, inspect, and recover only with the job stopped. `run stop` may interrupt a
tick before finally runs: wait for its VM's auto-destroy guard, or remove the
reserved VM explicitly, then confirm absence. Do not remove any other agent's VM.

```sh
run stop "$QA_JOB"
boxd machine list --json
# Only if ru-qa-scheduled still exists after the job has stopped:
boxd machine remove ru-qa-scheduled --confirm --json
boxd machine list --json
ROUNDUP_QA_MODE=recover run loop/qa-coordinator.run.ts
run loop/qa-coordinator.run.ts
```

Recovery refuses while the reserved VM exists or listing fails, logs the previous
state as an acknowledgement, clears the lease/failure and resets the due time.
The last failed result remains visible until the next completed sweep. For a
permanent stop, finish cleanup and use `run remove "$QA_JOB"` instead of recovery
and registration. Never delete durable state to conceal a missed tick.

Next Driver self-dispatch: obtain independent review of L66, then on the normal
VM run the dry run, register the reviewed file, observe one exact-SHA sweep and
cleanup, allow the coordinator to auto-suspend, and verify the next scheduled
wake in Run logs. Test stop/recovery and delayed-wake red status. Record the
actual outputs before calling the automation live-verified. This does not
replace macOS CI or WKWebView proof.

## Command validation

The official [in-VM CLI guide](https://docs.boxd.sh/guides/vm-to-vm)
and [command reference](https://docs.boxd.sh/cli/commands) document `machine
new`, `list`, `exec --timeout`, `cp` and `remove --confirm`. The snapshot,
isolation and destroy-timer arguments also match `loop/boxd.sh` and the measured
recipes in `docs/boxd.md`. The [Run scripts reference](https://docs.boxd.sh/guides/automations/scripts)
documents `every` and top-level assignments on `object`; the coordinator uses no
machine SDK methods. [Bun's subprocess reference](https://bun.sh/docs/runtime/child-process)
documents argv arrays, environment passing, streams, file descriptors and
`exited`. The fake-CLI test exercises that Bun boundary, including inherited
PATH, jq capacity parsing, archive copy and removal.

The Builder VM has neither `boxd` nor `run` installed: `command -v` and attempts
to read `boxd machine ... --help` confirmed this on 2026-10-03. CLI signatures
are documentation-checked, not live-verified here. Before registration the Driver
must compare the normal VM's installed `--help` for `new`, `exec`, `cp`, `list`
and `remove` against these arguments and record one disposable isolated sweep.
No VM was created by this Builder; it cannot run `boxd machine list` here to
attest to the account's other VMs.

## Builder proof

The continuation fixed a healthy overlapping directory lease being marked red,
made offline checks share the scheduled health rules, and set the remote exec
timeout explicitly. New regression tests failed before those fixes. The
original implementation's earlier test results are superseded by the evidence
below. Full check output is retained locally under `loop/out/qa-builder-proof/`;
the Driver must attach it to the PR before cleaning this ignored directory.

Validation uses the existing directory `/tmp/roundup-qa-check` as `TMPDIR`.
Focused L66 tests: 26 passed. L52 fake-browser sweep tests passed. `pnpm install
--frozen-lockfile`, `pnpm slop` (zero dead code or duplication), `pnpm lint`
(zero errors; existing unused `EmulatorFactory` warning in desktop perf), and
`loop/rules.sh vocab` passed. Strict standalone core TypeScript checking also passed.

`TMPDIR=/tmp/roundup-qa-check just check` exited 0 on this continuation's
full run: 636 Rust tests and 947 desktop tests passed (one existing todo),
along with lint, typecheck and slop. No test failed, so no isolated flake rerun was needed. This is
local Linux evidence, not CI or macOS approval. No registration, push or GitHub
credential use occurred.
