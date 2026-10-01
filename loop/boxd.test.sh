#!/usr/bin/env bash
# Tests for loop/boxd.sh against a stub `boxd`. Usage: loop/boxd.test.sh
set -euo pipefail

script="$(cd "$(dirname "$0")" && pwd)/boxd.sh"
failures=0

new_repo() {
  dir=$(mktemp -d)
  cd "$dir"
  git init -q -b main
  git config user.email t@t
  git config user.name t
  mkdir loop bin
  cp "$script" loop/boxd.sh
  echo x >f
  git add -A
  git commit -qm base
  git update-ref refs/remotes/origin/main HEAD
  echo 'do the thing' >prompt.md
  # The stub logs every call; `exec ... claude` answers per $STUB_MODE.
  cat >bin/boxd <<'S'
#!/usr/bin/env bash
echo "$*" >>"$STUB_LOG"
if [ "$1 $2 $3" = "machine cp -" ]; then cat >"$STUB_DIR/$(basename "$4")"; else cat >/dev/null 2>&1 || true; fi
case "$1 $2" in
  "env list") if [ "${STUB_MODE:-}" = no-secret ]; then echo '[]'; else echo '[{"name":"CLAUDE_CODE_OAUTH_TOKEN"}]'; fi ;;
  "machine list")
    if [ -n "${STUB_RU:-}" ]; then jq -nc --argjson n "$STUB_RU" '[range($n) | {name: "ru-\(.)"}] + [{name: "db"}, {name: "web-1"}, {name: "ru"}]'
    elif [ "${STUB_MODE:-}" = full ]; then echo '[{"name":"ru-1"},{"name":"ru-2"},{"name":"ru-3"},{"name":"ru-4"}]'
    elif [ "${STUB_MODE:-}" = mixed ]; then echo '[{"name":"ru-builder-1"},{"name":"ru-reviewer-2"},{"name":"ru-x8-1"},{"name":"ru-builderx"},{"name":"ru-x-builder-9"},{"name":"ru-builder-x"},{"name":"ru-builder-3"}]'
    else (cd "$STUB_DIR" && ls alive-* 2>/dev/null || true) | jq -Rnc '[inputs | {name: sub("^alive-"; "")}]'; fi ;;
  "machine new")
    [ "${STUB_MODE:-}" != exists ] || exit 1
    touch "$STUB_DIR/alive-$3"; ls "$STUB_DIR"/alive-* | wc -l | tr -d " " >>"$STUB_DIR/maxlog" ;;
  "machine reboot") ;;
  "machine remove")
    if [ "${STUB_MODE:-}" = remove-fails ] || [ "${STUB_FAIL_REMOVE:-}" = "$3" ]; then exit 1; fi
    rm -f "$STUB_DIR/alive-$3"
    if [ -e "$STUB_DIR/exec-$3.pid" ]; then kill "$(cat "$STUB_DIR/exec-$3.pid")" 2>/dev/null || true; fi ;;
  "machine exec")
    case "$*" in
      *pgrep*)
        case "$3" in ru-builder-1) echo agent-running ;; ru-reviewer-2) echo idle ;; *) exit 1 ;; esac ;;
      *claude*)
        case "${STUB_MODE:-}" in
          limit) echo '{"is_error":true,"api_error_status":429,"result":"x"}'; exit 1 ;;
          *)
            echo $$ >"$STUB_DIR/exec-$3.pid"
            sleep "${STUB_CLAUDE_SLEEP:-0}"
            echo '{"is_error":false,"result":"ok","num_turns":1,"duration_ms":1000,"total_cost_usd":0.1}' ;;
        esac ;;
      *"just check"*) [ "${STUB_MODE:-}" != check-fails ] || exit 7 ;;
      *format-patch*) echo "patch" ;;
    esac ;;
esac
S
  chmod +x bin/boxd
  export BOXD_LOCK_WAIT=1 PATH="$PWD/bin:$PATH" STUB_LOG="$PWD/log" STUB_DIR="$PWD/cp"
  mkdir cp
  : >log
}

