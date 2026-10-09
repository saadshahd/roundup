# Spike: can hooks alone derive roundup's Status Kind?

Claude Code 2.1.286, interactive, in a PTY, `--settings settings.json --setting-sources local` (no user settings touched).
Harness: `settings.json` (command hook on every documented event -> `logger.py` -> `log.jsonl`), `drive.py`/`drive2.py`/`drive3.py` (pty driver, `screen*.jsonl` = timestamped PTY output + marks). Raw logs per run: `log.run{1,2,3}.jsonl`.
Docs: https://code.claude.com/docs/en/hooks (event list via WebFetch; payload fields below are what was OBSERVED, which differs from the docs summary in places).

## Observed events -> Kind

Common payload fields: session_id, transcript_path, cwd, hook_event_name, prompt_id (per turn), permission_mode, effort, scratchpad_dir.

| Event | Extra fields observed | Kind | Latency vs. visible state |
|---|---|---|---|
| SessionStart | source=startup, model | idle | ~0.5-1s after trust-dialog accepted |
| UserPromptSubmit | prompt | working | ~0.3s after Enter |
| PreToolUse | tool_name, tool_input, tool_use_id | working (also fires right before a permission dialog) | immediate |
| PermissionRequest | tool_name, tool_input, permission_suggestions | needs-you (dialog is on screen) | ~0.04s after dialog drawn. Fires for Bash AND AskUserQuestion |
| Notification notification_type=permission_prompt | message "Claude needs your permission" | needs-you (confirm) | **~6s late** (also emitted as OSC 777 notify at the same moment) |
| Notification notification_type=idle_prompt | message | idle (already idle since Stop) | 60s after Stop. Informational only |
| PostToolUse | tool_response, duration_ms | working (permission resolved / tool ran) | immediate; this is the only "dialog answered yes" signal |
| PostToolUseFailure | error ("Exit code 1 ..."), is_interrupt | working (tool error is NOT an agent error) | immediate |
| PostToolBatch, MessageDisplay | tool_calls / delta, final | working | noise |
| Stop | last_assistant_message, stop_hook_active, background_tasks | idle (or done, see below) | same ms as final MessageDisplay; title flips ~50ms later |
| SubagentStop | agent_type "" (spurious, ~1.3s after Stop, last msg "ls") | ignore (internal helper) | do NOT treat as a state change |
| StopFailure | **`error`** (not error_type), e.g. model_not_found, last_assistant_message=error text | error | ~0.6s after prompt submit; Stop does NOT fire |
| SessionEnd | reason=prompt_input_exit on /exit | done | ~0.3s; process exit code 0 |

Not observed (no trigger attempted or not fired): PermissionDenied (auto mode only), Elicitation (MCP only), Compact/Worktree/Task events.

## Gaps (hooks are silent)

1. **Esc interrupt of a running turn: no hook at all** (no Stop, no PostToolUseFailure). Agent looks "working" forever in hook-state.
2. **Esc on a permission/AskUserQuestion dialog, and choosing "No": no hook at all.** needs-you never clears via hooks. (Choosing Yes clears via PostToolUse.)
3. **Trust-folder dialog on first run in a dir: no hooks** (SessionStart only fires after acceptance). Also default selection is "No, exit".
4. **SIGKILL / crash: no SessionEnd.** Need PTY EOF / waitpid.
5. Auth-expired/login screens and other pre-session prompts: not hookable.
6. needs-you notification arrives 6s late; PermissionRequest is the prompt one.
7. "done" vs "idle": hooks do not distinguish; Stop = turn finished. `done` must be defined by roundup (e.g. SessionEnd, or process exit).

## Derivation of Kinds

- needs-you: PermissionRequest (fast). Cleared by PostToolUse / Stop / UserPromptSubmit; NOT cleared by deny/Esc (gap 2).
- working: UserPromptSubmit .. Stop (reset on any tool event).
- idle: SessionStart, Stop. idle_prompt Notification is redundant.
- error: StopFailure (`error` field gives the reason). Tool failures are not errors. Process crash = PTY EOF with nonzero/signal status.
- done: SessionEnd or PTY EOF with clean exit.

## Fallback signal that closes gaps 1-2: terminal title (OSC 0)

