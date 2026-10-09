# Tooling

Status: Accepted

## Decision

pnpm + Cargo workspace, Oxlint with anti-slop plugins, clippy `-D warnings`, `cargo nextest`, Vitest, Playwright against the webview. `just` arrives in Phase 1.

## Considered

Biome alone, make.

## Why

Anti-slop and the loop rules (jscpd, knip, cargo udeps) must be machine checks, not review opinions.

## Notes

Nothing is installed system-wide in Phase 0.
