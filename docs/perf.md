# Perf

Rule 7's budgets and what measures each. Numbers from a busy laptop or a Linux VM bound a tail; they are not the macOS gate.

| Budget | Measured by | Where |
|---|---|---|
| Daemon cold start < 300 ms | `just perf` (`cold_start_ms`: spawn `rupd` to its first `daemon.ping` reply) | any machine; the App's cold start to a visible window is manual |
| 10 idle Agents < 150 MB extra RSS | `just perf` (`rss_extra_mb`: the Daemon's growth with ten login-shell Terminals) | any machine; the Agents' own processes are manual |
| Keystroke-to-render < 16 ms p95 | manual, below | the macOS App only |
| Regression above 10% fails | `just perf` against `crates/perf/budgets.json` | same OS as the baseline |

`just perf` runs a fresh Daemon 11 times, takes each metric's median, prints the range and each run's load average, writes `target/perf.json` and exits 1 on a miss. Run it on a quiet machine: load is printed so a noisy run can be thrown away. A metric fails above its `limit`, or above its OS's `baseline` by more than 10% plus its `noise` allowance.

## What is enforced, and what is not

Rule 7's 10% regression test is enforced for five metrics on Linux only, and is a gross tripwire for two. Measured on an isolated 2-vCPU boxd VM (AMD EPYC, load 2 to 4 from the runs themselves): 9 invocations of 11 runs each, the spread of the 9 medians against their median was

| Metric | max / median | min / median | Gate |
|---|---|---|---|
| `rss_extra_mb` | 1.05 | 0.93 | 10% + 0.05 MB |
| `ping_p95_ms` | 1.13 | 0.96 | 10% + 0.001 ms |
| `rail_tree_40_p95_ms` | 1.06 | 0.97 | 10% + 0.001 ms |
| `terminal_write_p95_ms` | 1.04 | 0.88 | 10% + 0.001 ms |
| `write_to_output_p95_ms` | 1.09 | 0.96 | 10% + 0.001 ms, limit 16 ms |
| `cold_start_ms` | 3.08 | 0.57 | limit 300 ms only: its median varies 3x between identical invocations, so no 10% test can hold |
| `rail_tree_10_p95_ms` | 1.66 | 0.51 | limit 2 ms only (about 8x its median): same reason |

`noise` is an absolute floor for the clock's resolution, not slack: it is 0.05 MB on a 1.6 MB metric and 1 microsecond on latencies of 25 to 265 microseconds. The worst observed ratio (1.13, `ping`) is a 0.5 microsecond overshoot that the floor covers; with only 9 invocations, one false fail in a few dozen runs is expected, so rerun before believing a single fail. The `linux` baselines are the medians of those 9 invocations and are valid for that VM class only. There is no `macos` baseline because the laptop was never quiet; on macOS only the limits apply until someone records one on an idle machine (the `limit`s hold anywhere). To accept a new baseline, copy the medians from `target/perf.json` into `budgets.json` under the OS name in the same PR that explains why. See `scenarios/perf.md` (R1 to R10).

## Keystroke-to-render in WKWebView

> **This opens a window over your screen and types into it.** The window is always on top and on every Space for about 40 s per run (three runs by default), and it never takes the keyboard focus on purpose, but it covers whatever you are doing. `just perf-keystroke` and `perf-keystroke` refuse unless `ROUNDUP_ALLOW_WINDOW=1` is set, and it is set only by someone the user asked to run it (R12). No agent, Reviewer, CI job, boxd VM or `just check` runs it.

What the number proves: keydown to the first xterm render that shows the echo, in the real WKWebView on macOS, with the renderer and mean frame time recorded. What it does not: it is not a CI gate and not a Linux number, it excludes the keyboard, OS event path and display scan-out, `performance.now()` is a 1 ms bound, and p95 in whole milliseconds makes a 10% test move in 1 ms steps, so its budget is the 16 ms limit only. The cheap headless bound is the Daemon-side `write_to_output_p95_ms` limit of 16 ms in `just perf` (R8).

`just perf-keystroke` (macOS only) builds the App with the keystroke probe (`VITE_ROUNDUP_PERF`, K1) into `target/perf-app`, opens its window once per run, lets the webview type 1000 keys into a raw `cat` Terminal and reads the result back (K3, R11). The report carries the renderer (`webgl` or `dom`) and the mean frame time.

What can go wrong:
- **A covered or napped window stops drawing.** macOS stops rAF and timers for a window other windows cover and naps an App that is not frontmost: measured here, with the window behind a full-screen Chrome, rendering stopped entirely and every key timed out. The perf build's window is therefore always on top and on every Space (a `TAURI_CONFIG` override in the recipe, so normal builds are unchanged), and the runner passes `-NSAppSleepDisabled YES`. The window stays over whatever you are doing for about 40 s per run, three runs. A throttled run is refused, not reported: mean frame time above 25 ms or more than 5% dropped keys exits 2.
- **Load.** Run it with the laptop idle; the load average is printed per run.
- **Windows over a headless Chromium number.** Chromium with the same xterm measured p95 under 5 ms; WKWebView on an M4 Max in the earlier spike measured 11 to 15 ms. The difference is the frame wait, so the budget is only meaningful here.

