# Tauri 2 webview for the UI

Status: Pending evidence

## Decision

Tauri 2 shell with a web UI.

## Considered

Native Rust renderer (GPUI/egui/wgpu terminal), Electron.

## Why

Small binary, Rust core shared in-process or via socket, web UI makes Extension panels and the Tufte layout cheap. Risk: webview terminal latency vs the 16 ms keystroke budget.

## Notes

Decision rests on `spikes/tauri-latency/REPORT.md`. That measures in-webview latency (keydown to xterm render callback), not input-to-photon. p95 >= 16 ms means this ADR is rejected and the UI stack changes before Phase 1.
