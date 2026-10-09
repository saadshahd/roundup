#!/usr/bin/env bash
# F7: run only on a disposable GitHub runner. Prints the private directory containing the captures.
set -euo pipefail
umask 077
[[ ${GITHUB_ACTIONS:-} == true ]] || { echo 'F7 requires a disposable GitHub Actions runner' >&2; exit 2; }
: "${CLAUDE_CODE_OAUTH_TOKEN:?F7 needs the runner OAuth environment, never a copied personal config}"
command -v tmux >/dev/null || { echo 'Install tmux on this disposable runner before F7' >&2; exit 2; }
SB=$(mktemp -d "${TMPDIR:-/tmp}/roundup-f7.XXXXXX")
export CLAUDE_CONFIG_DIR="$SB/config"
export DISABLE_AUTOUPDATER=1
unset CLAUDECODE
socket="roundup-f7-$(basename "$SB")"
cleanup() { tmux -L "$socket" kill-server 2>/dev/null || true; }
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
mkdir -p "$SB/out" "$SB/shim" "$CLAUDE_CONFIG_DIR" "$SB/w1" "$SB/w2/.claude" "$SB/w3"
printf '#!/bin/sh\necho "use agent_spawn" >&2; exit 1\n' > "$SB/shim/claude"
chmod +x "$SB/shim/claude"
jq -n --arg w1 "$SB/w1" --arg w2 "$SB/w2" --arg w3 "$SB/w3" \
  '{hasCompletedOnboarding:true,theme:"dark",projects:{($w1):{hasTrustDialogAccepted:true},($w2):{hasTrustDialogAccepted:true},($w3):{hasTrustDialogAccepted:true}}}' > "$CLAUDE_CONFIG_DIR/.claude.json"
claude auth status --json > "$SB/auth-status.json"
jq -e '.loggedIn == true' "$SB/auth-status.json" >/dev/null || { echo 'F7 isolated login is unavailable; no cases observed' >&2; exit 1; }
P1='Run the bash command: echo hi-int . Reply with its output, or the word CANNOT if you have no way to run it.'
P2='Call the Agent tool now with the prompt: say hi. Reply with the tool result, or the words NO SUCH TOOL.'
case_run() {
  local name=$1 dir=$2
  shift 2
  claude --version > "$SB/out/$name.version.txt"
  tmux -L "$socket" new-session -d -s "$name" -x 180 -y 50 -c "$dir" "$@"
  sleep 12
  tmux -L "$socket" send-keys -t "$name" "$P1"
  sleep 1
  tmux -L "$socket" send-keys -t "$name" Enter
  sleep 40
  tmux -L "$socket" send-keys -t "$name" "$P2"
  sleep 1
  tmux -L "$socket" send-keys -t "$name" Enter
  sleep 40
  tmux -L "$socket" capture-pane -t "$name" -p -S -200 > "$SB/out/$name.txt"
  test -s "$SB/out/$name.txt"
  tmux -L "$socket" kill-session -t "$name"
}
echo '{"permissions":{"allow":["Bash(*)"]}}' > "$SB/w2/.claude/settings.json"
echo '{}' > "$SB/s.json"
case_run i_base "$SB/w1" claude --model haiku --tools Read,Grep,Glob --settings "$SB/s.json"
case_run i_proj "$SB/w2" claude --model haiku --tools Read,Grep,Glob --settings "$SB/s.json"
echo '{"permissions":{"allow":["Bash(*)"]}}' > "$CLAUDE_CONFIG_DIR/settings.json"
case_run i_user "$SB/w3" claude --model haiku --tools Read,Grep,Glob --settings "$SB/s.json"
echo '{}' > "$CLAUDE_CONFIG_DIR/settings.json"
jq -n --arg path "$SB/shim:$PATH" '{env:{PATH:$path},permissions:{allow:["Bash(*)"]}}' > "$SB/s4.json"
P1='Run the bash command: claude --version . Reply with its output only.'
case_run i_shim "$SB/w1" claude --model haiku --tools Read,Bash --settings "$SB/s4.json"
printf 'Captured, not yet judged: inspect each pane for login, trust, timeout or tool errors before marking it observed.\n' > "$SB/out/DONE"
printf '%s\n' "$SB/out"
