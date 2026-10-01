# SolidJS + Vite + xterm.js (WebGL addon)

Status: Accepted

## Decision

Solid for UI, xterm.js with the WebGL addon for terminals.

## Considered

React, Svelte, Leptos.

## Why

Fine-grained reactivity without a vDOM suits a rail that re-renders on every Status change. xterm.js is the mature webview terminal.

## Notes

The latency spike chose the WebGL addon (p95 11-13 ms vs 14-15 ms DOM); see ADR 0001.
