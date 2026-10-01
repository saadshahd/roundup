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
    elif [ "${STUB_MODE:-}" = full ]; then echo '[{"name":"ru-1"},{"name":"ru-2"},{"name":"ru-3"},{"name":"ru-4"}]'; else echo '[]'; fi ;;
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
      *"grep -rlF /tmp/warm"*) [ "${STUB_MODE:-}" != warm-dirty ] || exit 1 ;;
      *"grep -rIlE"*) [ "${STUB_MODE:-}" = bake-leak ] || exit 1 ;;
      *"just check"*)
        case "${STUB_MODE:-}" in check-fails) exit 7 ;; token-output) echo "log gho_abcdefghijklmnopqrstuvwxyz0123 end" ;; esac ;;
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

# L12: `check` merges on this machine and runs on an isolated VM. The origin lives beside the repo, not inside it.
check_repo() {
  new_repo
  git init -q --bare "$dir-origin.git"
  git remote add origin "$dir-origin.git"
  git push -q origin HEAD:main HEAD:refs/pull/54/head HEAD:refs/heads/builder/x
  git fetch -q origin
  export STUB_GIT_LOG="$dir-git.log"
  # shellcheck disable=SC2016 # the shim body expands when it runs
  printf '#!/usr/bin/env bash\necho "$*" >>"$STUB_GIT_LOG"\nexec %s "$@"\n' "$(command -v git)" >bin/git
  chmod +x bin/git
}
run_check_ref() { STUB_MODE=${STUB_MODE:-} loop/boxd.sh check "$1" >out 2>err; }

check_repo; got=0; run_check_ref 54 || got=$?
expect_true "L12 check of a PR number succeeds" test "$got" -eq 0
expect_log 'machine new ru-chk-54 .*--isolated' "L12 the check VM is isolated"
expect_log 'machine new ru-chk-54 .*--auto-destroy-timeout 1800' "L12 the check VM has an auto-destroy timer"
expect_log 'machine remove ru-chk-54' "L12 VM destroyed after success"
expect_true "L12 nothing is cloned or fetched on the VM" bash -c '! grep -qE "git (clone|fetch)" log'
expect_true "L12 the check installs from the lockfile" grep -q -- '--frozen-lockfile' log
expect_true "L12 the ref reaches git fetch after --" grep -q '^fetch -q origin -- +refs/heads/main:refs/boxd-check/54-base +pull/54/head:refs/boxd-check/54$' "$STUB_GIT_LOG"
expect_true "L12 no ref is left behind by the fetch" test -z "$(git for-each-ref refs/boxd-check)"

# The upload is origin/main as it is on origin now (not the local HEAD, not the stale tracking ref) plus the PR.
check_repo; echo local >local-only; git add local-only; git commit -qm local-only
git checkout -q -b pr-work origin/main; echo pr >pr-marker; git add pr-marker; git commit -qm pr; git push -q origin HEAD:refs/pull/54/head; git checkout -q main
git clone -q "$dir-origin.git" "$dir-other"; (cd "$dir-other"; echo fresh >fresh-main; git add fresh-main; git -c user.email=t@t -c user.name=t commit -qm fresh; git push -q origin HEAD:main)
got=0; run_check_ref 54 || got=$?
expect_true "L12 check succeeds with a local HEAD that differs from origin/main" test "$got" -eq 0
expect_true "L12 the upload holds the PR's change" bash -c 'tar tzf cp/src.tgz | grep -qx pr-marker'
expect_true "L12 the upload holds origin/main as fetched now" bash -c 'tar tzf cp/src.tgz | grep -qx fresh-main'
expect_true "L12 the upload leaves out local-only commits" bash -c '! tar tzf cp/src.tgz | grep -qx local-only'
expect_true "L12 the base is origin/main without the PR" bash -c 'tar tzf cp/base.tgz | grep -qx fresh-main && ! tar tzf cp/base.tgz | grep -qx pr-marker'