run() { STUB_MODE=${STUB_MODE:-} loop/boxd.sh build t prompt.md >out 2>err; }

expect_code() {
  local want=$1 name=$2 got=0
  run || got=$?
  if [ "$got" -ne "$want" ]; then echo "FAIL: $name (wanted exit $want, got $got)"; failures=$((failures + 1)); else echo "ok:   $name"; fi
}

# expect_true <name> <command...>: ok when the command succeeds.
expect_true() {
  local name=$1
  shift
  if "$@" >/dev/null 2>&1; then echo "ok:   $name"; else echo "FAIL: $name"; failures=$((failures + 1)); fi
}

expect_log() {
  if grep -qE -- "$1" log; then echo "ok:   $2"; else echo "FAIL: $2 (log lacks /$1/)"; failures=$((failures + 1)); fi
}

new_repo; STUB_MODE='' expect_code 0 "L5 build succeeds"
expect_log 'machine remove ru-t' "L5 VM destroyed after success"
expect_log 'machine new ru-t .*--isolated' "L7 VM is isolated"
expect_true "L5 patch written" test -s loop/out/patches/t.patch

new_repo; STUB_MODE=limit expect_code 75 "L4 limit error pauses with 75"
expect_true "L4 PAUSED written" test -e loop/out/PAUSED
expect_log 'machine remove ru-t' "L5 VM destroyed after a limit"
: >log; STUB_MODE='' expect_code 75 "L4 paused file blocks the next run"
grep -q 'machine new' log && { echo "FAIL: no VM while paused"; failures=$((failures + 1)); } || echo "ok:   no VM while paused"

new_repo; STUB_MODE=check-fails expect_code 7 "L5 failed check keeps its exit code"
expect_log 'machine remove ru-t' "L5 VM destroyed after a failed check"


new_repo; STUB_MODE=remove-fails expect_code 0 "L5 cleanup failure does not change the exit code"
expect_true "L5 leaked VM reported" grep -q LEAKED err

new_repo; STUB_MODE=exists expect_code 1 "L5 a failed create does not remove the VM"
if grep -q 'machine remove' log; then echo "FAIL: L5 existing VM removed"; failures=$((failures + 1)); else echo "ok:   L5 existing VM untouched"; fi

new_repo; STUB_MODE=no-secret expect_code 1 "L6 a missing boxd secret stops the run"
if grep -q 'machine new' log; then echo "FAIL: L6 VM created without the secret"; failures=$((failures + 1)); else echo "ok:   L6 no VM without the secret"; fi

new_repo; BOXD_MAX_VMS=4 STUB_MODE=full expect_code 1 "L8 BOXD_MAX_VMS=4 refuses a fifth VM"
if grep -q 'machine new' log; then echo "FAIL: L8 VM created over the cap"; failures=$((failures + 1)); else echo "ok:   L8 no VM over the cap"; fi

new_repo; mkdir -p loop/out/lock; STUB_MODE='' expect_code 1 "L8 a held lock stops the run"

new_repo; mkdir -p loop/out; echo old-time >loop/out/PAUSED; STUB_MODE='' expect_code 75 "L4 existing pause is kept"
expect_true "L4 PAUSED keeps its original time" test "$(cat loop/out/PAUSED)" = old-time


new_repo
if loop/boxd.sh review r prompt.md >out 2>err; then echo "ok:   L9 review succeeds"; else echo "FAIL: L9 review succeeds"; failures=$((failures + 1)); fi
expect_true "L9 verdict written" test "$(cat loop/out/verdicts/r.md)" = ok
expect_log 'machine new ru-r .*--isolated' "L9 review VM is isolated"
expect_log 'machine remove ru-r' "L9 review VM destroyed"

new_repo; STUB_RU=12 STUB_MODE='' expect_code 1 "L8 the default cap refuses a 13th ru- VM (12 exist)"
if grep -q 'machine new' log; then echo "FAIL: L8 VM created over the default cap"; failures=$((failures + 1)); else echo "ok:   L8 no VM over the default cap"; fi
new_repo; STUB_RU=11 STUB_MODE='' expect_code 0 "L8 11 ru- VMs leave room for a 12th; non-ru- VMs are not counted"
new_repo; BOXD_MAX_VMS=x STUB_MODE='' expect_code 2 "L8 non-numeric BOXD_MAX_VMS is refused"

