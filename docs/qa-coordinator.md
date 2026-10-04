# Scheduled QA coordinator (L66)

Runbook for the one normal VM that sweeps public `main` every 10 minutes (P2, P5). Behaviour: `scenarios/loop-qa.md`.

## Facts

| Item | Value |
|---|---|
| Coordinator VM | one normal VM with the reviewed, merged `main` checkout; never a Builder checkout or a second registration |
| Needs | Bun, GNU timeout, bash, jq, curl, boxd, Run; `ROUNDUP_QA_REPO=<owner>/roundup` |
| QA VM | `ru-qa-scheduled` (reserved), from `ru-toolchain`, isolated, auto-destroy 540 s, auto-suspend 0, made with `@boxd-sh/sdk@0.2.9` `machines.create` (the CLI lacks the timer flag) |
| QA snapshot needs | git, pnpm, just, python3, jq, curl, agent-browser |
| Deadlines | work 420 s, output copy 30 s, removal 30 s; a tick over 60 s late is red |
| State | Run object `roundup-qa-scheduled` (authoritative), mirror `loop/out/qa-scheduled/state.json`; the latest 144 output dirs, each with `sweep.tgz` (L52's JSON and screenshots) |
| Credentials | none: an unauthenticated read-only API call for the SHA, a public clone pinned to it; SDK errors are replaced with a fixed message |
| Verified | SDK creation with these settings (live probe, 2026-10-04); registration, a full tick and standby wake are not yet verified |

## Install

```sh
cd ~/roundup
export ROUNDUP_QA_REPO='<owner>/roundup'
pnpm install --frozen-lockfile
TMPDIR=/tmp bun loop/qa-coordinator.run.ts dry-run   # fakes only: no VM, network or state
run loop/qa-coordinator.run.ts                        # same path every deploy replaces the job
run jobs --json
```

## Inspect

```sh
export QA_JOB='<id from run jobs>'
run logs "$QA_JOB"
bun loop/qa-coordinator.run.ts check   # nonzero on missing state, a failure, a stale lease or a late tick
boxd machine list --json
```

Every failure stays red until recovery; `failure` in the state names the next action.

## Recover

Only with the job stopped, and touching no other agent's VM:

```sh
run stop "$QA_JOB"
boxd machine list --json
boxd machine remove ru-qa-scheduled --confirm --json   # only if it still exists
ROUNDUP_QA_MODE=recover run loop/qa-coordinator.run.ts
run loop/qa-coordinator.run.ts
```

Recovery refuses while `ru-qa-scheduled` exists or listing fails. To stop for good, clean up and `run remove "$QA_JOB"`. Never delete state to hide a missed tick.

## Before calling it live

After independent review of L66, on the coordinator VM: dry run, register, watch one exact-SHA sweep and cleanup, let it auto-suspend, confirm the next wake in `run logs`, then exercise stop, recovery and a late-wake red. It never replaces macOS CI or WKWebView proof.
