# SolidJS + Vite + xterm.js (WebGL addon)

Status: Accepted, subject to 0001

## Decision

Solid for UI, xterm.js with the WebGL addon for terminals.

## Considered

React, Svelte, Leptos.

## Why

Fine-grained reactivity without a vDOM suits a rail that re-renders on every Status change. xterm.js is the mature webview terminal.

## Notes

WebGL addon on/off is part of the latency spike.
