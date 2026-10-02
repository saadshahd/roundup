#!/bin/sh
# Spike F7: the real Claude Code TUI in a tmux pty. Output: /tmp/sbi/out/<case>.txt (the pane after the prompts)
set -u
SB=/tmp/sbi; rm -rf $SB/out $SB/w*; mkdir -p $SB/out $SB/shim
printf '#!/bin/sh\necho "use agent_spawn" >&2; exit 1\n' > $SB/shim/claude; chmod +x $SB/shim/claude
P1='Run the bash command: echo hi-int . Reply with its output, or the word CANNOT if you have no way to run it.'
P2='Call the Agent tool now with the prompt: say hi. Reply with the tool result, or the words NO SUCH TOOL.'
case_run() { # name workdir "claude args"
  name=$1; dir=$2; shift 2
  tmux kill-session -t $name 2>/dev/null
  tmux new-session -d -s $name -x 180 -y 50 -c "$dir" "$*"
  sleep 12
  tmux send-keys -t $name "$P1"; sleep 1; tmux send-keys -t $name Enter; sleep 40
  tmux send-keys -t $name "$P2"; sleep 1; tmux send-keys -t $name Enter; sleep 40
  tmux capture-pane -t $name -p -S -200 > $SB/out/$name.txt
  tmux kill-session -t $name
}
mkdir -p $SB/w1 $SB/w2/.claude $SB/w3
cat > ~/.claude.json <<J
{"hasCompletedOnboarding":true,"theme":"dark","projects":{"$SB/w1":{"hasTrustDialogAccepted":true},"$SB/w2":{"hasTrustDialogAccepted":true},"$SB/w3":{"hasTrustDialogAccepted":true}}}
J
echo '{"permissions":{"allow":["Bash(*)"]}}' > $SB/w2/.claude/settings.json
echo '{}' > $SB/s.json
case_run i_base $SB/w1 claude --model haiku --tools Read,Grep,Glob --settings $SB/s.json
case_run i_proj $SB/w2 claude --model haiku --tools Read,Grep,Glob --settings $SB/s.json
mkdir -p ~/.claude; [ -f ~/.claude/settings.json ] && cp ~/.claude/settings.json $SB/user.bak
echo '{"permissions":{"allow":["Bash(*)"]}}' > ~/.claude/settings.json
case_run i_user $SB/w3 claude --model haiku --tools Read,Grep,Glob --settings $SB/s.json
if [ -f $SB/user.bak ]; then cp $SB/user.bak ~/.claude/settings.json; else rm ~/.claude/settings.json; fi
printf '{"env":{"PATH":"%s:%s"},"permissions":{"allow":["Bash(*)"]}}' $SB/shim "$PATH" > $SB/s4.json
P1='Run the bash command: claude --version . Reply with its output only.'
case_run i_shim $SB/w1 claude --model haiku --tools Read,Bash --settings $SB/s4.json
echo done > $SB/out/DONE
