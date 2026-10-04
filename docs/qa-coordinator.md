# Scheduled QA coordinator (L66)

The coordinator serves P2 and P5: a disposable isolated VM checks only public
trusted main, and never writes a branch. Install only the reviewed coordinator
files after they merge to trusted main, on one normal VM; do not run a Builder
checkout there. The Builder snapshot has no Run installation and must not
register this job. The creation path uses the SDK verified by the Driver on
2026-10-04;
registration, an end-to-end sweep and standby wake still need normal-VM proof.

Run's platform schedule wakes an auto-suspended coordinator. The only Run SDK
imports are `every` and `object`. Creation uses the pinned official
`@boxd-sh/sdk@0.2.9` dependency in a one-shot Bun child; other VM operations use
`Bun.spawn` and the installed `boxd machine` CLI. The SDK uses automatic in-VM
authentication; no authentication value is read, printed or copied by coordinator
code. The coordinator needs Bun, GNU timeout, bash, jq, curl, boxd and Run. Fallow lists `loop/*.run.ts` as entrypoints and excludes the
platform-provided `@boxd/run` from dependency accounting; it is not an npm
dependency shipped with roundup. The QA snapshot needs git, pnpm, just, python3, jq, curl and agent-browser.
The remote command sources the snapshot’s `~/.cargo/env` to put just on PATH.
The existing sweep starts `just harness tree-40 5199` itself and owns its cleanup;
the coordinator starts no second harness. It does not run native tsc or Claude,
so does not use the snapshot reboot workaround from `.agents/data/boxd.md`. A harness or
exec hang is red at the deadline, not reported as a successful sweep.

From the reviewed, merged main checkout on the normal VM, set the actual trusted public owner
(the Builder checkout deliberately has no git remote):

```sh
cd ~/roundup
export ROUNDUP_QA_REPO='<owner>/roundup'
pnpm install --frozen-lockfile
TMPDIR=/tmp bun loop/qa-coordinator.run.ts dry-run
run loop/qa-coordinator.run.ts
run jobs --json
```

The dry run executes L66's injected SDK and fake command tests with no VM,
network call, durable state change or registration. Registration is exactly the same file path on
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
The VM has a platform 540-second auto-destroy inactivity guard, with auto-suspend
disabled. This is not a promised wall-clock expiry. The SDK child uses the same
remaining work deadline through GNU timeout, covering authentication and creation,
and disables SDK retries. A nonzero creation exit or timeout is unknown; SDK
errors are replaced with a fixed message to keep authentication details out of logs.
Remote timeouts are unknown outcomes;
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

## Verified SDK creation and deployment gates (2026-10-04)

The normal VM's CLI has no `--auto-destroy-timeout`; creation therefore uses
`new Boxd().machines.create` from the official SDK, with these exact settings:

```ts
await boxd.machines.create({
  name: "ru-qa-scheduled",
  fromSnapshot: "ru-toolchain",
  isolated: true,
  config: { autoDestroyTimeout: 540, autoSuspendTimeout: 0 },
});
```

The Driver's live probe used installed `@boxd-sh/sdk@0.2.9` with automatic in-VM
authentication to create `ru-sdk-ttl-probe` from `ru-toolchain`, isolated, with a
300-second destroy timer and auto-suspend zero. External `boxd machine get`
confirmed isolation, snapshot, auto-destroy 300 seconds and auto-suspend off.
The probe was removed. This verifies that creation path and configuration
mapping; it does not demonstrate actual expiry or a complete coordinator tick.
The coordinator retains L66's 540-second setting, proven at the injected SDK boundary.

The one-shot `create` mode never imports Run or registers a job. It runs only
inside the existing bounded command wrapper during a tick. Cleanup is persisted
as pending before launching it, and any failure or lost reply still leads to
removal by the reserved VM name. The rest of the CLI workflow is unchanged.

Deployment still requires independent review and reviewed main, locked dependency
installation, registration on the normal coordinator VM, exact-SHA sweep and
cleanup evidence, and standby-wake proof. Follow the Driver steps above, including
stop/recovery and delayed-wake checks. Do not register from a Builder checkout.

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

## L66 review-fix proof

The stale-restart copy and automated-check regression tests failed against the
reviewed implementation. Cleanup now attempts the bounded archive copy into the
recorded output directory before removal, retains the stale failure, records
copy errors, and removes even when copying fails or times out. A fake CLI also
proves the archive reaches disk before removal after a restart. Both L66 suites
run in `just check`; macOS CI installs Bun, jq and GNU timeout first.

Proof logs are under `loop/out/l66-fix-proof/` (ignored); the Driver must attach
them to #281. These changes serve P2 and P5: neither shares a working directory
nor writes a shared branch. Deployment remains subject to the gates above.

On 2026-10-04, `TMPDIR=/tmp/roundup-qa-check just check` exited 0: 636 Rust
tests, 31 L66 tests and 950 desktop tests passed (one existing todo). Lint had
only the existing desktop `EmulatorFactory` warning; slop found no dead code or
duplication. The L52 fake-browser sweep, strict standalone coordinator typecheck,
vocabulary and diff whitespace checks also passed. Size printed only its advisory.
Self-review is in the proof directory; it is not independent approval. macOS CI
has not run for this new head. The full check waited on another build's Cargo
lock and finished without interrupting it.

## SDK continuation proof (2026-10-04)

The new SDK parameter test failed before implementation while all 31 existing
L66 tests passed. After replacing creation, both suites pass 35 tests, including
the exact 540/0 timers and isolation, remaining deadline and persisted cleanup
ordering, and rejected/hanging SDK child outcomes. The dependency is pinned to
the Driver-probed `@boxd-sh/sdk@0.2.9`; SDK retries are disabled.

Proof and self-review are in `loop/out/l66-sdk-proof/` (ignored), for the Driver
to attach to #281. Locked installation, root typecheck, strict core typecheck,
slop, vocabulary and whitespace checks pass. Lint has only the existing desktop
`EmulatorFactory` warning. An additional strict runner/test typecheck is limited
by existing Bun stdout union typing and missing platform `@boxd/run` declarations;
its output is retained. No job was registered, no authentication value was
printed or moved, and no GitHub write or live VM operation was performed here.
Independent approval and deployment proof remain outstanding.

`TMPDIR=/tmp/roundup-qa-check just check` exited 0: 636 Rust tests, 35 L66 tests,
and 950 desktop tests passed (one existing todo). One A14 Rust test took 168
seconds but passed in the original full run; no test failure needed a retry.
This is Linux proof, not exact-head CI or macOS approval.
