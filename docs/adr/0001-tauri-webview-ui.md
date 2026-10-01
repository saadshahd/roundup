# Tauri 2 webview for the UI

Status: Accepted (evidence: `spikes/tauri-latency/REPORT.md`)

## Decision

Tauri 2 shell with a web UI.

## Considered

Native Rust renderer (GPUI/egui/wgpu terminal), Electron.

## Why

Small binary, Rust core shared in-process or via socket, web UI makes Extension panels and the Tufte layout cheap. Risk: webview terminal latency vs the 16 ms keystroke budget.

## Notes

Spike result (M4 Max, macOS 26.0.1, Tauri 2.11.6 on WKWebView, xterm.js 6.0.0, ~3000 samples per mode, keydown to xterm `onRender`):

| | p50 | p95 | p99 |
|---|---|---|---|
| WebGL addon | 2 ms | 11 ms | 16 ms |
| DOM renderer | 3 ms | 14 ms | 17 ms |

p95 passes the 16 ms budget with WebGL; use the WebGL addon (DOM leaves little headroom). Tauri IPC costs about 1 ms p50 and 3 ms p95 and the PTY round trip is sub-millisecond, so the rest is frame quantisation (about 16.7 ms cadence), which sets the p99.

Caveats: this measures in-webview latency, not input-to-photon (add 1-2 frames). Synthetic, idle, one character at a time, `cat` echo, 1 ms timer resolution. It does not prove the budget under bursty agent output; the Phase 5 perf bench must measure that. The latest Tauri crates need rustc 1.90 and this machine has 1.89, so Phase 1 pins a toolchain.
