# Claude Code hooks as the state Signal

Status: Accepted (evidence: `spikes/hooks-state/REPORT.md`, Claude Code 2.1.286, interactive PTY)

## Decision

Adapter takes Observations: PTY output, Signals (hook payloads) and exit. Hooks are preferred; screen scraping is the fallback behind `AgentAdapter`.

## Considered

Screen scraping only, parsing transcripts.

## Why

Hooks are structured and versioned; scraping breaks on UI changes. Kind `blocked` is never emitted by the adapter; `rupd` derives it from Todos and Routes.

## Notes

Hooks alone are not enough; the adapter merges three inputs.

| Kind | Source |
|---|---|
| working | `UserPromptSubmit` (about 0.3 s after Enter); terminal title ◐/◑ |
| idle | `SessionStart`, `Stop`; terminal title ✳ |
| needs-you | `PermissionRequest` (about 0.04 s after the dialog; covers Bash and AskUserQuestion). Ignore the `permission_prompt` Notification, which arrives about 6 s late |
| error | `StopFailure` (field `error`; `Stop` does not fire). `PostToolUseFailure` is a failed tool, not an agent error |
| done | `SessionEnd` (`prompt_input_exit`); otherwise PTY EOF and exit status. Hooks cannot tell idle from done, so roundup defines done |

Silent transitions with no hook: Esc interrupt, deny or Esc on a dialog, the trust-folder dialog (before `SessionStart`), SIGKILL or crash. The terminal title (OSC 0) closes Esc interrupt only: in the fixtures a deny shows a spinner, not a star, so a deny holds `needs-you` until the next prompt. A star that follows `PreToolUse` lands 40–80 ms before `PermissionRequest` and is ignored, so `idle` never flickers before a dialog. Exit status closes the last. The trust dialog needs a screen-scrape or an explicit pre-trusted cwd. Ignore the spurious `SubagentStop` about 1.3 s after `Stop`.

Input design: per-Agent `--settings` file with one command hook per event writing JSON lines to the Daemon (`rup signal`; `PermissionRequest` runs `rup permission`, which waits for the user's answer, `docs/permission-decisions.md`), plus a title watcher and exit watcher. These are the `Signal`, `Title` and `Exit` Observations, plus `Stopped` when roundup stops the Agent itself (a stopped Agent is `done`, not `error`) and `Tick`, the injected clock, which resolves a star held after `PreToolUse` once `STAR_HOLD` has passed.

Sample size: one run per scenario, two for Esc and deny. Treat timings as indicative. Pre-trust the working directory to avoid the trust dialog.
