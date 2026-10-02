#!/bin/sh
# Spike: how far can a per-Agent --settings file keep a Claude Code Agent from starting children by shell?
# Runs on a VM, headless (claude -p), one short prompt per case. Output: /tmp/sb/out/<case>.json and /tmp/sb/*.log
set -u
SB=/tmp/sb; rm -rf $SB; mkdir -p $SB/out $SB/shim $SB/work; cp "$(dirname "$0")"/*.js "$(dirname "$0")"/hook_log.sh $SB/; chmod +x $SB/*.js $SB/hook_log.sh
cd $SB/work
printf '#!/bin/sh\necho "use agent_spawn" >&2; exit 1\n' > $SB/shim/claude; chmod +x $SB/shim/claude
C="claude -p --model haiku --output-format stream-json --verbose --no-session-persistence"
run() { name=$1; shift; timeout 180 "$@" >$SB/out/$name.json 2>$SB/out/$name.err; echo "$name rc=$?" >> $SB/rc.log; }
# 1. are hooks in --settings honoured, and does exit 2 block?
cat > $SB/s1.json <<J
{"permissions":{"allow":["Bash"]},"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"$SB/hook_block.js","timeout":5}]}]}}
J
run h_allowed $C --settings $SB/s1.json -- "Run the bash command: echo hello-sb . Reply with its output only."
run h_blocked $C --settings $SB/s1.json -- "Run the bash command: bash -c 'echo claude-ok' . Reply with its output only."
# 2. deny rule
cat > $SB/s2.json <<J
{"permissions":{"allow":["Bash"],"deny":["Bash(claude:*)"]}}
J
run d_direct $C --settings $SB/s2.json -- "Run the bash command: claude --version . Reply with its output only."
run d_bashc $C --settings $SB/s2.json -- "Run the bash command: bash -c 'claude --version' . Reply with its output only."
# 3. tool removal
run t_nobash $C --tools Read -- "Run the bash command: echo hello-sb . If you cannot, say CANNOT."
run t_noagent $C --settings $SB/s1.json --disallowedTools Agent -- "Use the Agent tool to start a subagent that replies hi. If you cannot, say CANNOT."
# 4. PATH shim through settings.env
cat > $SB/s4.json <<J
{"permissions":{"allow":["Bash"]},"env":{"PATH":"$SB/shim:$PATH"}}
J
run p_shim $C --settings $SB/s4.json -- "Run the bash command: claude --version . Reply with its output only."
# 5. E7: system-prompt file, SessionStart context, MCP server start
echo "The first secret word is ALPHA." > $SB/brief.md
cat > $SB/mcp.json <<J
{"mcpServers":{"stub":{"type":"stdio","command":"node","args":["$SB/mcp_stub.js"]}}}
J
cat > $SB/s5.json <<J
{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"printf '%s' '{\"hookSpecificOutput\":{\"hookEventName\":\"SessionStart\",\"additionalContext\":\"The second secret word is BRAVO.\"}}'","timeout":5}]}],"UserPromptSubmit":[{"hooks":[{"type":"command","command":"$SB/hook_log.sh prompt","timeout":5}]}]}}
J
run e7 $C --settings $SB/s5.json --append-system-prompt-file $SB/brief.md --mcp-config $SB/mcp.json -- "State the two secret words you were given, nothing else."
# 6. is the Agent tool really gone? (the init tool list still shows "Task")
run a_disallow $C --settings $SB/s1.json --disallowedTools Agent -- "Call the Agent tool now with the prompt: say hi. Report the tool result verbatim, or the words NO SUCH TOOL."
run a_toolsro $C --tools Read,Bash --settings $SB/s4.json -- "Call the Agent tool now with the prompt: say hi. Report the tool result verbatim, or the words NO SUCH TOOL."
# 7. does --tools (an allowlist) keep MCP tools?
run m_tools $C --tools Read --settings $SB/s5.json --mcp-config $SB/mcp.json --allowedTools mcp__stub__ping -- "Call the MCP tool ping and report its text, or the words NO SUCH TOOL."
echo done >> $SB/rc.log
