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
    if [ "${STUB_MODE:-}" = full ]; then echo '[{"name":"ru-1"},{"name":"ru-2"},{"name":"ru-3"},{"name":"ru-4"}]'; else echo '[]'; fi ;;
  "machine new") [ "${STUB_MODE:-}" != exists ] || exit 1 ;;
  "machine reboot") ;;
  "machine remove") [ "${STUB_MODE:-}" != remove-fails ] || exit 1 ;;
  "machine exec")
    case "$*" in
      *claude*)
        case "${STUB_MODE:-}" in
          limit) echo '{"is_error":true,"api_error_status":429,"result":"x"}'; exit 1 ;;
          *) echo '{"is_error":false,"result":"ok","num_turns":1,"duration_ms":1000,"total_cost_usd":0.1}' ;;
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


new_repo; loop/boxd.sh review r prompt.md >out 2>err && echo "ok:   L9 review succeeds" || { echo "FAIL: L9 review succeeds"; failures=$((failures + 1)); }
expect_true "L9 verdict written" test "$(cat loop/out/verdicts/r.md)" = ok
expect_log 'machine new ru-r .*--isolated' "L9 review VM is isolated"
expect_log 'machine remove ru-r' "L9 review VM destroyed"

new_repo; STUB_MODE=full expect_code 0 "L8 the default cap allows more than 4 VMs"
new_repo; BOXD_MAX_VMS=x STUB_MODE= expect_code 2 "L8 non-numeric BOXD_MAX_VMS is refused"

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

[ "$failures" -eq 0 ] || { echo "$failures failed"; exit 1; }
