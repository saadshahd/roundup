# Tauri 2 + xterm.js keystroke latency spike

Budget under test: keystroke-to-render p95 < 16 ms (in-webview, NOT input-to-photon).

## Machine
macOS 26.0.1 (25A362), Apple M4 Max, devicePixelRatio 2. Tauri 2.11.6 (WKWebView), xterm.js 6.0.0, @xterm/addon-webgl 0.19.0, portable-pty 0.9, release build, rustc 1.89 (lockfile resolved with MSRV fallback because latest tauri crates want rustc 1.90; no toolchain change made).

## Method
- Rust runs `sh -c "stty raw -echo; exec cat"` in a PTY. tty echo is off, so a byte only comes back if `cat` reads and rewrites it (real PTY round trip).
- Webview auto-driver: for each of 1600 keys (first 100 discarded as warmup; 1500 measured per run) it dispatches a synthetic `keydown` on xterm's textarea. xterm's own key handler emits `onData`, which calls `invoke("pty_write")`. Rust stamps entry, post-write, echo-read, pre-send; echo goes back over a `tauri::ipc::Channel` as raw bytes with the 4 timestamps as a header. JS then calls `term.write(bytes, cb)` and waits for the next `onRender`.
- One key in flight at a time, 20-40 ms random idle gap between keys (decorrelates from frame phase). Correlation is by FIFO (serial driver, 1 write -> 1 echo); samples with a missing Rust record (3 of 6000) were dropped.
- Runs: DOM renderer, WebGL, DOM again, WebGL again (2 x 1500 each; table pools both = ~3000 samples per mode).
- Clocks: JS `performance.timeOrigin + performance.now()` vs Rust `SystemTime` epoch. Cross-clock stages carry a constant skew, so the table reports IPC as the sum of both legs, where skew cancels.

## Results (ms, pooled over 2 runs per mode)

| Stage | DOM p50 | DOM p95 | DOM p99 | WebGL p50 | WebGL p95 | WebGL p99 |
|---|---|---|---|---|---|---|
| IPC in + IPC out (both legs, skew-free) | 0.97 | 2.97 | 3.97 | 0.97 | 2.97 | 3.97 |
| Rust dwell (cmd entry -> echo sent, incl. PTY write + cat + read) | 0.03 | 0.18 | 0.70 | 0.03 | 0.18 | 2.09 |
| keydown -> `xterm.write` callback (everything except frame wait) | 1.0 | 3.0 | 4.0 | 1.0 | 4.0 | 4.0 |
| write callback -> next `onRender` (frame wait + render) | 2.0 | 13.0 | 16.0 | 0.0 | 10.0 | 15.0 |
| **keydown -> `onRender` (total)** | **3.0** | **14.0** | **17.0** | **2.0** | **11.0** | **16.0** |

Per-run total p95 (keydown -> onRender): DOM 15 / 14, WebGL 11 / 13 (the four runs in order dom, webgl, dom2, webgl2: 15, 11, 14, 13). Max observed 19 ms. xterm parse of the echo: p50/p95 0 ms (below clock resolution). Raw samples are in `results.json`.

## Verdict vs 16 ms
- p95: WebGL 11-13 ms, DOM 14-15 ms. Both under 16 ms, WebGL with more headroom (~3-5 ms), DOM only ~1-2 ms.
- p99 is 16-17 ms in both modes, i.e. at/just over budget. The tail is frame quantisation: the render stage is dominated by waiting for the next rAF (~16.7 ms cadence), not by work.
- Non-render cost is small: IPC both legs ~1 ms p50 / ~3 ms p95, PTY round trip ~0.03 ms p50 (sub-ms), parse ~0. The p95 < 16 ms budget is consumed by the display frame, not by Tauri IPC or the PTY.
- Pass on p95 for this synthetic, idle, single-character workload. Not a proof the budget holds under load.

## Caveats (be honest)
1. In-webview only. Measures keydown -> `onRender` callback, not input-to-photon. Real photon latency adds keyboard/USB, event delivery to the webview, compositor, and display scan-out (typically another 1-2 frames).
2. `performance.now()` in WKWebView is quantised to 1 ms, so stage values below ~1 ms are unresolvable and percentiles are integer-ish. Cross-clock stages (ipc_in alone, ipc_out alone) show a ~1 ms skew (ipc_out p50 negative), which is why only the sum is reported.
3. `onRender` fires inside the render rAF; "render" here is when xterm issues its draw, not when pixels hit the screen. The WebGL draw is async to the GPU.
4. Synthetic `KeyboardEvent` dispatch skips the native key-event path into WKWebView and the IME/`textarea` input path. Window was visible and focused; a hidden/occluded window would throttle rAF.
5. One printable character per iteration with an idle terminal (80x24, nearly empty). No bursts, no heavy output, no concurrent agent output. Echo from `cat` is a best case for the PTY leg; a real shell/agent adds its own latency.
6. IPC `invoke` used a string payload; output used a raw-bytes Channel. Other IPC shapes were not compared.
7. Only 4 runs of 1500; DOM vs WebGL p95 difference (3 ms) is consistent across both pairs but run-to-run variance is ~1-2 ms. Display refresh rate was not pinned.
8. Count anomaly: each run showed one extra Rust->webview message (1601 vs 1600) in 3 of 4 runs; probably an initial/stray chunk. 3 samples dropped.

## Reproduce
`pnpm install && pnpm build && cd src-tauri && cargo build --release --features tauri/custom-protocol && ../src-tauri/target/release/tauri-latency` (needs a display; writes `results.json`, then exits). The `custom-protocol` feature is required when building without the Tauri CLI so the embedded `dist/` loads. Source: `src/main.ts`, `src-tauri/src/main.rs`.