check_repo; got=0; STUB_MODE=check-fails run_check_ref builder/x || got=$?
expect_true "L12 failed check keeps its exit code" test "$got" -eq 7
expect_log 'machine remove ru-chk-builder-x' "L12 VM destroyed after a failed check"

check_repo; got=0; STUB_MODE=token-output run_check_ref 54 || got=$?
expect_true "L12 a GitHub token in the check output is masked" bash -c '! grep -rq gho_abcdefgh err loop/out/runs'

check_repo; got=0; STUB_MODE=no-secret run_check_ref 54 || got=$?
expect_true "L12 check needs no Claude secret" test "$got" -eq 0

for bad_ref in 'x; rm -rf ~' -x --tags; do
  check_repo; got=0; run_check_ref "$bad_ref" || got=$?
  expect_true "L12 the ref '$bad_ref' is refused" test "$got" -eq 2
  if grep -q 'machine new' log; then echo "FAIL: L12 VM created for '$bad_ref'"; failures=$((failures + 1)); fi
done

check_repo; git checkout -q -b clash origin/main; echo a >f; git commit -qam a; git push -q origin HEAD:refs/heads/builder/clash
git checkout -q main; echo b >f; git commit -qam b; git push -q origin HEAD:main
got=0; run_check_ref builder/clash || got=$?
expect_true "L12 a PR that conflicts with main stops before any VM" test "$got" -ne 0
if grep -q 'machine new' log; then echo "FAIL: L12 VM created for a conflicting PR"; failures=$((failures + 1)); else echo "ok:   L12 no VM for a conflicting PR"; fi

check_repo; got=0; BOXD_MAX_VMS=4 STUB_MODE=full run_check_ref 54 || got=$?
expect_true "L12 BOXD_MAX_VMS=4 refuses a check VM" test "$got" -eq 1

# L12: `bake` runs isolated, warms from the lockfile and refuses to save a snapshot that holds a token.
bake_repo() { new_repo; printf '[toolchain]\nchannel = "1.89"\n' >rust-toolchain.toml; echo '{"packageManager":"pnpm@10.0.0"}' >package.json; git add rust-toolchain.toml package.json; git commit -qm toolchain; }
bake_repo; got=0; loop/boxd.sh bake >out 2>err || got=$?
expect_true "L12 bake succeeds" test "$got" -eq 0
expect_log 'snapshots save ru-bake ru-toolchain' "L12 bake saves the snapshot"
expect_log 'machine new ru-bake --isolated .*--auto-destroy-timeout 7200' "L12 the bake VM is isolated and has a TTL"
expect_log 'grep -rIlE .*gho_' "L12 the bake scan looks for GitHub OAuth tokens"
expect_log '--frozen-lockfile' "L12 the bake warms dependencies from the lockfile"
expect_log 'machine remove ru-bake' "L12 the bake VM is destroyed"
expect_log 'cargo clean -p' "L12 the bake cleans every workspace member from the warm target"
expect_log 'grep -rlF /tmp/warm' "L12 the bake asserts nothing under the warm target names the bake checkout"
bake_repo; got=0; STUB_MODE=warm-dirty loop/boxd.sh bake >out 2>err || got=$?
expect_true "L12 a warm target that still names the bake checkout refuses the snapshot" test "$got" -eq 1
if grep -q 'snapshots save' log; then echo "FAIL: L12 snapshot saved with a dirty warm target"; failures=$((failures + 1)); else echo "ok:   L12 no snapshot saved with a dirty warm target"; fi
bake_repo; loop/boxd.sh bake >out 2>err
bake_repo; got=0; STUB_MODE=bake-leak loop/boxd.sh bake >out 2>err || got=$?
expect_true "L12 a token in the bake VM refuses the snapshot" test "$got" -eq 1
if grep -q 'snapshots save' log; then echo "FAIL: L12 snapshot saved with a token"; failures=$((failures + 1)); else echo "ok:   L12 no snapshot saved with a token"; fi

[ "$failures" -eq 0 ] || { echo "$failures failed"; exit 1; }
