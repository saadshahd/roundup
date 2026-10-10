#!/bin/bash
# usage: loghook.sh <log_dir>
# Hook for every other event: appends its payload and arrival time to <log_dir>/events.jsonl.
printf '{"t":%s,"payload":%s}\n' "$(perl -MTime::HiRes=time -e 'printf "%.3f", time')" "$(cat)" >> "$1/events.jsonl"
