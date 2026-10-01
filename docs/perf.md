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

## Keystroke-to-render in WKWebView (manual)

Needs the real App on the laptop, a visible, focused window, and a quiet machine. The in-webview number is keydown to xterm's `onRender`, not input-to-photon. Not yet possible: the webview has no probe. The UI module must first add one (a `?perf` switch in `apps/desktop/src/terminal/emulator.ts` that stamps each key's `keydown` time and the next `onRender` into `window.__keystrokes`); until then no number can be read from the real App.

With the probe in place:

1. `just app-release <project>` (release Rust, embedded webview files, no dev server).
2. Select a Terminal running `cat`, so each typed byte echoes through the Daemon and the PTY.
3. In Safari, Develop > this Mac > roundup > Web Inspector (needs the `devtools` feature on a release build; use `just app <project>` if it is off), then in the console run the driver: dispatch 1600 `keydown` events on the terminal's `textarea`, 20 to 40 ms apart, and discard the first 100.
4. Read p50, p95 and p99 of `window.__keystrokes` for the WebGL renderer, then reload with the DOM renderer. Budget: p95 < 16 ms. Repeat 4 times and report the spread and `uptime`.

`spikes/tauri-latency/REPORT.md` has the method and the one measured result (WebGL p95 11 to 13 ms, DOM 14 to 15 ms on an M4 Max).