# Hostile inputs are refused before any VM or file is made.
refused() { # refused <name> <exit-code> <args...>
  local label=$1 args=("${@:2}")
  new_repo
  loop/boxd.sh "${args[@]}" >out 2>err && { echo "FAIL: $label accepted"; failures=$((failures + 1)); return; }
  if grep -q 'machine new' log || [ -e ../esc.json ] || [ -e loop/out/runs/esc.json ]; then echo "FAIL: $label made a VM or file"; failures=$((failures + 1)); else echo "ok:   $label refused"; fi
}
refused "L10 name with a path" build ../../esc prompt.md
refused "L10 name with a shell metacharacter" build 'a;rm -rf ~' prompt.md
refused "L10 name with a newline" build $'a\nb' prompt.md
refused "L10 option-like name" review -x prompt.md
refused "L10 option-like prompt file" build t -x
refused "L10 option-like ref" review t prompt.md --output=x

# The Reviewer's checkout: tag base is the merge-base, HEAD is the ref.
new_repo; git switch -qc feat; echo y >g; git add g; git commit -qm feat
loop/boxd.sh review r prompt.md feat >out 2>err
expect_true "L9 base archive is the merge-base (no feature file)" bash -c '! tar tzf cp/base.tgz | grep -q "^g$"'
expect_true "L9 src archive is the ref (has feature file)" bash -c 'tar tzf cp/src.tgz | grep -q "^g$"'
expect_log 'git tag base' "L9 base is tagged"
expect_log '--auto-destroy-timeout 1800' "L5 VM has an auto-destroy timer"
expect_log 'machine reboot ru-r' "L11 VM is rebooted after restore"

# swarm, status, kill
new_repo; export BOXD_LOCK_WAIT=30; echo p >p2.md; echo p >p3.md
if STUB_MODE='' loop/boxd.sh swarm build prompt.md p2.md p3.md >out 2>err; then echo "ok:   L12 swarm exits 0 when every agent succeeds"; else echo "FAIL: L12 swarm exit"; failures=$((failures + 1)); fi
for n in 1 2 3; do
  expect_true "L12 status line for ru-builder-$n" grep -qx "ru-builder-$n ok" out
  expect_log "machine new ru-builder-$n .*--isolated" "L12 ru-builder-$n is its own isolated VM"
done
expect_true "L12 exactly one line per VM" test "$(wc -l <out | tr -d ' ')" = 3

# At most BOXD_MAX_VMS agents at once, counted from the VMs that actually exist; 2 proves they do overlap.
for cap in 1 2; do
  new_repo; export BOXD_LOCK_WAIT=60; echo p >p2.md; echo p >p3.md
  BOXD_MAX_VMS=$cap STUB_CLAUDE_SLEEP=2 loop/boxd.sh swarm build prompt.md p2.md p3.md >out 2>err || true
  expect_true "L12 cap $cap: all three agents ran" test "$(grep -c ' ok$' out)" = 3
  expect_true "L12 cap $cap: never more than $cap VMs at once" test "$(sort -n cp/maxlog | tail -1)" = "$cap"
done

new_repo; export BOXD_LOCK_WAIT=30; echo p >p2.md
if STUB_MODE=check-fails loop/boxd.sh swarm build prompt.md p2.md >out 2>err; then echo "FAIL: L12 failing agents reported ok"; failures=$((failures + 1)); else echo "ok:   L12 failing agents fail the swarm"; fi
expect_true "L12 each failing agent gets its own failed line" test "$(grep -c 'failed rc=7 (see loop/out/runs/swarm-[0-9]*-[12].log)' out)" = 2
expect_true "L12 the two lines name different VMs" test "$(grep -c '^ru-builder-1 failed' out)$(grep -c '^ru-builder-2 failed' out)" = 11

