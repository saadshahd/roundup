#!/bin/sh
# Spike E7: does the real Claude Code TUI take a Brief (--append-system-prompt-file), a
# SessionStart context hook, and an MCP server at start?
#
# Headless is already observed (spikes/spawn-boundary/REPORT.md, row e7). This script runs
# the interactive TUI in a tmux pty, in a throwaway project directory, and leaves two
# artifacts for the report: $CI/out/pane.txt (what the model said) and $CI/mcp_started
# (when the MCP server's process wrote its first line, one UNIX timestamp per line).
#
# Needs: claude, tmux, node, python3, a throwaway directory ($CI below). Merges
# hasCompletedOnboarding and a trust entry for $CI/proj into ~/.claude.json so the run
# does not stop on the onboarding or trust screens, then restores the original file.
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
CI=/tmp/ci-e7; rm -rf "$CI"; mkdir -p "$CI/out" "$CI/proj"
cp "$HERE/mcp_stub.js" "$CI/"; chmod +x "$CI/mcp_stub.js"

echo "The first secret word is ALPHA." > "$CI/brief.md"
cat > "$CI/mcp.json" <<J
{"mcpServers":{"stub":{"type":"stdio","command":"node","args":["$CI/mcp_stub.js"],"env":{"CI_MCP_STARTED_FILE":"$CI/mcp_started"}}}}
J
cat > "$CI/settings.json" <<'J'
{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"printf '%s' '{\"hookSpecificOutput\":{\"hookEventName\":\"SessionStart\",\"additionalContext\":\"The second secret word is BRAVO.\"}}'","timeout":5}]}]}}
J

CLAUDE_JSON="$HOME/.claude.json"
BACKUP="$CI/claude.json.bak"
[ -f "$CLAUDE_JSON" ] && cp "$CLAUDE_JSON" "$BACKUP"
restore() { [ -f "$BACKUP" ] && cp "$BACKUP" "$CLAUDE_JSON" || rm -f "$CLAUDE_JSON"; tmux kill-session -t ci-e7 2>/dev/null; }
trap restore EXIT
python3 - "$CLAUDE_JSON" "$CI/proj" <<'PY'
import json, os, sys
path, projdir = sys.argv[1], sys.argv[2]
d = json.load(open(path)) if os.path.exists(path) else {}
d["hasCompletedOnboarding"] = True
d.setdefault("projects", {})[projdir] = {"hasTrustDialogAccepted": True}
json.dump(d, open(path, "w"))
PY

tmux kill-session -t ci-e7 2>/dev/null
tmux new-session -d -s ci-e7 -x 220 -y 50 -c "$CI/proj" \
  "claude --model haiku --settings $CI/settings.json --append-system-prompt-file $CI/brief.md --mcp-config $CI/mcp.json"
sleep 10
tmux capture-pane -t ci-e7 -p -S -200 > "$CI/out/ready.txt"

tmux send-keys -t ci-e7 "State the two secret words you were given, nothing else."
sleep 1; tmux send-keys -t ci-e7 Enter; sleep 15
tmux capture-pane -t ci-e7 -p -S -2000 > "$CI/out/pane.txt"

tmux send-keys -t ci-e7 "Call the MCP tool ping and report its text, or say NO SUCH TOOL."
sleep 1; tmux send-keys -t ci-e7 Enter; sleep 6
# the first MCP call shows a one-off permission prompt ("Do you want to proceed?"); accept it
tmux send-keys -t ci-e7 Enter; sleep 6
tmux capture-pane -t ci-e7 -p -S -2000 > "$CI/out/pane.txt"

tmux kill-session -t ci-e7 2>/dev/null
claude --version > "$CI/out/version.txt"
echo "done; read $CI/out/pane.txt, $CI/mcp_started and $CI/out/version.txt"
