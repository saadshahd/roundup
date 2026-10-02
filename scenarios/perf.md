# Perf gate

`just perf` measures a fresh Daemon `runs` times (default 11) and compares the median of each metric with `crates/perf/budgets.json`. A metric fails when it is above its `limit` (rule 7's absolute budget), or above its OS's `baseline` by more than 10% plus the metric's `noise` allowance. A metric with neither is reported only. On Linux two metrics (`cold_start_ms`, `rail_tree_10_p95_ms`) vary too much between identical runs for a 10% test and carry a limit only; on macOS `cold_start_ms` is stable and gated, and `ping_p95_ms` and `rail_tree_10_p95_ms` have no baseline: a gross tripwire, not rule 7's regression test (`docs/perf.md`). The result, the per-run values and each run's load average are written to `target/perf.json` and printed, so a noisy machine is visible. The keystroke-to-render budget is measured by `just perf-keystroke` in the real App's webview (K1 to K3, R11); its budget file is `crates/perf/keystroke-budgets.json`, separate because `just perf` fails on a budget it did not measure (R7).

**R1 within budget.** Given a baseline of 100 and no noise allowance, then a value of 109 passes and exactly 110 passes.

**R2 regression.** Given the same baseline, then a value of 111 fails as a regression from 100.

**R3 limit.** Given a limit of 300 and a baseline of 400 for the OS, then 350 fails as over the limit, whatever the baseline.

**R4 noise.** Given a baseline of 0.03 ms and a noise allowance of 0.3 ms, then 0.2 ms passes and 0.4 ms fails.

**R5 median.** Given the values `[9, 1000, 11]`, the median is 11; given `[1, 2, 3, 4]`, 2.5.

**R6 other platforms.** Given a baseline only for `linux`, then on `macos` only the limit applies.

**R7 no silent gate.** Given a budget for a metric the run did not measure, then the comparison is an error, not a pass.

**R8 the committed budgets.** Given `crates/perf/budgets.json`, then the limits are 300 ms cold start, 150 MB extra RSS and 16 ms write-to-output; every metric with a baseline has a regression ceiling within 15% of it; every metric without a baseline has a limit or, on macOS, none (`ping_p95_ms`, `rail_tree_10_p95_ms`); and the committed baselines are exactly those recorded (R8 pins each value, so deleting or doubling one fails).

**R9 boundaries.** A value exactly at a limit passes and the next representable value above it fails; the same holds for the regression ceiling, including 110.00000000000001 against a baseline of 100.

**R10 exit codes.** `perf` exits 2 when the budgets file or the `rupd` binary is missing, naming it.

## Keystroke-to-render in the App (`apps/desktop`, run by `crates/perf`)

**K1 opt-in.** Given a normal build, then its bundle holds no trace of the keystroke probe (the string `roundup-keystroke-probe` is absent). A build made with `VITE_ROUNDUP_PERF` carries it. The probe is chosen when the bundle is built, so a normal run pays nothing and no query or flag can switch it on.

**K2 a sample.** The time of a key is from its `keydown` to the first render after the emulator has parsed the typed character itself as output. A render before that echo does not count, and neither does other output of the program. A key whose echo is not rendered within 500 ms is dropped and counted. The first 100 keys are discarded; the rest give `n`, `dropped`, p50, p95, p99 and max by nearest rank.

**K3 the run.** Given the App built with the probe on a Project, when the webview mounts, it presses `+ terminal`, selects the new Terminal, waits for the program's `ready` line, then types 1000 keys one at a time, 20 to 40 ms apart, and writes one line `PERF <json>` through that Terminal. The JSON carries the K2 numbers, the renderer used (`webgl` or `dom`), the mean frame time, the timer resolution (1 ms in WKWebView), the pixel ratio and the user agent. A failure in any step is written as `{"error": ...}` the same way. Its observer is the real run (`just perf-keystroke`), not a test: the webview is the thing under test and no fake has one.

**R11 what the runner accepts.** `perf-keystroke` starts the App with `SHELL` set to a script that makes its tty raw, prints `ready` and runs `cat`, reads the `PERF` line from `terminal.output` on the Daemon's socket, and refuses (exit 2) a report that proves nothing: the webview's own error, a mean frame time above 25 ms (the window was covered or napped; the perf build's window is always on top and on every Space so that it is not), or more than 5% of keys dropped. Otherwise `keystroke_p95_ms` (median over `--runs`, default 3) is compared with `keystroke-budgets.json`: limit 16 ms.

**R12 the window needs a yes.** `perf-keystroke` and `just perf-keystroke` open a window over the user's screen, kept on top for about 40 s per run, and type into it. Without `ROUNDUP_ALLOW_WINDOW=1` in the environment they print why and exit 2 before building, starting or opening anything. The variable is set only by someone the user asked to run it; no agent, Reviewer, CI job, boxd VM or `just check` sets it.
