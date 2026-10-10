#!/bin/bash
# usage: hook.sh <sleep_s> <reply_file|-> <log_dir>
# PermissionRequest hook: logs its payload, its sleep pid, its start, each trapped signal
# and its end to <log_dir>/hook.jsonl, sleeps <sleep_s> s, then prints <reply_file> ("-": nothing).
s=$1 reply=$2 log=$3/hook.jsonl
ts() { perl -MTime::HiRes=time -e 'printf "%.3f", time'; }
say() { printf '{"t":%s,"pid":%s,"ev":"%s"}\n' "$(ts)" $$ "$1" >> "$log"; }
for sig in TERM HUP INT PIPE; do trap "say SIG$sig" $sig; done
payload=$(cat)
# The sleep ignores TERM/HUP/INT/PIPE, so only a SIGKILL ends it: sleep gone with the hook means a group kill.
( trap '' TERM HUP INT PIPE; exec sleep "$s" ) & sp=$!
printf '{"t":%s,"pid":%s,"sleep_pid":%s,"ev":"start","payload":%s}\n' "$(ts)" $$ $sp "$payload" >> "$log"
while kill -0 $sp 2>/dev/null; do wait $sp; done
say slept
[ "$reply" != - ] && cat "$reply"
say exit
