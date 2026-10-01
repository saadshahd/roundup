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

Silent transitions with no hook: Esc interrupt, deny or Esc on a dialog, the trust-folder dialog (before `SessionStart`), SIGKILL or crash. The terminal title (OSC 0) closes the first two; exit status closes the last. The trust dialog needs a screen-scrape or an explicit pre-trusted cwd. Ignore the spurious `SubagentStop` about 1.3 s after `Stop`.

Input design: per-Agent `--settings` file with one command hook writing JSON lines to the Daemon, plus a title watcher and exit watcher. These are the `Signal`, `Output` and `Exit` Observations.
