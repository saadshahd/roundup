# Spike E7: does the real Claude Code take a Brief, a SessionStart context and an MCP server at start?

Headless is already observed: `spikes/spawn-boundary/REPORT.md`, row `e7`, on `claude` 2.1.283, found all three reach a headless run and the MCP server start before the first prompt. This spike runs the interactive TUI, `spikes/context-injection/run.sh`, which the `claude` process itself is driving (not headless `-p`).

Ran 2026-10-06 on this container, Claude Code **2.1.291**, `--model haiku`, interactive TUI in a `tmux` pty, a throwaway project directory. Setup: `--append-system-prompt-file` naming `brief.md` ("The first secret word is ALPHA."); `--settings` naming a `SessionStart` command hook that prints `{"hookSpecificOutput": {"hookEventName": "SessionStart", "additionalContext": "The second secret word is BRAVO."}}`; `--mcp-config` naming a stdio server (`mcp_stub.js`) that appends a timestamp to a file on its first line of JS execution (module load, before it reads any request) and offers one tool, `ping`.

| # | Seam | Prompt | Observed |
|---|---|---|---|
| 1 | `--append-system-prompt-file` (the Brief) | "State the two secret words you were given, nothing else." | Replied `ALPHA` — the model could state its word. |
| 2 | `SessionStart` hook `additionalContext` | same prompt | Replied `BRAVO` on the next line — the model could state its word too. |
| 3 | `--mcp-config` stdio server | "Call the MCP tool ping and report its text, or say NO SUCH TOOL." | The server's process wrote its start timestamp (`mcp_started`) while the TUI was still on the onboarding/ready screen, about 10 s before the first prompt was sent and well before the tool was ever called. The call itself needed one first-use permission prompt ("Do you want to proceed?"), then returned `pong-from-stub`. |

All three: yes, the model could state its word (1 and 2), and the server started before the first prompt (3), matching the headless row `e7`. The MCP server is started at session start regardless of whether its tool is ever called — it is not started lazily on first tool call, so E6's open question ("If E7's report shows that Claude Code starts MCP servers only on the first tool call, [the no-channel-deadline clause] is rewritten there") does not apply; no clause in `scenarios/awareness.md` is contradicted.

One difference from headless worth noting for a Builder relying on this: the interactive TUI's first call to a newly-configured MCP tool shows a one-time permission prompt ("Do you want to proceed?" / "Yes, and don't ask again for `<server>` — `<Tool>` commands in `<dir>`") that headless mode (`-p`) does not surface. `rup mcp` is the user's own server (A11), so this is the vendor's standard first-use-of-a-tool gate, not something this spike's claims above change.

Not run: the sandbox, a real multi-turn Agent session (haiku, one exchange), what happens across a `/clear` or compaction, and whether the permission prompt in row 3 can be preseeded through `--settings` (`permissions.allow`) to avoid an interactive approval during an unattended Agent run — worth a follow-up if E4/E5 land before a non-interactive answer exists.

Not kept: raw pane captures and the `mcp_started` file were written under `/tmp/ci-e7` by `run.sh`, outside the repo, and are not checked in; re-run `run.sh` to reproduce.
