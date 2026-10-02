# Perf gate

`just perf` measures a fresh Daemon `runs` times (default 11) and compares the median of each metric with `crates/perf/budgets.json`. A metric fails when it is above its `limit` (rule 7's absolute budget), or above its OS's `baseline` by more than 10% plus the metric's `noise` allowance. A metric with neither is reported only. On Linux two metrics (`cold_start_ms`, `rail_tree_10_p95_ms`) vary too much between identical runs for a 10% test and carry a limit only; on macOS `cold_start_ms` is stable and gated, and `ping_p95_ms` and `rail_tree_10_p95_ms` have no baseline: a gross tripwire, not rule 7's regression test (`docs/perf.md`). The result, the per-run values and each run's load average are written to `target/perf.json` and printed, so a noisy machine is visible. The keystroke-to-render budget needs the webview and is a manual step (`docs/perf.md`).

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
