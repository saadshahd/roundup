# shellcheck shell=bash
# Sourced by the loop/*.test.sh suites that keep a command's output in $FIXTURES/out and $FIXTURES/err and count
# $failures.

# check <name> <exit> <command…>: the command exits <exit>.
check() {
  local name=$1 want=$2 got=0
  shift 2
  "$@" >"$FIXTURES/out" 2>"$FIXTURES/err" || got=$?
  if [ "$got" -eq "$want" ]; then echo "ok:   $name"; else echo "FAIL: $name (exit $got, wanted $want)"; cat "$FIXTURES/out" "$FIXTURES/err"; failures=$((failures + 1)); fi
}

# holds <name> <command…>: the command succeeds.
holds() {
  local name=$1
  shift
  if "$@"; then echo "ok:   $name"; else echo "FAIL: $name"; cat "$FIXTURES/out" "$FIXTURES/err"; failures=$((failures + 1)); fi
}
