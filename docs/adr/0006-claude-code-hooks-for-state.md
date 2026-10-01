# Claude Code hooks as the state Signal

Status: Pending evidence

## Decision

Adapter takes Observations: PTY output, Signals (hook payloads) and exit. Hooks are preferred; screen scraping is the fallback behind `AgentAdapter`.

## Considered

Screen scraping only, parsing transcripts.

## Why

Hooks are structured and versioned; scraping breaks on UI changes. Kind `blocked` is never emitted by the adapter; `rupd` derives it from Todos and Routes.

## Notes

Which hook signals needs-you, and whether interactive PTY mode fires the same set as `-p`, is established by `spikes/hooks-state/REPORT.md`.
