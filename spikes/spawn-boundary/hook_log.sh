#!/bin/sh
# usage: hook_log.sh <event> ; appends a timestamp and the stdin payload
d=$(cat); echo "$(date +%s.%N) $1 $d" >> /tmp/sb/hooks.log
