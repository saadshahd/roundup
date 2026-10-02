# Spawn boundary

Design for `scenarios/spawn-boundary.md` (F1 to F5). Terms: Meta-agent, Home, Worktree (`CONTEXT.md`). Series B (Messages) supplies the message half: `agent_message` is B12's `message_send`, not a second tool.

## The question

If a Meta-agent runs Claude Code, it can start children with `bash -c 'claude -p ...'`. Then roundup cannot give the child a Worktree (G2) or a Home. The only way to make the Daemon start every child is to leave the Meta-agent no other way. This doc says what each layer can and cannot stop, from a real run (`spikes/spawn-boundary/REPORT.md`) and not from memory.

## What each layer does

| Layer | Stops | Does not stop | Observed |
|---|---|---|---|
| Remove `Bash`, `Agent` and everything unnamed: `--tools Read,Grep,Glob` | Every shell and subagent path in Claude Code's own tools. MCP tools stay. | Tools the user's own MCP servers add; a Meta-agent that must run commands | `t_nobash`, `a_toolsro`, `m_tools`: **guarantee** |
| Deny rule `Bash(claude:*)` | A direct `claude ...` | `bash -c 'claude ...'` (ran and printed a version) | `d_direct`, `d_bashc`: best effort, weak |
| `PreToolUse` hook that refuses the command | Any command whose text names a CLI, including inside `bash -c` | Absolute paths, obfuscation; costs one process per Bash call; the model rewrote the command and asked a subagent to run it | `h_blocked`: works, not chosen |
| `PATH` shim for the Adapters' program names (today `claude`) through `settings.env` | A lookup by name; fails loud with a message | An absolute path, a renamed copy, a script the Agent writes | `p_shim`: best effort |
| Process-tree scan | Nothing; it flags a vendor binary not registered as a Terminal | A renamed binary | not run here; best effort detection |
| OS sandbox (Seatbelt) with a network allowlist | Could stop a vendor CLI reaching its API from Bash | Escapes, MCP servers and hooks run outside it; it restricts the Agent's real network use | not run: not chosen now |

Scope: Meta-agents only, as the user decided. Chosen: the tool allowlist as the guarantee (F2), `agent_spawn` as the way (F3), and the shim (F4) and the scan (F5) as best effort that fails loud. Dropped: the deny rule (the shim covers its case better) and the hook (it adds only absolute-path calls and costs a process per call). Left for the user: the sandbox.

## Honest limits

- A shell can launch anything. With a shell, F4 and F5 stop and flag the common cases; they do not make a guarantee. The guarantee is that a Meta-agent has no shell (F2), and it holds inside Claude Code's tools only.
- The model works round a block (`h_blocked`: renamed the command, then asked a subagent). A layer that refuses one path is not a stop while another exists, which is why `Agent` is removed with `Bash`.
- Headless observation: the spike ran `claude -p` on one version. Interactive behaviour of `--tools` and `settings.env` is the vendor's docs, not observed.
- The user's own `.claude/settings.json` can set its own `permissions`. The spike did not test whether it can override flags; `--tools` is a flag, not a setting, which is why F2 uses it.
- A Meta-agent has no `Edit` or `Write`: it places and coordinates, and its children do the work. The user accepted a Meta-agent with no shell.

## Seam (rule 4, with the first Builder PR)

No new RPC method. `agent.spawn` gains a caller rule for Actors of kind `agent` (F3). `RailNode` gains `stray` and the event `agent.stray` is new (F5); the contract PR lists every `RailNode` caller (the webview fixtures, `seeds.ts`). `agent_spawn` is M1's rule applied to `agent.spawn` (a tool named after its method). It depends on #159 (G2, Home), #162 (E1's Brief) and #160 (B12).

## Gates

- **P2.** A child started by `agent_spawn` gets a Worktree when the setting is on (G2), so with it on two Agents do not share a cwd; with it off they may.
- **P4.** The flag, the shim names and `PATH` belong to `claude_code/`; the scan and the caller rule are vendor-neutral; `AgentAdapter` gains no seam.
- **P6.** The child's Home is its Meta-agent's node, set by the Daemon, not by the caller (a `parent` param from an Agent is ignored).

## Open for the user

- Decided by the user: F2 to F5 apply to Meta-agents only. An ordinary Agent keeps every tool and may run `claude -p` on purpose. F4 and F5 are redundant against a Meta-agent without a shell; they cover a shell a user's own MCP server adds, and an allowlist widened later.
- F5's cost is a process-table read every 2 s. The Builder measures it; if it moves the 10-idle-Agents budget, the interval or the trigger changes.