Claude Code sets the title to `<glyph> <task title>`: glyph alternates **◐/◑ every ~1s while working**, and is **✳ whenever it is NOT working** (idle, permission dialog showing, after Esc/deny). Flip is ~50ms after hook-visible transitions; flips to ✳ on Esc (95.84s vs. Esc at 95.77s) with no hook. Also OSC 777 `notify;Claude Code;Claude needs your permission` accompanies the permission Notification.
So: title says "working vs not", hooks say *why* not working (needs-you via PermissionRequest vs idle via Stop).

## Recommended adapter input design

Two inputs merged into one reducer, last-writer-wins by timestamp:
1. Hook stream: a single command hook for {SessionStart, UserPromptSubmit, PreToolUse, PermissionRequest, PostToolUse, PostToolUseFailure, Stop, StopFailure, SessionEnd} writing JSON lines (session_id, ts, payload) to a per-agent socket/file (path via env var; per-agent `--settings` file). Skip Notification, MessageDisplay, PostToolBatch, SubagentStop.
2. PTY title watcher: parse OSC 0 glyph. Rules: glyph is ◐/◑ -> working (even if hook says needs-you, which cannot happen legitimately). Glyph ✳ while hook state is working/needs-you for >~1.5s with no hook -> idle (covers interrupt, deny, dialog cancel).
3. PTY EOF + waitpid -> done (status 0) or error (nonzero/signal).
4. Trust dialog: roundup should pre-trust the dir (or detect "Is this a project you created or one you trust" in screen text) since no hook exists.

Cautions: payload field names differ from docs summary (StopFailure `error`, UserPromptSubmit `prompt`); keep parsing tolerant and log unknowns. Caveat: typing into the PTY while a dialog is open is read as dialog input; when no dialog is open it submits as a prompt (UserPromptSubmit prompt "4" seen).
Sample size: one run per scenario; Esc/deny gap observed twice each (run2, run3).

## Addendum: Esc during tool run / dialog, Yes-approval timing

Driver `drive4.py <n> <esc-allowed|esc-dialog|approve3>` (per-run `--settings` file written to the tmp dir, hooks -> `logger2.py <log>`, throwaway cwd; trust dialog accepted via the driver as before, not skipped). Raw: `log.run{4..7}.jsonl` (hooks + `MARK` lines, same clock), `screen.run{4..7}.jsonl` (PTY output + parsed `title` records + marks). `analyze.py N...` prints the merged timeline. Notes: a standalone `sleep 30` is blocked by Claude Code itself ("use Monitor/run_in_background"), and `sleep` in a plain prompt gets backgrounded, so the long tool is `python3 -c "import time; time.sleep(30)"` with `permissions.allow ["Bash(python3:*)"]`.

| Scenario | Runs | Result |
|---|---|---|
| 1. Esc 3 s into an auto-allowed Bash tool (no dialog) | run4, run5 | **No hook after Esc** in a 12 s window: no PostToolUseFailure (so no `is_interrupt`), no PostToolUse, no Stop, no SubagentStop. Title: ◐/◑ until Esc, then a single `✳` frame **+70 ms** (run4) / **+64 ms** (run5) after Esc, no further frames. |
| 2. Esc on an open PermissionRequest dialog | run6 | Dialog ✳ at PreToolUse+48 ms, PermissionRequest +19 ms later. After Esc (2.5 s later): **no hook and no title frame** for 12 s (title was already ✳). Matches the earlier finding. |
| 3. Dialog approved with Yes (Enter) | run7 (3 dialogs) | See below. |

Yes-approval ordering is always PreToolUse -> title `✳` -> PermissionRequest; after the Yes keypress: title `◑/◐` +~10 ms, PostToolUse +50-400 ms, Stop ~1 s later.

| Dialog (all 9 captured) | PreToolUse -> `✳` title | `✳` title -> PermissionRequest |
|---|---|---|
| old runs (5) | 80, 60, 14, 23, 49 ms | 50, 79, 38, 45, 63 ms |
| run6 (Esc) | 48 ms | 19 ms |
| run7 (Yes x3) | 14, 14, 9 ms | 19, 18, 18 ms |
| **min / max over 9** | **9 / 80 ms** | **18 / 79 ms** |

PreToolUse -> PermissionRequest is therefore 59-130 ms total; the star precedes PermissionRequest by at most ~80 ms (no ~200 ms window observed). The `✳` stays through the whole dialog until the user answers (2.5 s in these runs).

Implication: Esc during a running tool is signalled ONLY by the `✳` title (~65 ms later); hooks stay silent, so hook-state remains "working" until the title rule fires.