new_repo; echo p >p2.md
if loop/boxd.sh swarm build prompt.md missing.md >out 2>err; then echo "FAIL: L12 missing prompt accepted"; failures=$((failures + 1)); else echo "ok:   L12 a bad prompt file is refused"; fi
if grep -q 'machine new' log; then echo "FAIL: L12 a VM was made before the bad prompt was refused"; failures=$((failures + 1)); else echo "ok:   L12 no VM before every prompt is validated"; fi

new_repo; git switch -qc feat; echo y >g; git add g; git commit -qm feat; git switch -q main; git update-ref refs/remotes/origin/main main
BOXD_REF=feat loop/boxd.sh swarm review prompt.md >out 2>err
expect_log 'machine new ru-reviewer-1 ' "L12 swarm review names ru-reviewer-<n>"
expect_true "L12 swarm review checks out BOXD_REF" bash -c 'tar tzf cp/src.tgz | grep -q "^g$"'

refused "L12 swarm with an unknown role" swarm nope prompt.md
refused "L12 swarm with no prompt files" swarm build

# SIGTERM: the swarm removes its VMs and its children die, instead of burning quota to the end.
new_repo; echo p >p2.md
STUB_CLAUDE_SLEEP=30 loop/boxd.sh swarm build prompt.md p2.md >out 2>err &
swarm_pid=$!
for _ in $(seq 100); do [ "$(find cp -name 'exec-*.pid' | wc -l | tr -d ' ')" = 2 ] && break; sleep 0.1; done
kill -TERM "$swarm_pid"
rc=0; wait "$swarm_pid" || rc=$?
expect_true "L12 interrupted swarm exits 130" test "$rc" = 130
expect_log 'machine remove ru-builder-1' "L12 interrupt removes ru-builder-1"
expect_log 'machine remove ru-builder-2' "L12 interrupt removes ru-builder-2"
expect_true "L12 interrupt stops the agents" bash -c "for f in cp/exec-*.pid; do ! kill -0 \$(cat \$f) 2>/dev/null || exit 1; done"

new_repo; STUB_MODE=mixed loop/boxd.sh status >out 2>err
expect_true "L13 status shows agent-running" grep -qx 'ru-builder-1 agent-running' out
expect_true "L13 status shows idle" grep -qx 'ru-reviewer-2 idle' out
expect_true "L13 status shows unreachable" grep -qx 'ru-x8-1 unreachable' out
expect_true "L13 status lists every ru- VM" test "$(wc -l <out | tr -d ' ')" = 7

new_repo; STUB_MODE=mixed loop/boxd.sh kill all >out 2>err
expect_log 'machine remove ru-builder-1' "L14 kill all removes swarm builders"
expect_log 'machine remove ru-reviewer-2' "L14 kill all removes swarm reviewers"
for foreign in ru-x8-1 ru-builderx ru-x-builder-9 ru-builder-x; do
  if grep -q "machine remove $foreign " log; then echo "FAIL: L14 kill all removed $foreign"; failures=$((failures + 1)); else echo "ok:   L14 kill all leaves $foreign"; fi
done
new_repo
if STUB_MODE=mixed STUB_FAIL_REMOVE=ru-builder-1 loop/boxd.sh kill all >out 2>err; then echo "FAIL: L14 failed removal hidden"; failures=$((failures + 1)); else echo "ok:   L14 a failed removal fails kill all"; fi
expect_true "L14 failed removal is named" grep -q '^FAILED' out
new_repo; STUB_MODE=mixed loop/boxd.sh kill all >out 2>err
new_repo; loop/boxd.sh kill foo >out 2>err; expect_log 'machine remove ru-foo' "L14 kill <name> removes ru-<name>"
new_repo
if loop/boxd.sh kill '../x' >out 2>err; then echo "FAIL: L14 hostile kill name accepted"; failures=$((failures + 1)); else echo "ok:   L14 hostile kill name refused"; fi

[ "$failures" -eq 0 ] || { echo "$failures failed"; exit 1; }
