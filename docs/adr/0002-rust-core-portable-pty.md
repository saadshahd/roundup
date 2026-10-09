# Rust core with portable-pty

Status: Accepted

## Decision

Core in Rust: `portable-pty` for PTYs, `alacritty_terminal` or `vt100` for screen state, `tokio`.

## Considered

Node + node-pty, Go.

## Why

Cold-start and RSS budgets (300 ms, 150 MB for 10 idle Agents) favour no GC runtime and no bundled Node. `portable-pty` is the maintained cross-platform PTY crate.

## Notes

Screen-state crate choice (`alacritty_terminal` vs `vt100`) is made in the terminal module's first PR against the scrape-fallback need; both sit behind the terminal crate's interface.
