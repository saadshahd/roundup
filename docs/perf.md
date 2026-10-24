# Perf

Rule 7's budgets and what measures each. Linux VM numbers are Linux numbers. The macOS baseline was recorded on a laptop in ordinary use (load 4 to 7 on 16 cores, 0.25 to 0.45 per core), not an idle one, so it is a baseline for that machine under that load.

| Budget | Measured by | Where |
|---|---|---|
| Daemon cold start < 300 ms | `just perf` (`cold_start_ms`: spawn `rupd` to its first `daemon.ping` reply) | any machine; the App's cold start to a visible window is manual |
| Cold plugin materialization < 300 ms | `just perf` (`cold_plugin_materialization_ms`: pinned Q11 plugins into fresh empty storage) | any machine; 10% regression against its recorded Linux baseline |
| 10 idle Agents < 150 MB extra RSS | `just perf` (`rss_extra_mb`: the Daemon's growth with ten login-shell Terminals) | any machine; the Agents' own processes are manual |
| Keystroke-to-render < 16 ms p95 | `just perf-keystroke`, only when the user asks (below) | the macOS App only |
| Regression above 10% fails | `just perf` against `crates/perf/budgets.json` | same OS as the baseline |
| 100-tool-use loop's hook-process cost stays within a fixed limit | `just perf` (`hook_loop_ms`: 200 `rup signal` calls against a fresh Daemon, H15) | any machine; limit-only, no regression ceiling |

**H15's hook cost.** `rup signal` cost 3.8 ms wall and 4.3 ms CPU per call on architect-swarm's Mac before H15 (read by hand, not committed), about 760 ms wall for the 200 calls (`PreToolUse` and `PostToolUse`) a 100-tool-use loop makes. The replay rule in `crates/agents/tests/state_events.rs`, which also replays each fixture's titles and compares the whole Status, finds no event in `STATE_EVENTS` prunable: `PreToolUse`'s Signal is what lets A2's star hold (`crates/agents/src/claude_code/mod.rs`) tell a star that may precede a dialog from one that may not. `STATE_EVENTS` keeps all nine events and a tool use still costs two `rup signal` calls. The one saving is a current-thread runtime in `crates/rup/src/main.rs`, so `rup signal` runs one OS thread (`h15_rup_signal_never_runs_more_than_one_os_thread`, `crates/rup/tests/signal.rs`). `hook_loop_ms` is the wall time of 200 sequential `rup signal` calls against a fresh Daemon; it is limit-only, with no baseline and no R13 skip. Its `limit` of 2000 ms is a gross tripwire, not a saving: the figure was read by hand and is not committed. The end-to-end check, with the real settings `agent.spawn` writes, is `h15_every_hook_call_of_a_hundred_tool_use_loop_is_answered_and_the_final_stop_leaves_the_agent_idle` (`crates/rup/tests/e2e.rs`).

`just perf` runs a fresh Daemon 11 times, takes each metric's median, prints the range and each run's load average, writes `target/perf.json` and exits 1 on a miss. Run it on a quiet machine: load is printed so a noisy run can be thrown away. A metric fails above its `limit`, or above its OS's `baseline` by more than 10% plus its `noise` allowance.

**Q11's cold plugin materialization.** Each release `perf` run materializes the pinned plugin bundle into a new empty temporary directory. Only `plugins::materialize` is timed; `bundle.validate()` must then succeed or the observer exits 2. The median is printed as `cold_plugin_materialization_ms` and retained in `target/perf.json`. Its rule 7 limit remains 300 ms on every OS at every load. Regression above the recorded Linux baseline by more than 10% plus the clock-resolution allowance fails (exit 1), using the same R13 load rule as other latencies. `just check` verifies empty-directory materialization and validation without a wall-clock assertion (`q11_cold_materialization_produces_a_valid_bundle`).

Q11's Linux baseline is 11.940635 ms, the median of nine eleven-sample release observations on 2026-10-14 (Ubuntu 24.04, four available CPUs, AMD EPYC 7763 64-Core Processor, rustc 1.99.0). The nine medians were 11.889899, 11.971422, 12.378832, 11.931701, 11.680185, 11.874712, 11.944743, 11.940635 and 12.034941 ms (max/median 1.037, min/median 0.978); median load per CPU was 0.60–1.20. The probe is commit `9ff3cf3e` in [#1762](https://github.com/saadshahd/roundup/pull/1762); this baseline applies to that Linux machine class, not macOS. The clock-resolution allowance is 0.001 ms, and R13's Linux threshold is 2.0 load per CPU. The 300 ms hard limit applies even when load skips the regression comparison; macOS has the hard limit until its own baseline is measured. Tests `q11_cold_plugin_materialization_keeps_the_300_ms_limit`, `q11_cold_plugin_materialization_rejects_a_release_regression_below_the_hard_limit` and `q11_perf_reports_cold_plugin_materialization_and_fails_an_over_budget_median` exercise the gates with synthetic samples, never clock assertions.

## What is enforced, and what is not

Rule 7's 10% regression test is enforced for six metrics on Linux (including Q11) and five on macOS (below) — except that a time metric's regression test is skipped, loudly, when the machine is busier than its baseline's load (R13) — and is a gross tripwire for the rest. A latency miss on macOS is rerun on a quiet machine before it is believed. Measured on an isolated 2-vCPU boxd VM (AMD EPYC, load 2 to 4 from the runs themselves): 9 invocations of 11 runs each, the spread of the 9 medians against their median was

| Metric | max / median | min / median | Gate |
|---|---|---|---|
| `rss_extra_mb` | 1.05 | 0.93 | 10% + 0.05 MB |
| `ping_p95_ms` | 1.13 | 0.96 | 10% + 0.001 ms |
| `rail_tree_40_p95_ms` | 1.06 | 0.97 | 10% + 0.001 ms |
| `terminal_write_p95_ms` | 1.04 | 0.88 | 10% + 0.001 ms |
| `write_to_output_p95_ms` | 1.09 | 0.96 | 10% + 0.001 ms, limit 16 ms |
| `cold_start_ms` | 3.08 | 0.57 | limit 300 ms only: its median varies 3x between identical invocations, so no 10% test can hold |
| `rail_tree_10_p95_ms` | 1.66 | 0.51 | limit 2 ms only (about 8x its median): same reason |

`noise` is an absolute floor for the clock's resolution, not slack: it is 0.05 MB on a 1.6 MB metric and 1 microsecond on latencies of 25 to 265 microseconds. The worst observed ratio (1.13, `ping`) is a 0.5 microsecond overshoot that the floor covers; with only 9 invocations, one false fail in a few dozen runs is expected, so rerun before believing a single fail. The `linux-boxd` baselines are the medians of those 9 invocations and are valid for that VM class only. The `macos` baselines are in the next section; the `limit`s hold anywhere. To accept a new baseline, copy the medians from `target/perf.json` into `budgets.json` under the OS name (`macos`) or Linux class key (`linux-boxd`, `linux-9v45`, `linux-7763`, `linux-9v74`, `linux-8573c`, `linux-8370c`) in the same PR that explains why. See `scenarios/perf.md` (R1 to R10).

## Linux Builder classes (R16 specification)

R16 supports five distinct Builder identities. All require `ID=ubuntu`, `VERSION_ID=24.04`, architecture `x86_64`, online CPU set `0-3`, four available CPUs and unrestricted affinity `0-3`:

| Class | Exact CPU model | Baseline and provenance |
|---|---|---|
| Builder 9V45 | `AMD EPYC 9V45 96-Core Processor` | Measured on 9V45 only |
| Builder 7763 | `AMD EPYC 7763 64-Core Processor` | Measured on 7763 only |
| Builder 9V74 | `AMD EPYC 9V74 80-Core Processor` | Measured on 9V74 only |
| Builder 8573C | `INTEL(R) XEON(R) PLATINUM 8573C` | Measured on Intel 8573C only |
| Builder 8370C | `Intel(R) Xeon(R) Platinum 8370C CPU @ 2.80GHz` | Measured on Intel 8370C only |
| boxd | Historical two-vCPU VM | Existing boxd values and provenance retained separately |

Record host identity with these commands in the same process environment as the observer; keep the full output with every measurement report:

```sh
cat /etc/os-release
LC_ALL=C lscpu
cat /sys/devices/system/cpu/online
nproc
sed -n '/^Cpus_allowed_list:/p' /proc/self/status
uname -a
rustc -Vv
cargo -V
printf 'SHELL=%s\n' "$SHELL"
```

`nproc` reports available CPUs; the online set and process affinity must both be `0-3`. Compare the parsed distribution, version, architecture and full CPU model for exact equality; do not match an EPYC or Xeon family prefix. Record release toolchain/build settings, login-shell workload and binary hashes with the reports as R16 requires. Kernel and shell differences are diagnostic metadata, not CPU identity. CPU affinity or CPU-count rescaling cannot convert boxd, 7763, 9V45, 9V74, Intel 8573C or Intel 8370C into one another.

The identity evidence in #1266 records a 9V45 Builder at `7922b6f225452fc4da68396d7d75b4ed0054fa74`; it establishes the supported identity, not performance. A 7763 report cannot establish a 9V45 baseline even with identical Ubuntu and CPU counts. An unknown identity, any missing required identity field, or a supported identity without its measured baseline and provenance is an observer error (exit 2), naming the mismatch or missing input. It cannot reuse another class's values or report a clean pass. R16 assigns selection/report metadata to `r16_builder_selects_its_measured_class` and these rejections to `r16_unknown_or_incomplete_linux_class_is_an_observer_error`; these are implementation obligations for all five Builder identities.

The identity evidence in #1303 records an Intel 8573C Builder at `ab349a1d5f46d0dce3c6d51a0a0f8617311c4669` on 2026-10-12: Ubuntu 24.04, x86_64, online CPUs and affinity `0-3`, four available CPUs, kernel `6.17.0-1022-azure`, rustc `1.99.0 (b940084d7 2026-09-28)` and cargo `1.99.0 (5f94df478 2026-08-27)`. This records identity only; AMD measurements establish no Intel baseline.

The identity evidence in #1313 records a 9V74 Builder at `2ae44989f5ec68a2c9021a381f5f2eae8cbb63ff` on 2026-10-12: Ubuntu 24.04 (`ID=ubuntu`, `VERSION_ID=24.04`), x86_64, exact CPU model `AMD EPYC 9V74 80-Core Processor`, online CPUs and affinity `0-3`, four available CPUs, kernel `6.17.0-1022-azure`, rustc `1.99.0 (b940084d7 2026-09-28)`, cargo `1.99.0 (5f94df478 2026-08-27)` and shell `/bin/bash`. This records identity only. No other CPU class can establish its baseline or provenance.

The identity evidence in #1941 records an Intel 8370C Builder at `06fd4eb7e24c476b104e52b94e78fa491d2f47f4`: Ubuntu 24.04.5 (`ID=ubuntu`, `VERSION_ID=24.04`), architecture `x86_64`, exact CPU model `Intel(R) Xeon(R) Platinum 8370C CPU @ 2.80GHz`, online CPUs `0-3`, four available CPUs and process affinity `0-3`. Diagnostic metadata: kernel `6.17.0-1022-azure`, rustc `1.99.0 (b940084d7 2026-09-28)`, cargo `1.99.0 (5f94df478 2026-08-27)` and shell `/bin/bash`. This is identity evidence only; its standalone preflight exit 2 is a qualification failure, not a `just perf` result or latency observation. No Daemon/probe build, timing or benchmark failure is claimed. No other CPU class establishes its baseline or provenance, including Intel 8573C.

#1266, #1303, #1313 and #1941 specify identities only; the Intel specification PR closes #1303 when its specification acceptance holds and the 9V74 specification PR uses `Refs #1313` and closes #1313 only when its specification acceptance holds; the 8370C specification PR uses `Refs #1941` and `Closes #1941` only when its specification acceptance holds. #1366 stays open for same-host Message latency evidence, #1361 for final combined latency acceptance and #1251 for observer implementation, independent calibration and completion observations. Measurement policy: for each newly measured class, including 9V74 and 8370C independently, R16 requires historical/current same-host diagnosis with nine alternating invocations of eleven fresh Daemons per reference, compatible identical probe bytes and isolation of probe changes using the same binaries, nine clean calibration invocations only after regression diagnosis and repair, then three separate clean eleven-Daemon observations on the implementation head with the baseline fixed. Retain every report, per-run metric and load, failed observation and repeat, exit code, reference commit, binary hash and exact command; retain and repeat busy invocations rather than counting them as clean. Diagnose and repair Daemon or probe regressions before accepting a baseline. This identity amendment supplies no evidence that either regression is absent.

### Class selection

`just perf` reads `/etc/os-release`, `lscpu`, `/sys/devices/system/cpu/online`, `/proc/self/status` and the available CPU count (`crates/perf/src/machine.rs`), picks the class whose exact identity matches, and compares only that class's `baseline` entry in `crates/perf/budgets.json`. `crates/perf/classes.json` holds each class's baseline provenance; `target/perf.json` and the printed `class` line carry the key, provenance and machine facts. The boxd class is two online CPUs, affinity `0-1` and an `AMD EPYC` model, so narrowing a Builder's affinity never selects it. An unknown model, a missing field, a class without provenance or without all five Linux baseline metrics (`ping_p95_ms`, `rail_tree_40_p95_ms`, `rss_extra_mb`, `terminal_write_p95_ms`, `write_to_output_p95_ms`) exits 2 naming it. Today only `linux-boxd` is fully measured; `linux-7763` holds Q11's baseline alone, so the Builder classes exit 2 until #3700 records their baselines, each under its own key and provenance. Load thresholds stay keyed by OS (`linux`).

R1–R15 and R8's pins remain unchanged, as do ten login shells, all measured calls, #1366's historical write ceilings and #1361's final combined acceptance. Every class keeps all absolute limits, 10% plus existing noise, Linux's load threshold of 2.0 per CPU, memory comparisons at every load and loud latency skips. A skipped latency comparison cannot complete R16, even on exit 0. No automatic baseline generation, removed metric, widened allowance, baseline increase to hide a regression or CPU-count rescaling is permitted. The implementation pins each class's values, provenance and thresholds in R8's tests; see R16 for test names and the failure/exit table.

### R23 execution lifetime (#3764)

[#3764](https://github.com/saadshahd/roundup/issues/3764) remains open: Builder
[40127972413](https://github.com/saadshahd/roundup/actions/runs/40127972413)
at `f82fd7499a07f81a8dd42947b2b9d3c353c15905` has exact model
`AMD EPYC 7763 64-Core Processor`, so its R23 host preflight exits 2.
The commands, full identity, statuses and existing-host search are retained in
[`r23-host-40127972413`](../crates/perf/evidence/r23-host-40127972413/README.md).
No R23 timing or profile was collected. The earlier 9V45 diagnosis below predates
R23's candidate and supplies neither a live host nor an R23 observation.

Builder [40131015578](https://github.com/saadshahd/roundup/actions/runs/40131015578)
also failed the exact-model preflight (exit 2): its model is
`Intel(R) Xeon(R) Platinum 8370C CPU @ 2.80GHz`. Ubuntu 24.04, x86_64,
four online/available CPUs and affinity `0-3` match, but do not qualify that CPU.
[Full identity and existing-host search](../crates/perf/evidence/r23-host-40131015578/README.md)
retain the commands and statuses. No reachable qualified lifetime was identified;
runner inventory returned HTTP 403 and active sibling logs HTTP 404. No resource,
R23 timing or profile was produced. #3764 remains open and #3760 stays blocked;
remaining measurement/repair work stays under #3720.

For the next execution attempt, use the existing Builder facility:

1. Search existing host evidence and active Actions jobs before allocating a host.
   A job name or `ubuntu-latest` label does not establish CPU identity or access.
   This attempt's runner inventory request returned HTTP 403; job metadata exposed
   no remote execution endpoint. The existing `build-issue.yml` selects
   `ubuntu-latest` without an exact CPU selector and serializes builds of one Issue.
   Dispatching another build during this one would not hand off its lifetime.
2. On the execution host, retain the full identity commands above and execute the
   preflight in `qualification.txt`. Require exit 0 and retain the Actions run/job,
   runner name and `/proc/sys/kernel/random/boot_id` to identify the lifetime.
   Exit 2 means stop before timing; never substitute another CPU class.
3. Keep that host alive through #3760's observations. Build historical
   `2029ed2c4ee606d26e73c858c4d7c93130df36e0` and candidate
   `50e546c1a3ce74689738b08a3dfe273895f9ae31` there with the same toolchain and
   release settings (`cargo build --release -p rupd -p rup -p perf`). Retain
   executable SHA-256 hashes and #3760's compatibility diff before measuring.
4. Execute #3760's compatible probe commands in order historical-1, candidate-1,
   historical-2, candidate-2, historical-3, candidate-3, eleven fresh Daemons each.
   Retain reports, logs, statuses, failed attempts and busy-pair repeats. Collect
   both diagnostic profiles separately from these timing invocations. Keep all
   evidence under `crates/perf/evidence/` and link it on #3760 before the job ends.
   If another run cannot execute on this same live host, the qualifying run must
   collect these observations itself; an identity-only handoff is incomplete.

R23 in `scenarios/perf.md` governs workloads, load qualification, ceilings and
attribution. #3760 owns probe compatibility and per-metric conclusions; remaining
measurement and repair work stays linked to #3720. This failed preflight establishes
no calibration, performance pass or completed host prerequisite.

### Builder 9V45 diagnosis (#3700, R16 steps 1 to 3)

No Builder baseline is recorded: the same-class comparison finds a regression, not a class mismatch, and R16 step 3 says to repair it before accepting baselines. The earlier 7763 figures (taken from `main` itself, with no historical comparison) are withdrawn; a 7763 baseline needs its own diagnosis on a 7763 host.

References: the commit that set the Linux baselines, `2029ed2c4ee606d26e73c858c4d7c93130df36e0` (2026-10-01), and `main` at `90df821c0` (2026-10-24), 21,720 added lines of Daemon and probe source apart. Both built with `cargo build --release -p rupd -p rup -p perf`, rustc 1.99.0; hashes and machine facts are in `crates/perf/evidence/linux-9v45/machine.txt`. Commands, nine rounds alternating the two on one VM (AMD EPYC 9V45, four CPUs), 11 fresh Daemons each, script `crates/perf/evidence/linux-9v45/diagnosis/reproduce.txt`: historical `perf --runs 11 --rupd <hist>/rupd --out historical-N.json`, current `perf --runs 11 --budgets <scratch>/budgets.json --out current-N.json`, where the scratch `budgets.json` and `classes.json` copy `linux-boxd` into `linux-9v45` so the current binary runs. Every report, log and exit code is kept (`historical-N.*`: exit 0; `current-N.*`: exit 1, the ceilings missed). Loads were 1.6 to 3.8 on four CPUs, under R13's 2.0 per CPU, so every invocation is clean.

| Metric | Historical median | Ceiling (×1.10 + noise) | Current median | Current invocations over ceiling |
|---|---|---|---|---|
| `ping_p95_ms` | 0.0358 | 0.0404 | 0.0700 | 9 of 9 |
| `rail_tree_40_p95_ms` | 0.1917 | 0.2119 | 0.3575 | 9 of 9 |
| `rss_extra_mb` | 2.5195 | 2.8215 | 3.0742 | 9 of 9 |
| `terminal_write_p95_ms` | 0.0624 | 0.0697 | 0.1077 | 9 of 9 |
| `write_to_output_p95_ms` | 0.1108 | 0.1229 | 0.1629 | 9 of 9 |

Conclusion per metric: the historical reference misses the boxd ceilings by a factor near 1.4 (`ping_p95_ms` 0.0358 against 0.025), which is class; current `main` is a further 1.5 to 2 times slower than the historical reference on the same VM, which is a regression that class does not excuse. `ping_p95_ms`, `terminal_write_p95_ms` and `write_to_output_p95_ms` keep step 3 in full; `rail_tree_40_p95_ms` is covered by the user's 0.71 ms settlement (its 0.3575 is under it); `rss_extra_mb` rose 0.55 MB and stays far under 150 MB.

Probe isolation failed and is recorded rather than hidden: the historical probe against today's `rupd` and today's probe against the historical `rupd` both exit 2 (`rail.createGroup` and `rail.createWorkstream` not found; `newprobe-histdaemon-N` and `oldprobe-curdaemon-N`). A first-parent `git bisect` between the two also skipped most steps, because intermediate commits do not build or write a report. #3720 owns the repair and the isolation; #3700 stays blocked on it. The sidecar is not the cause (rule 7 cold start about 30 ms, ten idle Agents about 3 MB extra). The macOS write-path figures belong to #3696, whose owner direction records the 16 ms delay as fixed.

## macOS baseline

Recorded on one Apple M4 Max laptop (16 cores); the key is the OS only, so the numbers are valid for that machine class: `macos-latest` in CI or a slower Mac would be judged against an M4 Max and needs its own baseline before it is gated. The machine was in ordinary use, not idle (a developer's laptop is never idle). Its load average was 4 to 7, which is 0.25 to 0.45 per core. The baseline is 9 invocations of 11 runs each. The spread of the 9 medians against their median:

| Metric | max / median | Gate on macOS |
|---|---|---|
| `cold_start_ms` | 1.07 | 10% + 0.3 ms, limit 300 ms (Linux: limit only) |
| `rss_extra_mb` | 1.01 | 10% + 0.05 MB, limit 150 MB |
| `rail_tree_40_p95_ms` | 1.07 | 10% + 0.001 ms |
| `terminal_write_p95_ms` | 1.09 | 10% + 0.001 ms |
| `write_to_output_p95_ms` | 1.09 | 10% + 0.001 ms, limit 16 ms |
| `ping_p95_ms` | 1.19 | none: 4 microseconds of jitter on 24 is 19% |
| `rail_tree_10_p95_ms` | 1.19 | none: same |
| keystroke p95 in WKWebView (`just perf-keystroke`) | 11 to 14 ms across 9 runs of 1000 keys | limit 16 ms only: it is whole milliseconds and a frame is 16.7 ms, so 10% is under one tick |

macOS cold start is stable (1.07) where the Linux VM's was not (3.08), so it is gated here.

**Load changes latency far more than 10%.** The same 7 metrics with 8 busy loops running (load 7 to 14): cold start x1.4, `ping` x2.1, `write_to_output` x2.2, `rail.tree` 10 Groups x8, 40 Groups x1.0 to 1.9 and widely spread; `rss_extra_mb` did not move (3.10 against a median of 3.14, committed as 3.1 because clippy rejects a literal that looks like pi). So a latency miss on a busy machine proves nothing, and a memory miss proves a lot.

**Observed false fail.** A `just perf` at load 7.5 (the baseline's loads were 0.25 to 0.45 per core, above) put `write_to_output_p95_ms` at 0.064 ms against a ceiling of 0.063 ms and failed; nothing had regressed. Each time metric with a baseline carries a `max_load_per_cpu` per OS in `budgets.json` (R13, `scenarios/perf.md`); above it, that metric's regression test is skipped, its `limit` still applies, and the run prints why. `target/perf.json` carries a `skipped` key (one entry per skipped metric, empty when none), and whenever at least one test was skipped the run's last printed line is `skipped <n> regression tests: load <x> per cpu`.

**A latency miss on macOS is rerun on a quiet machine before it is believed; a memory miss is believed at any load.**

**Real regression or noise.** Look at the load printed for each run first. A real regression moves the median above the ceiling in three reruns at the load of the baseline (up to about 0.45 per core) and does not track the load; noise falls back under the ceiling when the machine quiets, and the miss and the load rise together. A memory miss is real whatever the load. Re-baseline only with a PR that says why (copy the medians from `target/perf.json`).

## Keystroke-to-render in WKWebView

> **This opens a window over your screen and types into it.** The window is always on top and on every Space for about 40 s per run (three runs by default), and it never takes the keyboard focus on purpose, but it covers whatever you are doing. `just perf-keystroke` and `perf-keystroke` refuse unless `ROUNDUP_ALLOW_WINDOW=1` is set, and it is set only by someone the user asked to run it (R12). No agent, Reviewer, CI job, boxd VM or `just check` runs it.

What the number proves: keydown to the first xterm render that shows the echo, in the real WKWebView on macOS, with the renderer and mean frame time recorded. What it does not: it is not a CI gate and not a Linux number, it excludes the keyboard, OS event path and display scan-out, `performance.now()` is a 1 ms bound, and p95 in whole milliseconds makes a 10% test move in 1 ms steps, so its budget is the 16 ms limit only. The cheap headless bound is the Daemon-side `write_to_output_p95_ms` limit of 16 ms in `just perf` (R8).

`just perf-keystroke` (macOS only) builds the App with the keystroke probe (`VITE_ROUNDUP_PERF`, K1) into `target/perf-app`, opens its window once per run, lets the webview type 1000 keys into a raw `cat` Terminal and reads the result back (K3, R11). The report carries the renderer (`webgl` or `dom`) and the mean frame time.

R21 judges each run separately: p95 must be strictly below 16 ms, with zero
dropped keys, including the 100 warm-up keys. Warm-up latency samples remain
excluded. `target/perf-keystroke.json` retains the complete `reports` array and
per-run `loads`, including failures, and is updated after each run. A latency
or drop miss exits 1; an invalid observation exits 2. The runner uses temporary
App, webview and Daemon data even when the caller has data overrides. Its
Workstream uses the fake vendor executable, never the user's real `claude`.

What can go wrong:
- **A covered or napped window stops drawing.** macOS stops rAF and timers for a window other windows cover and naps an App that is not frontmost: measured here, with the window behind a full-screen Chrome, rendering stopped entirely and every key timed out. The perf build's window is therefore always on top and on every Space (a `TAURI_CONFIG` override in the recipe, so normal builds are unchanged), and the runner passes `-NSAppSleepDisabled YES`. The window stays over whatever you are doing for about 40 s per run, three runs. A mean frame time above 25 ms exits 2; its report is retained. Any dropped key fails under R21.
- **Low Power Mode.** It halves the webview's frame rate, so a run in it measures the power setting, not the App; run on AC power with Low Power Mode off.
- **Load.** Run it with the laptop idle; the load average is printed per run.
- **Windows over a headless Chromium number.** Chromium with the same xterm measured p95 under 5 ms; WKWebView on an M4 Max in the earlier spike measured 11 to 15 ms. The difference is the frame wait, so the budget is only meaningful here.

### R20 observations retained from #3550

These are the work order's observations, before the repair, on AC power on a
16-core Mac. The raw reports, OS/app versions, power-mode confirmation and
per-hop attribution were not supplied. They cannot establish the dominant hop
or replace R20's quiet-machine observations. No after-repair performance is
claimed by the observer-safety changes.

| Method / workload | roundup p50 / p95 (ms) | Solo p50 / p95 (ms) | Samples / load |
| --- | --- | --- | --- |
| WKWebView, raw `cat` | 30 / 35 | — | 1000 keys × 3; 13–25 drops/run; load 6–9 |
| Daemon `write_to_output` | — / 20.8 | — | `just perf`; load 6–9 |
| Outside key-to-pixel, idle zsh | 36 / 45 | — | 60 samples × 2; load 6–9 |
| Outside, idle zsh comparison | 40 / 50 | 39 / 48 | one run set; load ~7 |
| Outside, real `claude`, no Enter | 39–41 / 48 | 39.5 / 49–53 | one run set; load ~7 |
| Outside, typing beside `yes` | 101–106 / 129–136 | 33 / 38.5 | load 5.5–6.8 |
| Outside, switch two Terminals | 70–71 / 80–81 | 75 / 81–82 | load 5.5–6.8 |
| Outside, scroll 10k lines | 20–23 / 26–29 | 26–28 / 37 | load 5.5–6.8 |

The flooding observation had 100–120% CPU each in the webview, Daemon and host
App, versus about 30% for Solo's flooding Terminal. The proposed hidden-screen
explanation remains unproven. The native observations below use
`ROUNDUP_ALLOW_WINDOW=1 just perf-keystroke --out target/perf-keystroke-before.json`.
The [retained reports](perf-keystroke-mac-observations.json) include the owner's
complete comments, console output, source links and JSON, including the failures.


The recorder repairs in #3694 and #3717 merged before a further native run at
`01dacd91ce7b1e5ffd874202df4ada323e114d5c`. The command above exited 2: all
three reports contained `{"probe":"roundup-keystroke-probe","error":"no key was echoed and rendered after the warm-up"}`.
The machine was an M4 Max MacBook Pro, 16 CPUs, macOS 26.0.1, AC power at
100% charge, Low Power Mode off, unlocked, with the owner not touching the
window. Per-run 1-minute loads were 10.69091796875, 2.76953125 and
2.396484375 (0.67, 0.17 and 0.15 per CPU); the first run was busy after the
release build. There are no latency, renderer, frame-time or timer-resolution
measurements from these failed runs. Full console output and the saved report
remain on [#3550](https://github.com/saadshahd/roundup/issues/3550).

#3748 binds typing to the Terminal that printed `ready`, which already owns
report delivery. The old document-wide lookup could select an Agent's earlier
xterm textarea. A no-window regression test demonstrates that case with two
inputs and asserts that only the ready shell receives the key. The owner's
repeat at `7ce598e5a` still failed all three runs with the same warm-up error
(exit 2; [comment](https://github.com/saadshahd/roundup/issues/3550#issuecomment-6387912547)).

### Successful idle observation on #3755 (2026-10-24)

At `853f9440b8269b943dab07c9fa1dae79d060a204`, based on `59cff55ca`, the
owner ran the command above on an M4 Max MacBook Pro, 16 CPUs, macOS 26.0.1,
AC power, Low Power Mode off. Exit: 0. Each run reports 1000 measured keys,
zero drops, WebGL, 1 ms timer resolution and device pixel ratio 2.

| Run | 1-minute load / per CPU | p50 / p95 / p99 (ms) | Mean frame (ms) |
| --- | --- | --- | --- |
| 1 | 5.7705 / 0.361 | 6 / 10 / 12 | 8.3220 |
| 2 | 4.4482 / 0.278 | 6 / 10 / 11 | 8.3559 |
| 3 | 3.0508 / 0.191 | 6 / 10 / 15 | 8.3390 |

All three are below R20's 0.45 load-per-CPU threshold. The owner identified
an observer failure: the Workstream's Agent Terminal appeared about 200 ms
after `ready`, took focus and folded the shell bubble. xterm's
IntersectionObserver then paused rendering; echoes were parsed but never
rendered. #3755 waits for that Agent Terminal before opening the shell Terminal.
This repairs the recorder's visibility, not a measured production latency hop.
The successful reports came from that open PR; they do not establish results
for an unmodified `main` build.

The largest reported hop is `parsed_to_frame`, p50 4 ms and p95 7 ms in all
three runs. `dispatch_to_echo_event` is next, p50 2 / 1 / 2 ms and p95
3 / 3 / 4 ms. `write_reply` overlaps that chain (p95 4 ms in each run).
The reported uncertainty is 1 ms timer resolution, not cross-clock calibration.
These coarse webview spans cannot identify the largest of all R20's required
hops: base64, Daemon receipt, PTY write and echo read remain unsplit.

#3550 stays open. Missing acceptance includes full R20 attribution and calibrated
uncertainty, reproducible quiet-Mac Solo comparisons, a measured production
repair with before/after distributions, the shared Agent/shell input-path test,
and three passing runs while another Terminal floods output. The three idle
runs above meet the numerical idle budget only. No further Mac timing is
requested by this evidence update.


## X14 retained flood observation

`just perf --flood --out target/perf-flood.json` builds the release probe and runs
only the isolated real-Daemon flood observer. Ordinary `just perf` retains every
idle metric, budget, load/skip rule and qualification requirement above; flood
mode is a separate correctness observation, not latency or R16 qualification.
It opens no native window and supplies no R11/R12 keystroke evidence.

```text
real temporary Daemon → ten gated shells → exec yes
                      → independent RPC output drain → chunks per Terminal > Bus capacity
paused actual Bus receiver → two identified Rail markers → resume after producer shutdown
report → counters + received markers + payloads + structured lag + cleanup + outcome
```

The two Rail markers are emitted while the actual receiver is unread, before
releasing the ten programs; their count is strictly below the Daemon's reported
Bus capacity. Output is drained through the existing RPC client independently
of that receiver. Collection stops only after every Terminal exceeds capacity;
producers then stop before resuming the receiver to obtain a finite observation.
The report retains partial counters on failure, both marker identity lists,
Terminal payload count, lag error and missed-event counts, and verified stopped
Terminal count. Missing or incomplete counters and markers cannot pass.
`--flood-timeout 60` bounds startup/collection; shutdown and shutdown verification
have separate ten-second bounds. Only this temporary Project's processes and
socket/data are touched.

| Executable exit | Meaning |
| --- | --- |
| 0 | Complete counters, all markers in order, no payloads or lag, successful cleanup |
| 1 | Complete observation violates isolation or loses markers (expected on current transport) |
| 2 | Startup, collection timeout/error, cleanup or report-write failure; no valid acceptance |

A failing command through `just` may return the recipe's exit 1; the executable's
exit and JSON `flood.error` distinguish an observation error from detected lag.
Keep the JSON and command/exit with source and probe binary hashes, host,
toolchain, profile and load when recording an observation. Do not replace or
relax `budgets.json`. #1930 must migrate this output drain to selected streams
in its Contract PR, rerun this command and retain zero Rail lag; this observer
does not complete #1930 or X14's Terminal regression/journey acceptance.

### R20 hop attribution

Each report now carries `hops` and `hop_uncertainty_ms` (the 1 ms timer
resolution). All boundaries are read on the webview's one clock, so no
cross-process subtraction is involved; a report without every hop is invalid
(exit 2). Per key, from the same key's echo (unrelated output is ignored):

| Hop | From → to |
| --- | --- |
| `keydown_to_input` | `keydown` → xterm `onData` |
| `input_to_dispatch` | `onData` → `terminal.write` called; holds base64 (same tick, not separable) and the wait for the previous write's reply |
| `dispatch_to_echo_event` | `terminal.write` called → the echo reaches `emulator.write`; spans Tauri, the Daemon's write, the PTY, the echo read and the output channel |
| `echo_event_to_parsed` | `emulator.write` → xterm has parsed the echo |
| `parsed_to_frame` | parsed → the next render |
| `write_reply` | `terminal.write` called → its reply (overlaps the echo's trip) |

The Daemon-side boundaries inside `dispatch_to_echo_event` (Daemon received, PTY
write completed, echo read) are not yet stamped: they need the Daemon's clock
and a calibrated offset. R20 requires those boundaries even when that coarse
span is not largest.
The quiet-Mac reports above place `parsed_to_frame` first among the recorded
spans; full attribution and a production latency repair remain unclaimed.

The recorder accepts input, write replies, echo bytes and renders only from the
Terminal that printed the ready line. Previously every emulator shared those
marks: another Terminal printing `a` could supply the echo, and another
Terminal's render could complete a sample after the measured echo was parsed.
The deterministic `r20_another_terminal_cannot_supply_the_echo_or_finish_its_render`
and `r20_another_terminals_write_cannot_stamp_the_measured_input` tests reproduce
both forms of contamination without a native window. The retained Mac reports
predate this correction; this identifies a possible attribution error, not
evidence that another Terminal actually contaminated those runs. No reports are
discarded and no production latency improvement is claimed.

## macOS `cold_start_ms` and `rail_tree_40_p95_ms` (#3696)

Neither is a spawned `ps`: `resources.rs` runs only for `daemon.resources` and `scan.rs` only with a live Door, and `strace -f -e execve` of a fresh `rupd` shows one `execve`, so the lazy sidecar adds no spawn at start. Measured on one Linux machine at one load, `just perf` at the macOS baseline commit `02fd86775` against `main`: `rail_tree_40_p95_ms` 0.25 against 0.71 ms, `ping_p95_ms` 0.046 against 0.093 ms, `cold_start_ms` 14 to 16 against 24 ms. The Daemon itself got slower after the baseline, as the Mac's ratios show.

- `rail.tree`: `Shared::present` read each node's `finished` with `WHERE CAST(id AS TEXT)=?`, a full scan per node, and `load_nodes` parsed its SQL on every call. A tree read now takes one `finished_all` query and prepares both statements once; 0.71 became about 0.49 ms.
- `cold_start_ms`: a fresh Project replays 37 migrations into `project.db` (`storage::open`), 11 to 12 ms of DDL parse and `ALTER TABLE` schema checks; an existing Project starts in 10 ms against 24 ms for a fresh one. `PRAGMA synchronous=OFF` saves 1 ms and `legacy_alter_table` nothing. `build.rs` now migrates an empty database once at build time (`VACUUM INTO`), and `storage::open` copies that image when `project.db` does not exist, giving the two per-Project ids (`deliverable_project`, `reaction_project`) fresh values. `r3696c_a_fresh_project_db_equals_the_migrated_one_with_its_own_identity` holds the image equal to the migrated database. No budget changes; the Mac's `target/perf.json` is the observer.

- `rail_tree_40_p95_ms` after the batched read, per call inside the Daemon (release, 40 Workstreams, a Linux machine at load 1.2 per CPU): `Shared::tree` 90 µs (`load_nodes` 55 µs, of which the `closed_sources` subquery is 12 µs; presenting 8 µs), then `serde_json::to_value` of the 16.5 KB reply 90 µs, then the writer's `to_vec` 17 µs. `daemon.ping` is 19 µs in the Daemon. Rewriting the subquery uncorrelated and reading `kind` without an allocation moved `load_nodes` by under 3 µs, within noise, so neither is kept. The largest remaining share is the `Value` tree that every `Module::call` reply passes through; removing it (a pre-serialized reply that `dispatch` and the connection writer carry without a `Value`) changes the `Module` seam and is the next repair, not a budget change. This measurement needs no Mac; whether it brings the Mac's 0.338 ms under 0.149 ms is the Mac's `just perf` to say.

- Mac run at `6ce9103e2` (`cold_start_ms` 10.54, `rail_tree_40_p95_ms` 0.220, `rss_extra_mb` 3.81; the first two were 16.87 and 0.338 before the repairs above). Two more repairs, both measured without the Mac:
  - `rail.tree` and `rail.list` reach the socket as JSON text written from the typed nodes (`Module::call_reply`, `rpc::Reply::Json`, `connection::Frame::Json`), with no `serde_json::Value` tree between: `rail_tree_40_p95_ms` on a loaded Linux machine (load 3 to 5 per 4 CPUs) 0.62 against 0.54 ms through the same client, `rail_tree_10_p95_ms` 0.25 against 0.22. Every other method still returns a `Value`; `r3696c_a_rail_tree_reply_written_directly_equals_the_value_route` holds the two replies equal.
  - `cold_start_ms`: a fresh start made 12 `fsync` calls (`strace -f -e fsync`): three SQLite commits at about four each (the seeding `UPDATE`, and the switch of `project.db` to WAL mode). The image is now stored in WAL mode, so `storage::open` writes nothing, and the seeded copy is written with `synchronous=OFF` then synced once before it is linked: 1 `fsync`. A fresh start went from 16.1 to 17.1 ms down to 14.6 to 14.8 ms (15 starts each) on a filesystem where `fsync` is cheap; `fsync` costs more on APFS, so the Mac's gain should be larger. The remaining start is CPU: about 0.6 ms for each of the 17 stores opening `project.db`, 2.4 ms for `Agents::open`.
  - `rss_extra_mb`: ten Terminals grow the Daemon by 2.85 MB of anonymous memory on Linux (about 285 KB each: two `vt100` grids, the output ring, two threads). No cause of the Mac's rise from 3.1 to 3.81 is established, and no change is claimed. The Mac's next `target/perf.json` is the observer.


- Mac A/B at `bca7dc582` against the baseline `02fd86775` (9 runs each, one session, #3696): `cold_start_ms` 16.51 against 5.91 ms, `rail_tree_40_p95_ms` 0.1926 against 0.1237, `rss_extra_mb` 3.50 against 3.11, `ping_p95_ms` 0.0411 against 0.0193; the baseline reproduced its recorded numbers. Every request, `daemon.ping` included, costs about 22 µs more than at the baseline, so part of `rail.tree`'s 69 µs is the shared request path and not the read. The Linux attribution of a fresh start (release, 4 CPUs at load 4): `rupd` is 7.3 ms from `open_stores` to the socket, `storage::open` 2.5 of it (seed 1.7, WAL switch 0.4) and 17 `project.db` connections about 0.4 ms each. Tried and not kept: opening the independent stores on threads made a start 2 ms slower; dropping the per-store `PRAGMA journal_mode=WAL`, or reading it first, only moves the 0.06 ms of WAL setup per store to its first query. Kept: the fresh `project.db` image takes its two ids by replacing placeholder bytes in the copy (`storage::reidentified`), so a start opens no connection, `-wal` or `-shm` to seed; a fresh start went from 8.1 to 7.9 ms (median of 40 starts, each side alternating), more where file creation costs more. The Mac's `target/perf.json` stays the observer; the rest of the 10.6 ms is not attributed.

- Per-phase cold start (#3696): `rupd` now ends its `serving` line with `(open X ms, bound Y ms)`, measured from `main`: `open` is `Daemon::open_project` (stores, plugins), `bound` the socket. The gap between `cold_start_ms` and `bound` is process spawn, runtime start and the first ping round trip. On the Mac, run `RUPD_SOCKET=/tmp/x.sock ROUNDUP_DATA_DIR=$(mktemp -d) target/release/rupd $(mktemp -d)` at the baseline `02fd86775`'s successor commits and `main` to attribute the 10.6 ms.

- Per-request cost (#3696), found by profiling `daemon.ping` in a release `rupd` on Linux (`perf record`, 150 000 pings), no Mac needed. Each request paid for work no request needs: `Daemon::start_pushes` built a `Reader` for the waker and again for the evaluator (a module-map copy plus a `getenv`) before checking there were events to hand them, and `Auto::start` built one more before checking it had already started; and `daemon.ping` built its `contract.methods` (about 300 names, sorted, then one `String` each) as a `Value` every call. The readers are now built only when there are events, and `daemon.ping` writes its reply as JSON text with the Contract part rendered once (`builtin::ping_json`, `r3696c_a_ping_written_directly_equals_the_value_route`). A ping round trip from a plain client went from 43 to 28 us (baseline `02fd86775`: 20 us; its reply was 14 bytes, today's is about 8 KB, which accounts for the rest and for `ping_p95_ms` having no baseline). `rail.tree` at 40 Workstreams shows no change beyond noise on Linux (130 us either side through a Python client), so the Mac's 0.069 ms gap there is the read itself, which has grown with the fields the Rail now loads; `Shared::tree` is 90 us of it above.
- Binary size, a hypothesis only the Mac can test: release `rupd` is 34.3 MB against 5.6 MB at `02fd86775` (`reqwest` with `rustls`, `axum`, `notify`, `regex`, bundled SQLite). On Linux a start costs 1.4 ms more (10.3 against 11.7 ms to the first ping); macOS pays at exec for every page it validates against the code signature and for the rebase of every pointer in the data segments, so size may be most of the Mac's 10.6 ms. The `serving` line's `bound` time against `cold_start_ms` separates that (spawn and load) from `open` (stores and plugins).

- Cold start's remaining Mac cost (#3696), attributed from the Mac runs already on this Issue: `cold_start_ms` was 10.54 ms at `6ce9103e2`, whose fresh start paid SQLite's 12 plain `fsync` calls, and 15.65 to 16.51 ms at `bca7dc582`, after `fdfad6b6c` replaced them with one `std::fs::File::sync_all` of the seeded image. Rust's `sync_all` is `fcntl(F_FULLFSYNC)` on macOS, which waits for the drive to flush its cache and costs several ms on APFS, more than the 12 plain `fsync` calls it replaced; on Linux it is `fsync`, so the Linux measurement could not show it. `seed_fresh` now calls `rustix::fs::fsync`, the plain `fsync` SQLite itself uses, so no `F_FULLFSYNC` runs at start. The Mac's `target/perf.json` is the observer; the ping and `rail.tree` per-request gap and `rss_extra_mb` are not attributed by this change.


- Retained Terminal memory (#3696): `460203cfa` added cell source annotations for X14. On this Linux x86_64 runner, ten 80×24 idle `vt100` parsers retain 949,600 heap bytes, including 921,600 bytes of 48-byte cells. The pre-annotation cell was 32 bytes: 307,200 extra bytes for ten grids, independent of machine load. These annotations are used by Output replay; the live screen retains the same cell representation even when it has no annotations. This explains an allocation increase, not the entirety of the Mac's measured RSS change (3.109 to 3.500 MB); RSS also includes allocator residency and the added output channels. R3696c now stores each opaque source value as eight bytes and its presence in the unused length bit: a cell is 40 bytes, preserving `None`, zero and `u64::MAX`, all text capacity, and screen/history dimensions. `r3696c_ten_idle_terminal_grids_retain_under_850_kb` measured 796,000 retained heap bytes after the repair, saving 153,600 bytes per ten idle grids. It runs without a Daemon or clock. Existing edit/replay tests cover source movement and invalidation; the literal `Cell == 48` test assertion was removed because the library's compile-time size assertion already catches representation growth and the new observer checks actual parser allocation. No Mac RSS pass, baseline rebase or complete R3696c pass is claimed. The later same-M4-Max `just perf` and per-phase startup attribution remain required.

### Retaining startup attribution (#3696)

`just perf` keeps each cold-start Daemon's stderr beside its report: with the default output,
`target/perf.startup-1.log` through `target/perf.startup-11.log`; with
`--out target/mac/current-1.json`, `target/mac/current-1.startup-1.log` through
`target/mac/current-1.startup-11.log`. The CLI prints the path before each run,
and an observer failure retains the log even when no JSON report can be written.
The parent opens the file before timing cold start and passes its descriptor directly
to the Daemon, with no pipe collector or extra process. This replaces `/dev/null`;
metric calculations and `budgets.json` do not change.

The `serving` line's cumulative times are measured from entry to async `main`:
`open` covers `Daemon::open_project`, and `bound - open` covers socket setup.
`cold_start_ms - bound` includes process creation/loading, Tokio runtime startup,
readiness polling and the first ping; it is **not** a pure process-load measurement.
Plugin materialization is deferred to `Launcher::launch_on` at Agent launch, so no
plugin materialization occurs in this cold start. The separately timed
`cold_plugin_materialization_ms` is not part of `cold_start_ms`.

For the next same-M4-Max observation, retain these logs with `perf.json`, commit,
load and exit code. The last posted A/B (`bca7dc582` against `02fd86775`) predates
the plain-fsync repair and #3745's retained Terminal allocation repair. These logs
supply the missing in-process evidence; they do not establish a Mac pass or attribute
the entire remaining startup gap. The nine alternating same-session invocations,
commit-by-commit attribution, unchanged-code false-fail bar and separate later main
pass required by #3696 remain outstanding. No baseline rebase is justified here.

### Rail reply allocation (#3696)

The latest M4 Max A/B on #3696 (`59cff55cae` against `02fd86775`, nine
alternating invocations each) reproduces the baseline and still fails current
cold start (10.64 ms), Rail 40 (0.186 ms) and RSS growth (3.922 MB). The startup
logs give median open/bound times of 5.32/5.38 ms; neither these logs nor the
following Linux measurement attributes the entire remaining startup or RSS gap.

`e03234712` removes temporary allocations from Rail reads: rusqlite's borrowed
`ValueRef` supplies kind and Order text, traversal moves loaded nodes instead of
cloning them, and presentation formats a finish count only in the Status that
uses it. No result is cached. Through `Module::call_reply`, the ordinary
forty-Workstream reply allocates 290 times / 176,266 bytes, against 530 times /
181,047 bytes before the repair. `r3696c_forty_workstreams_avoid_temporary_node_copies`
uses the existing `stats_alloc` observer, with no Daemon, process or clock; its
400-allocation ceiling fails on the original path. Existing Rail ordering,
Order and Status tests cover the returned contents.

A separate release probe on Linux (four available CPUs, AMD EPYC 9V45) constructs
40 Workstreams, warms 100 reads, then times seven batches of 10,000 `Rail::tree`
reads. Two alternating before/after invocations, with builds and tests stopped,
gave median batch means of 38.35 us before (37.22–41.07) and 32.44 us after
(31.85–33.10), about 15% less construction time. This probe excludes live
presentation, JSON and transport; it is not the end-to-end Mac metric. The final
presentation change is included in the allocation observer, not this timing.
The saved allocations are temporary request work, not an explanation of idle
RSS. Budgets remain unchanged. R3696c's same-M4-Max full gate, remaining
attribution, false-fail evidence and separate later main pass remain open.

### Fresh-image search cost (#3696)

`reidentified` searches a 507,904-byte migrated image twice to replace the two
Project identities. `2a259871a` introduced a scalar comparison at every byte;
this startup work is independent of SQLite, process sampling and the listing
sidecar. `memchr::memmem::find_iter` replaces that scan, searching the immutable
image before copying replacement bytes. The image, synchronization, migration
fallback and budgets stay unchanged.

Linux x86_64, Intel Xeon Platinum 8573C, four available CPUs, rustc 1.99.0:
extract `reidentified`, `FRESH` and `IDENTITIES` from `storage.rs`, replace only
the RNG call with `random.fill(0x71)`, and compile with `rustc -O --edition 2024`.
Each invocation times seven batches of 1,000 copies, black-boxing every returned
image. The after probe links the existing release `memchr` rlib. With builds
and checks stopped, three alternating before/after invocations gave these
median batch means (ms per copy):

| Pair | Scalar scan | Library search |
|---|---:|---:|
| 1 | 0.68837 | 0.04957 |
| 2 | 0.68939 | 0.04981 |
| 3 | 0.69635 | 0.05155 |

The repair removes about 0.64 ms of Linux image preparation. It excludes RNG,
file writing, fsync, database opening and process startup; it is neither a full
cold-start timing nor Mac attribution. The existing
`r3696c_a_fresh_project_db_equals_the_migrated_one_with_its_own_identity` checks
schema/data equivalence, different Project identities and removal of every
placeholder. No new clock-sensitive test is added.

R3696c's full Mac gate, remaining startup/RSS attribution, false-fail evidence and
separate later main pass remain open. No baseline rebase is justified here.


### Idle Terminal event rings (#3696)

The per-Terminal Tokio broadcast channel reserves 1,024 event slots at spawn,
including for a quiet program that has printed nothing. On Linux x86_64,
`EventData` is 88 bytes; each ring stored the complete application event enum
inline even though Terminals publish only titles and exits. Storing
`Arc<EventData>` in the same channel removes 80 bytes per slot, or 819,200 bytes
across ten queues. Capacity, lag reporting, ordering and exit delivery are still
Tokio's; no replacement channel or reduced backlog is introduced.

The allocation observer
`r3696c_ten_quiet_terminals_retain_under_800_kb` uses `stats_alloc` around ten
real `Terminals::spawn` calls with silent `/bin/cat` programs. Before this repair
it retained 1,311,686 heap bytes and failed the 800,000-byte cap; after it retains
492,346 bytes and passes (819,340 bytes less in this observation).
The programs receive no input, so screen parsers remain pending: this measures
the queues and spawn bookkeeping separately from the existing no-process parser
allocation observer. Each program is killed before the assertion, even on a
budget miss. No Daemon, socket or clock assertion is involved; the observer runs
in its own test binary so other tests do not contaminate its global allocator.

This identifies avoidable retained queue memory, not the origin of every byte
in the Mac's RSS rise. The saving is heap allocation on Linux, not a measured
Mac RSS saving. No baseline changes or full R3696c pass are claimed. Startup,
Rail, same-M4-Max observations, unchanged-code false-fail evidence and the
separate later main pass remain outstanding.
