# Perf

Rule 7's budgets and what measures each. Numbers from a busy laptop or a Linux VM bound a tail; they are not the macOS gate.

| Budget | Measured by | Where |
|---|---|---|
| Daemon cold start < 300 ms | `just perf` (`cold_start_ms`: spawn `rupd` to its first `daemon.ping` reply) | any machine; the App's cold start to a visible window is manual |
| 10 idle Agents < 150 MB extra RSS | `just perf` (`rss_extra_mb`: the Daemon's growth with ten login-shell Terminals) | any machine; the Agents' own processes are manual |
| Keystroke-to-render < 16 ms p95 | manual, below | the macOS App only |
| Regression above 10% fails | `just perf` against `crates/perf/budgets.json` | same OS as the baseline |

`just perf` runs a fresh Daemon 7 times, takes each metric's median, prints the range and each run's load average, writes `target/perf.json` and exits 1 on a miss. Run it on a quiet machine: load is printed so a noisy run can be thrown away. A metric fails above its `limit`, or above its OS's `baseline` by more than 10% plus its `noise` allowance. `noise` is wide on purpose: on a 2-vCPU Linux VM (AMD EPYC, load about 5 from the run itself) the median cold start moved between 6.7 and 21 ms across four invocations of seven runs each, and a 10-Group `rail.tree` p95 between 0.25 and 1.1 ms, so a bare 10% would flake. The committed `linux` baselines are the median of those four invocations; there is no `macos` baseline yet because the laptop was never quiet, so on macOS only the limits apply until someone records one on an idle machine. To accept a new baseline, copy the medians from `target/perf.json` into `budgets.json` under the OS name in the same PR that explains why. See `scenarios/perf.md` (R1 to R7).

## Keystroke-to-render in WKWebView (manual)

Needs the real App on the laptop, a visible, focused window, and a quiet machine. The in-webview number is keydown to xterm's `onRender`, not input-to-photon. Not yet possible: the webview has no probe. The UI module must first add one (a `?perf` switch in `apps/desktop/src/terminal/emulator.ts` that stamps each key's `keydown` time and the next `onRender` into `window.__keystrokes`); until then no number can be read from the real App.

With the probe in place:

1. `just app-release <project>` (release Rust, embedded webview files, no dev server).
2. Select a Terminal running `cat`, so each typed byte echoes through the Daemon and the PTY.
3. In Safari, Develop > this Mac > roundup > Web Inspector (needs the `devtools` feature on a release build; use `just app <project>` if it is off), then in the console run the driver: dispatch 1600 `keydown` events on the terminal's `textarea`, 20 to 40 ms apart, and discard the first 100.
4. Read p50, p95 and p99 of `window.__keystrokes` for the WebGL renderer, then reload with the DOM renderer. Budget: p95 < 16 ms. Repeat 4 times and report the spread and `uptime`.

`spikes/tauri-latency/REPORT.md` has the method and the one measured result (WebGL p95 11 to 13 ms, DOM 14 to 15 ms on an M4 Max).
