#!/usr/bin/env bash
# Tests for loop/boxd.sh against a stub `boxd`. Usage: loop/boxd.test.sh
set -euo pipefail

script="$(cd "$(dirname "$0")" && pwd)/boxd.sh"
rules_script="$(cd "$(dirname "$0")" && pwd)/rules.sh"
failures=0

new_repo() {
  dir=$(mktemp -d)
  cd "$dir"
  git init -q -b main
  git config user.email t@t
  git config user.name t
  mkdir loop bin
  cp "$script" loop/boxd.sh
  cp "$rules_script" loop/rules.sh
  echo x >f
  git add -A
  git commit -qm base
  git update-ref refs/remotes/origin/main HEAD
  echo 'do the thing' >prompt.md
  # The stub logs every call; `exec ... claude` answers per $STUB_MODE.
  cat >bin/boxd <<'S'
#!/usr/bin/env bash
echo "$*" >>"$STUB_LOG"
timeout_s=1800
prev=""
for a in "$@"; do [ "$prev" != --timeout ] || timeout_s=$a; prev=$a; done
if [ "$1 $2 $3" = "machine cp -" ]; then vm=${4%%:*}; mkdir -p "$STUB_DIR/by-vm/$vm"; tee "$STUB_DIR/by-vm/$vm/$(basename "$4")" >"$STUB_DIR/$(basename "$4")"; else cat >/dev/null 2>&1 || true; fi
case "$1 $2" in
  "env list") if [ "${STUB_MODE:-}" = no-secret ]; then echo '[]'; else echo '[{"name":"CLAUDE_CODE_OAUTH_TOKEN"}]'; fi ;;
  "machine list")
    if [ -n "${STUB_BUSY_LISTS:-}" ] && { echo x >>"$STUB_DIR/listcalls"; [ "$(wc -l <"$STUB_DIR/listcalls")" -le "$STUB_BUSY_LISTS" ]; }; then jq -nc --argjson n "${BOXD_MAX_VMS:-12}" '[range($n) | {name: "ru-other-\(.)"}]'
    elif [ -n "${STUB_RU:-}" ]; then jq -nc --argjson n "$STUB_RU" '[range($n) | {name: "ru-\(.)"}] + [{name: "db"}, {name: "web-1"}, {name: "ru"}]'
    elif [ "${STUB_MODE:-}" = full ]; then echo '[{"name":"ru-1"},{"name":"ru-2"},{"name":"ru-3"},{"name":"ru-4"}]'
    elif [ "${STUB_MODE:-}" = mixed ]; then echo '[{"name":"ru-builder-1"},{"name":"ru-reviewer-2"},{"name":"ru-x8-1"},{"name":"ru-builderx"},{"name":"ru-x-builder-9"},{"name":"ru-builder-x"},{"name":"ru-builder-1x"},{"name":"ru-builder-3"}]'
    else (cd "$STUB_DIR" && ls alive-* 2>/dev/null || true) | jq -Rnc '[inputs | {name: sub("^alive-"; "")}]'; fi ;;
  "machine new")
    [ "${STUB_MODE:-}" != exists ] && [ ! -e "$STUB_DIR/alive-$3" ] || exit 1
    touch "$STUB_DIR/alive-$3"; ls "$STUB_DIR"/alive-* | wc -l | tr -d " " >>"$STUB_DIR/maxlog" ;;
  "machine reboot") ;;
  "machine remove")
    if [ "${STUB_MODE:-}" = remove-fails ] || [ "${STUB_FAIL_REMOVE:-}" = "$3" ]; then exit 1; fi
    rm -f "$STUB_DIR/alive-$3"
    if [ -e "$STUB_DIR/exec-$3.pid" ]; then kill "$(cat "$STUB_DIR/exec-$3.pid")" 2>/dev/null || true; fi ;;
  "machine exec")
    case "$*" in
      *" -- true")
        echo x >>"$STUB_DIR/trues"
        [ "$(wc -l <"$STUB_DIR/trues")" -gt "${STUB_NOANSWER:-0}" ] || exit 1 ;;
      *"tar xzf /tmp/base.tgz"* | *"git fetch -q /tmp/r.bundle"*)
        echo x >>"$STUB_DIR/tars"
        if [ "$(wc -l <"$STUB_DIR/tars")" -le "${STUB_TARFAIL:-0}" ]; then sleep "${STUB_TARFAIL_SLEEP:-0}"; exit 2; fi ;;
      *pgrep*)
        case "$3" in ru-builder-1) echo agent-running ;; ru-reviewer-2) echo idle ;; *) exit 1 ;; esac ;;
      *claude*)
        case "${STUB_MODE:-}" in
          no-output) exit 1 ;;
          deadline) sleep "${STUB_CLAUDE_SLEEP:-0}"; echo 'exec error: status: DeadlineExceeded, message: "no output from the command for 30m: wedged"' >&2; exit 1 ;;
          limit) echo '{"type":"result","is_error":true,"api_error_status":429,"result":"x"}'; exit 1 ;;
          events-forever)
            echo $$ >"$STUB_DIR/exec-$3.pid"
            timeout "$timeout_s" bash -c 'i=0; while :; do i=$((i + 1)); echo "{\"type\":\"progress\",\"n\":$i}"; sleep 1; done' ;;
          events-forever-ignoring-boxd-timeout)
            # Unlike events-forever, this never stops itself: it stands in for a real boxd whose --timeout is a
            # no-output deadline that a steady stream of events never trips, so only our own wall-clock cutoff ends it.
            echo $$ >"$STUB_DIR/exec-$3.pid"
            i=0; while :; do i=$((i + 1)); echo "{\"type\":\"progress\",\"n\":$i}"; sleep 1; done ;;
          events-then-exit)
            echo '{"type":"progress","n":1}' ;;
          progress-then-result)
            echo '{"type":"progress","n":1}'
            jq -cn --arg r "final answer" '{type:"result",is_error:false,result:$r,num_turns:7,duration_ms:5000,total_cost_usd:1.23}' ;;
          limit-then-more)
            echo '{"type":"result","is_error":true,"api_error_status":429,"result":"x"}'
            echo '{"type":"system","subtype":"turn_end"}' ;;
          retry-429-then-ok)
            echo '{"type":"system","subtype":"turn_end","api_error_status":429}'
            jq -cn --arg r ok '{type:"result",is_error:false,result:$r,num_turns:1,duration_ms:1000,total_cost_usd:0.1}' ;;
          error-no-limit)
            jq -cn --arg r boom '{type:"result",is_error:true,result:$r}' ;;
          ok-quotes-limit)
            phrase="usage"" limit and rate"" limit"
            jq -cn --arg r "ok, mentions the $phrase case but succeeded" '{type:"result",is_error:false,result:$r,num_turns:1,duration_ms:1000,total_cost_usd:0.1}' ;;
          limit-text)
            phrase="usage"" limit"
            jq -cn --arg r "$phrase hit" '{type:"result",is_error:true,result:$r}'
            exit 1 ;;
          *)
            echo $$ >"$STUB_DIR/exec-$3.pid"
            echo '{"type":"system","subtype":"init"}'
            sleep "${STUB_CLAUDE_SLEEP:-0}"
            echo '{"type":"result","is_error":false,"result":"ok","num_turns":1,"duration_ms":1000,"total_cost_usd":0.1}' ;;
        esac ;;
      *"grep -rlF /tmp/warm"*) [ "${STUB_MODE:-}" != warm-dirty ] || exit 1 ;;
      *"grep -rIlE"*) [ "${STUB_MODE:-}" = bake-leak ] || exit 1 ;;
      *"just check"*)
        if [ -n "${STUB_FS_FIXTURE:-}" ]; then
          cmd=${!#}
          rewritten=$(printf '%s' "$cmd" | sed "s#/node_modules#$STUB_FS_FIXTURE/node_modules#g")
          # If boxd.sh ever stops naming the stray path literally, the sed above silently no-ops: refuse rather
          # than run an unrewritten "sudo rm -rf /node_modules" against the real filesystem.
          [ "$rewritten" != "$cmd" ] || { echo "boxd stub: command has no /node_modules to rewrite into the fixture; refusing to run it unmodified" >&2; exit 1; }
          HOME="$STUB_FS_FIXTURE/home" PATH="$STUB_FS_FIXTURE/bin:$PATH" FAKE_TSC_ROOT="$STUB_FS_FIXTURE" bash -c "$rewritten"
        else
          case "${STUB_MODE:-}" in check-fails) exit 7 ;; check-deadline) echo 'exec error: status: DeadlineExceeded, message: "no output from the command for 30m: wedged"' >&2; exit 1 ;; token-output) echo "log gho_abcdefghijklmnopqrstuvwxyz0123 end" ;; esac
        fi ;;
      *"git diff --cached base"*) echo "partial-diff-against-base" ;;
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

new_repo; BOXD_AGENT_TIMEOUT=900 loop/boxd.sh review r prompt.md >out 2>err
expect_log 'machine exec ru-r --timeout 900 .*claude' "L5 BOXD_AGENT_TIMEOUT sets the agent timeout"
new_repo; BOXD_AGENT_TIMEOUT=x STUB_MODE='' expect_code 2 "L5 non-numeric BOXD_AGENT_TIMEOUT is refused"
new_repo; BOXD_AGENT_TIMEOUT=1801 STUB_MODE='' expect_code 2 "L5 BOXD_AGENT_TIMEOUT above 1800 would outlast the VM timer and is refused"

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

# The Reviewer's checkout: tag base is the merge-base, HEAD is the ref, carried as a real git bundle (L22).
new_repo; git switch -qc feat; echo y >g; git add g; git commit -qm feat
loop/boxd.sh review r prompt.md feat >out 2>err
expect_true "L9 the real history is bundled by name, not a raw SHA" bash -c "git bundle list-heads cp/r.bundle | grep -q 'refs/boxd-review/r-ref\$' && git bundle list-heads cp/r.bundle | grep -q 'refs/boxd-review/r-base\$'"
expect_log 'git checkout -q --detach' "L9 the VM checks out the ref's own commit"
expect_log 'git tag base' "L9 the VM tags base"
expect_true "L9 no temporary bundle ref is left behind" test -z "$(git for-each-ref refs/boxd-review)"

# Run the exact script a given VM executed (taken from the stub's log) against a real repo in <work>, with
# /tmp and ~ redirected, so the test proves what git on the VM actually does with the bundle it was given.
replay_on_vm() { # replay_on_vm <work-dir> [vm-name, default ru-r]
  local w=$1 vm=${2:-ru-r}
  mkdir -p "$w"
  cp "cp/by-vm/$vm/r.bundle" "$w/r.bundle" 2>/dev/null || cp cp/r.bundle "$w/r.bundle"
  grep "^machine exec $vm -- mkdir -p ~/roundup" log | tail -1 | sed -e "s#^machine exec $vm -- ##" -e "s#/tmp/#$w/#g" >"$w/replay.sh"
  HOME=$w bash "$w/replay.sh" 2>"$w/replay.err"
}
vm_log() { git -C "$1/roundup" log --format="$2" base..HEAD; }

new_repo; git switch -qc feat; echo y >g; git add g; git commit -qm feat
loop/boxd.sh review r prompt.md feat >out 2>err
replay_on_vm "$dir/vm"
expect_true "L9 base's tree has no feature file" bash -c "! git -C '$dir/vm/roundup' cat-file -e base:g 2>/dev/null"
expect_true "L9 the ref's tree lands at HEAD (has the feature file)" test -e "$dir/vm/roundup/g"

# base has its own parent here, so a base that landed one commit off (tagging the merge-base's parent instead
# of the merge-base itself) would be a different real commit, not just a tree that happens to look the same.
new_repo; echo second >main2; git add main2; git commit -qm second; git update-ref refs/remotes/origin/main HEAD
git switch -qc feat; echo y >g; git add g; git commit -qm feat
loop/boxd.sh review r prompt.md feat >out 2>err
replay_on_vm "$dir/vm"
expect_true "L9 HEAD is pinned to the ref's own commit hash, not a replay" test "$(git -C "$dir/vm/roundup" rev-parse HEAD)" = "$(git rev-parse feat)"
expect_true "L9 base is pinned to the real merge-base hash, not an ancestor of it" test "$(git -C "$dir/vm/roundup" rev-parse base)" = "$(git merge-base origin/main feat)"

# A merge commit in the range is bundled too: it reaches the VM as a real commit with its own two parents,
# not replayed as a non-merge series plus a stood-in resolution commit (L9's merge clause).
new_repo; git switch -qc feat; echo y >g; git add g; git commit -qm feat
git switch -q main; echo z >h; git add h; git -c user.name=bob -c user.email=b@b commit -qm other
git update-ref refs/remotes/origin/main main
git switch -q feat; git merge -q --no-edit main
loop/boxd.sh review r prompt.md feat >out 2>err
replay_on_vm "$dir/vm"
expect_true "L9 the ref's own merge commit lands at HEAD unchanged" test "$(git -C "$dir/vm/roundup" rev-parse HEAD)" = "$(git rev-parse feat)"
expect_true "L9 a merge commit in the range keeps its own two parents" test "$(git -C "$dir/vm/roundup" rev-list --merges base..HEAD | wc -l | tr -d ' ')" = 1

new_repo; git switch -qc feat; echo y >g; git add g
git -c user.name=alice -c user.email=a@a commit -qm 'feat

Author-Agent: alice-agent'
loop/boxd.sh review r prompt.md feat >out 2>err
replay_on_vm "$dir/vm"
expect_true "L9 the replay keeps the author and the trailer" test "$(vm_log "$dir/vm" '%an|%s|%(trailers:key=Author-Agent,valueonly)')" = "alice|feat|alice-agent"
expect_true "L9 the replay adds no synthetic head commit" test "$(vm_log "$dir/vm" '%s')" = feat

# A branch that already ends in an empty approval commit replays whole: a bundle carries every real commit,
# empty or not, so the empty commit needs no special handling.
new_repo; git switch -qc feat; echo y >g; git add g; git commit -qm feat; git commit -q --allow-empty -m 'review: approve'
loop/boxd.sh review r prompt.md feat >out 2>err
replay_on_vm "$dir/vm"
expect_true "L16 the VM keeps the empty approval commit on top" test "$(vm_log "$dir/vm" '%s' | paste -sd, -)" = "review: approve,feat"
expect_true "L16 the approval commit is still empty on the VM" test -z "$(git -C "$dir/vm/roundup" diff-tree --no-commit-id --name-only -r HEAD)"

# L22: review carries the branch's real history, so a merge, its author and its trailers reach the VM exactly
# as they are on the real branch, and loop/rules.sh trailers base gives the same verdict on both.
norm_hashes() { sed -E 's/\b[0-9a-f]{7,40}\b/<hash>/g'; }
# merge_trailers_repo [no-alice-trailer] [no-fixup]: the shape PR #113 itself has. feat (alice, Author-Agent
# unless asked to omit it) and the current origin/main (bob, Author-Agent) both change the same line of f; feat
# merges that advanced origin/main, the merge conflicts, and (unless no-fixup) dave's follow-up fixup commit (no
# trailer of its own — the bug in finding 1) finishes it, then an empty approval commit (carol, Reviewed-by-Agent).
# Bob's commit becomes the merge-base, so it is upstream of the range, same as any commit already on main before
# the PR branched. Without the fixup commit the merge commit (excluded from trailers by --no-merges) is the only
# place dave's resolution lands, so the range is clean: alice's trailer and carol's approval are all it checks.
merge_trailers_repo() {
  local alice_trailer=$'\n\nAuthor-Agent: alice-agent' skip_fixup=0 arg
  for arg in "$@"; do
    [ "$arg" != no-alice-trailer ] || alice_trailer=''
    [ "$arg" != no-fixup ] || skip_fixup=1
  done
  git switch -qc feat
  echo alice-version >f
  git -c user.name=alice -c user.email=a@a commit -qam "feat$alice_trailer"
  git switch -q main
  echo bob-version >f
  git -c user.name=bob -c user.email=b@b commit -qam 'other

Author-Agent: bob-agent'
  git update-ref refs/remotes/origin/main main
  git switch -q feat
  git merge -q --no-edit main || true
  echo resolved >f
  git add f
  git -c user.name=dave -c user.email=d@d commit -q --no-edit
  if [ "$skip_fixup" -eq 0 ]; then
    echo more >>f
    git -c user.name=dave -c user.email=d@d commit -qam 'fixup after merge'
  fi
  git commit -q --allow-empty -m 'review: approve

Reviewed-by-Agent: carol-agent'
  git tag base "$(git merge-base origin/main feat)"
}

new_repo; merge_trailers_repo
loop/boxd.sh review r prompt.md feat >out 2>err
replay_on_vm "$dir/vm"
expect_true "L22 the checkout tree still matches the ref" test "$(cat "$dir/vm/roundup/f")" = "$(git show feat:f)"
expect_true "L22 the approval commit is still newest" test "$(vm_log "$dir/vm" '%s' | head -1)" = "review: approve"
expect_true "L22 no head commit is invented" bash -c "! git -C '$dir/vm/roundup' log --format=%s base..HEAD | grep -qx head"
real_rc=0; real_out=$(loop/rules.sh trailers base 2>&1) || real_rc=$?
vm_rc=0; vm_out=$(cd "$dir/vm/roundup" && loop/rules.sh trailers base 2>&1) || vm_rc=$?
expect_true "L22 the fixup commit's missing trailer is rejected on the real branch" test "$real_rc" != 0
expect_true "L22 the VM's trailers exit code matches the real branch" test "$vm_rc" = "$real_rc"
expect_true "L22 the VM's trailers findings match the real branch" test "$(norm_hashes <<<"$vm_out")" = "$(norm_hashes <<<"$real_out")"

new_repo; merge_trailers_repo no-alice-trailer
loop/boxd.sh review r prompt.md feat >out 2>err
replay_on_vm "$dir/vm"
real_rc=0; real_out=$(loop/rules.sh trailers base 2>&1) || real_rc=$?
vm_rc=0; vm_out=$(cd "$dir/vm/roundup" && loop/rules.sh trailers base 2>&1) || vm_rc=$?
expect_true "L22 two commits missing Author-Agent are rejected on the real branch" test "$real_rc" != 0
expect_true "L22 the VM rejects it the same way" test "$vm_rc" = "$real_rc"
expect_true "L22 the rejection matches once hashes are normalized" test "$(norm_hashes <<<"$vm_out")" = "$(norm_hashes <<<"$real_out")"

# A conflicted merge is not itself a rejection: with no fixup commit on top, trailers passes on both sides, so a
# checkout that always rejected a merge-shaped range the same way would not pass this one.
new_repo; merge_trailers_repo no-fixup
loop/boxd.sh review r prompt.md feat >out 2>err
replay_on_vm "$dir/vm"
real_rc=0; real_out=$(loop/rules.sh trailers base 2>&1) || real_rc=$?
vm_rc=0; vm_out=$(cd "$dir/vm/roundup" && loop/rules.sh trailers base 2>&1) || vm_rc=$?
expect_true "L22 a conflicted merge with no fixup commit passes trailers on the real branch" test "$real_rc" = 0
expect_true "L22 the VM passes it the same way" test "$vm_rc" = "$real_rc"
expect_true "L22 the pass matches once hashes are normalized" test "$(norm_hashes <<<"$vm_out")" = "$(norm_hashes <<<"$real_out")"
expect_log '--auto-destroy-timeout 4200' "L5 VM has an auto-destroy timer"
expect_log 'machine exec ru-r --timeout 1800 .*claude' "L5 the agent may run as long as the check"
expect_log 'machine reboot ru-r' "L11 VM is rebooted after restore"

# L15: `check` merges on this machine and runs on an isolated VM. The origin lives beside the repo, not inside it.
check_repo() {
  new_repo
  git init -q -b main --bare "$dir-origin.git"
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
expect_true "L15 check of a PR number succeeds" test "$got" -eq 0
expect_log 'machine new ru-chk-54-[0-9]+ .*--isolated' "L15 the check VM is isolated"
expect_log 'machine new ru-chk-54-[0-9]+ .*--auto-destroy-timeout 4200' "L15 the check VM has an auto-destroy timer"
expect_log 'machine remove ru-chk-54-[0-9]+' "L15 VM destroyed after success"
got=0; run_check_ref 54 || got=$?
expect_true "L17 two checks of one ref use two different VM names" test "$(grep -oE 'machine new ru-chk-54-[0-9]+' log | sort -u | wc -l | tr -d ' ')" = 2
expect_true "L15 nothing is cloned or fetched on the VM" bash -c '! grep -qE "git (clone|fetch)" log'
expect_true "L15 the check installs from the lockfile" grep -q -- '--frozen-lockfile' log
expect_true "L15 the ref reaches git fetch after --" grep -q '^fetch -q origin -- +refs/heads/main:refs/boxd-check/54-base +pull/54/head:refs/boxd-check/54$' "$STUB_GIT_LOG"
expect_true "L15 no ref is left behind by the fetch" test -z "$(git for-each-ref refs/boxd-check)"

# The upload is origin/main as it is on origin now (not the local HEAD, not the stale tracking ref) plus the PR.
check_repo; echo local >local-only; git add local-only; git commit -qm local-only
git checkout -q -b pr-work origin/main; echo pr >pr-marker; git add pr-marker; git commit -qm pr; git push -q origin HEAD:refs/pull/54/head; git checkout -q main
git clone -q "$dir-origin.git" "$dir-other"; (cd "$dir-other"; echo fresh >fresh-main; git add fresh-main; git -c user.email=t@t -c user.name=t commit -qm fresh; git push -q origin HEAD:main)
got=0; run_check_ref 54 || got=$?
expect_true "L15 check succeeds with a local HEAD that differs from origin/main" test "$got" -eq 0
expect_true "L15 the upload holds the PR's change" bash -c 'tar tzf cp/src.tgz | grep -qx pr-marker'
expect_true "L15 the upload holds origin/main as fetched now" bash -c 'tar tzf cp/src.tgz | grep -qx fresh-main'
expect_true "L15 the upload leaves out local-only commits" bash -c '! tar tzf cp/src.tgz | grep -qx local-only'
expect_true "L15 the base is origin/main without the PR" bash -c 'tar tzf cp/base.tgz | grep -qx fresh-main && ! tar tzf cp/base.tgz | grep -qx pr-marker'

# The VM name is the ref lower-cased, non-alphanumerics as dashes, at most 30 characters.
check_repo; git push -q origin HEAD:refs/heads/My_Branch HEAD:refs/heads/builder/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
run_check_ref My_Branch || true
expect_log 'machine new ru-chk-my-branch-[0-9]+ ' "L15 the VM name is the ref lower-cased with dashes"
: >log; run_check_ref builder/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa || true
expect_log 'machine new ru-chk-builder-aaaaaaaaaaaaaaaaaaaaaa-[0-9]+ ' "L15 the VM name is cut at 30 characters"

check_repo; got=0; STUB_MODE=check-deadline run_check_ref builder/x || got=$?
expect_true "L19 a check run that hits the deadline is recorded with VM and phase" grep -qE ' ru-chk-builder-x-[0-9]+ check deadline-exceeded' loop/out/events.log
check_repo; got=0; STUB_MODE=check-fails run_check_ref builder/x || got=$?
expect_true "L15 failed check keeps its exit code" test "$got" -eq 7
expect_log 'machine remove ru-chk-builder-x-[0-9]+' "L15 VM destroyed after a failed check"

check_repo; got=0; STUB_MODE=token-output run_check_ref 54 || got=$?
expect_true "L15 a GitHub token in the check output is masked" bash -c '! grep -rq gho_abcdefgh err loop/out/runs'

check_repo; got=0; STUB_MODE=no-secret run_check_ref 54 || got=$?
expect_true "L15 check needs no Claude secret" test "$got" -eq 0

# L40: the check must not depend on anything outside the checkout (see STRAY_NODE_MODULES in boxd.sh for why the stray
# directory exists). The stub replays boxd.sh's own "just check" command for real (sed-rewriting the literal
# /node_modules path into the fixture, like L9's replay_on_vm rewrites /tmp/) against a two-package pnpm workspace
# fixture below a planted node_modules/@types/node. `pnpm`, `just` and `tsc` are faked on PATH so the ubuntu-latest
# runner, which has none of the three installed, never needs them for real: the fake `pnpm` only links a package's
# own node_modules/@types/node when its package.json declares the dependency, the fake `just` runs a justfile
# recipe's body, and the fake `tsc` walks up to FAKE_TSC_ROOT exactly as the real ambient-@types lookup does.
build_l40_fixture() { # build_l40_fixture <dir> <declared: yes|no>
  local f=$1 declared=$2
  mkdir -p "$f/home/roundup/packages/pkg-a/src" "$f/home/roundup/packages/types-node" "$f/node_modules/@types/node" "$f/bin" "$f/home/.cargo"
  touch "$f/node_modules/@types/node/index.d.ts" "$f/home/.cargo/env"
  printf '{"name":"fixture-workspace","private":true}\n' >"$f/home/roundup/package.json"
  printf 'packages:\n  - "packages/*"\n' >"$f/home/roundup/pnpm-workspace.yaml"
  printf 'check:\n    pnpm -r --if-present typecheck\n' >"$f/home/roundup/justfile"
  if [ "$declared" = yes ]; then
    printf '{"name":"pkg-a","private":true,"version":"0.0.0","scripts":{"typecheck":"tsc"},"devDependencies":{"@types/node":"workspace:*"}}\n' >"$f/home/roundup/packages/pkg-a/package.json"
  else
    printf '{"name":"pkg-a","private":true,"version":"0.0.0","scripts":{"typecheck":"tsc"}}\n' >"$f/home/roundup/packages/pkg-a/package.json"
  fi
  printf 'export function readEnv() { return process.env.X }\n' >"$f/home/roundup/packages/pkg-a/src/index.ts"
  printf '{"name":"@types/node","private":true,"version":"0.0.0"}\n' >"$f/home/roundup/packages/types-node/package.json"
  cat >"$f/bin/tsc" <<'TSC'
#!/usr/bin/env bash
set -euo pipefail
dir=$PWD
found=0
while :; do
  [ -d "$dir/node_modules/@types/node" ] && { found=1; break; }
  [ "$dir" = "$FAKE_TSC_ROOT" ] && break
  dir=$(dirname "$dir")
done
if [ "$found" -eq 0 ] && grep -q 'process\.' src/index.ts 2>/dev/null; then
  echo "$PWD/src/index.ts(1,1): error TS2580: Cannot find name 'process'. Do you need to install type definitions for node?" >&2
  exit 2
fi
echo "tsc: no errors"
TSC
  chmod +x "$f/bin/tsc"
  cat >"$f/bin/pnpm" <<'PNPM'
#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  install)
    for pkg in packages/*/package.json; do
      jq -e '.devDependencies["@types/node"] // empty' "$pkg" >/dev/null 2>&1 || continue
      mkdir -p "$(dirname "$pkg")/node_modules/@types"
      ln -sfn "$(cd "$(dirname "$pkg")/../types-node" && pwd)" "$(dirname "$pkg")/node_modules/@types/node"
    done
    ;;
  -r)
    shift
    [ "${1:-}" != --if-present ] || shift
    script=$1
    for pkg in packages/*/package.json; do
      cmd=$(jq -r --arg s "$script" '.scripts[$s] // empty' "$pkg")
      [ -n "$cmd" ] || continue
      (cd "$(dirname "$pkg")" && eval "$cmd")
    done
    ;;
esac
PNPM
  chmod +x "$f/bin/pnpm"
  cat >"$f/bin/just" <<'JUST'
#!/usr/bin/env bash
set -euo pipefail
recipe=$1
mapfile -t lines < <(awk -v r="$recipe:" '
  $0 == r { found=1; next }
  found && NF && $0 !~ /^[ \t]/ { found=0 }
  found { sub(/^[ \t]+/, ""); print }
' justfile)
for line in "${lines[@]}"; do eval "$line"; done
JUST
  chmod +x "$f/bin/just"
  # Root-owned, like the stray directory actually found on the VM: a fix that drops `sudo rm` would leave this in place.
  # sudo and chown are only ever applied under $f, a mktemp'd directory, never the host's real /node_modules.
  sudo chown -R root:root "$f/node_modules"
}
l40_repo() { check_repo; fixture="$dir/fixture"; build_l40_fixture "$fixture" "$1"; }

l40_repo no
got=0; STUB_FS_FIXTURE="$fixture" run_check_ref 54 || got=$?
expect_true "L40 an undeclared package fails when the stray node_modules is removed" test "$got" -ne 0
l40_log=$(ls loop/out/runs/check-54-*.log)
expect_true "L40 the saved log holds the tsc error naming the package's file" grep -q 'packages/pkg-a/src/index.ts.*error TS2580' "$l40_log"

l40_repo yes
got=0; STUB_FS_FIXTURE="$fixture" run_check_ref 54 || got=$?
expect_true "L40 the same tree passes once the package declares @types/node" test "$got" -eq 0

# The stub's own safety net: if a command ever reaches it with no /node_modules to rewrite, it must refuse rather
# than run an unrewritten "sudo rm -rf /node_modules" against the real filesystem.
got=0; STUB_FS_FIXTURE="$fixture" boxd machine exec ru-x --timeout 1800 -- "cd ~/roundup && just check" >out 2>err || got=$?
expect_true "L40 the stub refuses a command with no /node_modules to rewrite" test "$got" -eq 1
expect_true "L40 the refusal names why" grep -q 'refusing to run it unmodified' err

for bad_ref in 'x; rm -rf ~' -x --tags; do
  check_repo; got=0; run_check_ref "$bad_ref" || got=$?
  expect_true "L15 the ref '$bad_ref' is refused" test "$got" -eq 2
  if grep -q 'machine new' log; then echo "FAIL: L15 VM created for '$bad_ref'"; failures=$((failures + 1)); fi
done

check_repo; git checkout -q -b clash origin/main; echo a >f; git commit -qam a; git push -q origin HEAD:refs/heads/builder/clash
git checkout -q main; echo b >f; git commit -qam b; git push -q origin HEAD:main
got=0; run_check_ref builder/clash || got=$?
expect_true "L15 a PR that conflicts with main stops before any VM" test "$got" -eq 1
expect_true "L15 the conflict is reported with the ref and the file" bash -c 'grep -q "builder/clash conflicts with origin/main" err && grep -q "CONFLICT" err'
if grep -q 'machine new' log; then echo "FAIL: L15 VM created for a conflicting PR"; failures=$((failures + 1)); else echo "ok:   L15 no VM for a conflicting PR"; fi

check_repo; got=0; BOXD_MAX_VMS=4 STUB_MODE=full run_check_ref 54 || got=$?
expect_true "L15 BOXD_MAX_VMS=4 refuses a check VM" test "$got" -eq 1

# L15: `bake` runs isolated, warms from the lockfile and refuses to save a snapshot that holds a token.
bake_repo() { new_repo; printf '[toolchain]\nchannel = "1.89"\n' >rust-toolchain.toml; echo '{"packageManager":"pnpm@10.0.0"}' >package.json; git add rust-toolchain.toml package.json; git commit -qm toolchain; }
bake_repo; got=0; loop/boxd.sh bake >out 2>err || got=$?
expect_true "L15 bake succeeds" test "$got" -eq 0
expect_log 'snapshots save ru-bake ru-toolchain' "L15 bake saves the snapshot"
expect_log 'machine new ru-bake --isolated .*--auto-destroy-timeout 7200' "L15 the bake VM is isolated and has a TTL"
expect_log 'grep -rIlE .*gho_' "L15 the bake scan looks for GitHub OAuth tokens"
expect_log 'grep -rIlE .*ghp_.*github_pat_.*sk-ant-' "L15 the bake scan looks for GitHub and Anthropic tokens"
expect_log 'NEEDRESTART_SUSPEND=1' "L15 the bake keeps apt from restarting the boxd agent"
expect_log '--frozen-lockfile' "L15 the bake warms dependencies from the lockfile"
expect_log 'machine remove ru-bake' "L15 the bake VM is destroyed"
expect_log 'cargo clean -p' "L15 the bake cleans every workspace member from the warm target"
expect_log 'grep -rlF /tmp/warm' "L15 the bake asserts nothing under the warm target names the bake checkout"
bake_repo; got=0; STUB_MODE=warm-dirty loop/boxd.sh bake >out 2>err || got=$?
expect_true "L15 a warm target that still names the bake checkout refuses the snapshot" test "$got" -eq 1
if grep -q 'snapshots save' log; then echo "FAIL: L15 snapshot saved with a dirty warm target"; failures=$((failures + 1)); else echo "ok:   L15 no snapshot saved with a dirty warm target"; fi
bake_repo; loop/boxd.sh bake >out 2>err
bake_repo; got=0; STUB_MODE=bake-leak loop/boxd.sh bake >out 2>err || got=$?
expect_true "L15 a token in the bake VM refuses the snapshot" test "$got" -eq 1
if grep -q 'snapshots save' log; then echo "FAIL: L15 snapshot saved with a token"; failures=$((failures + 1)); else echo "ok:   L15 no snapshot saved with a token"; fi

# L17: every run keeps its own log, named by VM name, UTC time and pid, so a relaunch never overwrites a failed run's log.
new_repo; loop/boxd.sh build t prompt.md >out 2>err; loop/boxd.sh build t prompt.md >out 2>err
expect_true "L17 two runs with one name keep two run logs" test "$(printf '%s\n' loop/out/runs/* | grep -cE '/t-[0-9]{8}T[0-9]{6}Z-[0-9]+\.jsonl$')" = 2
expect_true "L17 two runs with one name keep two check logs" test "$(printf '%s\n' loop/out/runs/* | grep -cE '/t-[0-9]{8}T[0-9]{6}Z-[0-9]+\.check\.log$')" = 2
expect_true "L17 the patch keeps its stable name" test -e loop/out/patches/t.patch

# L18: a run waits for a free slot below BOXD_MAX_VMS when asked to, and says so when none frees.
new_repo; BOXD_MAX_VMS=2 BOXD_SLOT_WAIT=10 STUB_BUSY_LISTS=3 STUB_MODE='' expect_code 0 "L18 a run waits for a slot taken by an outside VM"
new_repo; BOXD_MAX_VMS=2 BOXD_SLOT_WAIT=2 STUB_BUSY_LISTS=99 STUB_MODE='' expect_code 1 "L18 a wait that never ends exits 1"
expect_true "L18 the failed wait names the cap" grep -q 'cap BOXD_MAX_VMS=2' err
if grep -q 'machine new' log; then echo "FAIL: L18 VM created without a slot"; failures=$((failures + 1)); else echo "ok:   L18 no VM without a slot"; fi
started=$SECONDS; new_repo; BOXD_MAX_VMS=2 STUB_BUSY_LISTS=99 STUB_MODE='' expect_code 1 "L18 a single run does not wait by default"
expect_true "L18 the default wait is 0 s, not a pause" test $((SECONDS - started)) -lt 3
new_repo; BOXD_SLOT_WAIT=x STUB_MODE='' expect_code 2 "L18 non-numeric BOXD_SLOT_WAIT is refused"
new_repo; export BOXD_LOCK_WAIT=30; echo p >p2.md
BOXD_MAX_VMS=2 STUB_BUSY_LISTS=4 loop/boxd.sh swarm build prompt.md p2.md >out 2>err || true
expect_true "L18 swarm children wait for a slot an outside VM holds" test "$(grep -c ' ok$' out)" = 2

# L19: a VM that never answers after its reboot, or a failed upload in the first seconds, is retried once on a fresh VM, before the agent runs.
count_log() { grep -c -- "$1" log || true; }
new_repo; BOXD_REBOOT_ATTEMPTS=2 STUB_NOANSWER=2 STUB_MODE='' expect_code 0 "L19 a VM that never answers is retried once"
expect_true "L19 the retry made a second VM" test "$(count_log 'machine new ru-t ')" = 2
expect_true "L19 the first VM was destroyed before the retry" test "$(count_log 'machine remove ru-t')" = 2
expect_true "L19 the retry is visible in the output" grep -q 'did not answer after reboot; retrying once' err
expect_true "L19 the retry is recorded with the VM and phase" grep -q ' ru-t provision ru-t did not answer after reboot' loop/out/events.log
new_repo; BOXD_REBOOT_ATTEMPTS=3 STUB_NOANSWER=99 STUB_MODE='' expect_code 1 "L19 a VM that never answers twice fails"
expect_true "L19 a failed retry gives up after two VMs" test "$(count_log 'machine new ru-t ')" = 2
expect_true "L19 each VM is asked for exactly BOXD_REBOOT_ATTEMPTS answers" test "$(count_log ' -- true$')" = 6
expect_true "L19 the give-up is named" grep -q 'giving up after one retry' err
new_repo; STUB_NOANSWER=5 STUB_MODE='' expect_code 0 "L19 a VM that answers on its sixth try needs no retry by default"
expect_true "L19 the default allows more than three attempts" test "$(count_log 'machine new ru-t ')" = 1
new_repo; BOXD_REBOOT_ATTEMPTS=61 STUB_MODE='' expect_code 2 "L19 more than 60 reboot attempts is refused"
new_repo; BOXD_REBOOT_ATTEMPTS=0 STUB_MODE='' expect_code 2 "L19 zero reboot attempts is refused"
new_repo; STUB_TARFAIL=1 STUB_MODE='' expect_code 0 "L19 a tar failure in the first seconds is retried once"
expect_true "L19 the tar retry made a second VM" test "$(count_log 'machine new ru-t ')" = 2
expect_true "L19 the upload retry is visible in the output" grep -q 'upload to ru-t failed within 30 s (exit 2); retrying once' err
new_repo; STUB_TARFAIL=99 STUB_MODE='' expect_code 1 "L19 two tar failures fail the run"
new_repo; BOXD_RETRY_WITHIN=1 STUB_TARFAIL=1 STUB_TARFAIL_SLEEP=2 STUB_MODE='' expect_code 2 "L19 a late upload failure is not retried"
expect_true "L19 a late failure made one VM" test "$(count_log 'machine new ru-t ')" = 1
new_repo; STUB_MODE=no-output expect_code 1 "L19 a run whose agent produced nothing is not retried"
expect_true "L19 no second VM after the agent ran" test "$(count_log 'machine new ru-t ')" = 1
expect_true "L19 the no-output event is recorded with VM and phase" grep -q ' ru-t agent no-output' loop/out/events.log
new_repo; STUB_MODE=deadline expect_code 1 "L19 a wedged agent run fails"
expect_true "L19 the deadline event is recorded, not retried" bash -c "grep -q ' ru-t agent deadline-exceeded' loop/out/events.log && test \"\$(grep -c 'machine new ru-t ' log)\" = 1"
new_repo; BOXD_AGENT_TIMEOUT=1 STUB_CLAUDE_SLEEP=2 STUB_MODE=deadline expect_code 1 "L19 a silent run past BOXD_AGENT_TIMEOUT still fails"
expect_true "L19 no timeout message on a silent run past BOXD_AGENT_TIMEOUT" bash -c '! grep -q "agent timed out after" err'
expect_true "L19 the no-output message is used instead" grep -q "agent produced no output" err
new_repo; STUB_MODE=check-deadline expect_code 1 "L19 a build whose check hits the deadline fails"
expect_true "L19 the build check's deadline event is recorded with VM and phase" grep -q ' ru-t check deadline-exceeded' loop/out/events.log

# L20: each swarm review prompt may carry its own ref after an @; without one it uses BOXD_REF.
new_repo; git switch -qc feat; echo y >g; git add g; git commit -qm feat; git switch -q main; git update-ref refs/remotes/origin/main main; echo p >p2.md
export BOXD_LOCK_WAIT=30
loop/boxd.sh swarm review prompt.md@feat p2.md >out 2>err
replay_on_vm "$dir/vm1" ru-reviewer-1
replay_on_vm "$dir/vm2" ru-reviewer-2
expect_true "L20 the prompt with @feat reviews feat" test -e "$dir/vm1/roundup/g"
expect_true "L20 the prompt without a ref reviews HEAD" bash -c "! test -e '$dir/vm2/roundup/g'"
new_repo; git switch -qc feat; echo y >g; git add g; git commit -qm feat; git switch -q main; git update-ref refs/remotes/origin/main main; echo p >p2.md
export BOXD_LOCK_WAIT=30
BOXD_REF=feat loop/boxd.sh swarm review prompt.md@main p2.md >out 2>err
replay_on_vm "$dir/vm1" ru-reviewer-1
replay_on_vm "$dir/vm2" ru-reviewer-2
expect_true "L20 an explicit ref beats BOXD_REF" bash -c "! test -e '$dir/vm1/roundup/g'"
expect_true "L20 a prompt without a ref falls back to BOXD_REF" test -e "$dir/vm2/roundup/g"
new_repo; echo p >'a@b.md'; loop/boxd.sh swarm review 'a@b.md' >out 2>err || true
expect_true "L20 an existing file with an @ in its name is a plain prompt" grep -qx 'ru-reviewer-1 ok' out
new_repo; got=0; loop/boxd.sh swarm review prompt.md@nope >out 2>err || got=$?
expect_true "L20 an unknown ref is refused" test "$got" -eq 2
if grep -q 'machine new' log; then echo "FAIL: L20 VM created for an unknown ref"; failures=$((failures + 1)); else echo "ok:   L20 no VM for an unknown ref"; fi

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
replay_on_vm "$dir/vm" ru-reviewer-1
expect_true "L12 swarm review checks out BOXD_REF" test -e "$dir/vm/roundup/g"

refused "L12 swarm with an unknown role" swarm nope prompt.md
refused "L12 swarm with no prompt files" swarm build

# SIGTERM: the swarm removes its VMs and its children die, instead of burning quota to the end.
new_repo; export BOXD_LOCK_WAIT=30; echo p >p2.md
STUB_CLAUDE_SLEEP=30 loop/boxd.sh swarm build prompt.md p2.md >out 2>err &
swarm_pid=$!
for _ in $(seq 100); do [ "$(find cp -name 'exec-*.pid' | wc -l | tr -d ' ')" = 2 ] && break; sleep 0.1; done
kill -TERM "$swarm_pid"
SECONDS=0
rc=0; wait "$swarm_pid" || rc=$?
expect_true "L12 interrupt ends the swarm at once, not when the agents finish" test "$SECONDS" -lt 10
expect_true "L12 interrupted swarm exits 130" test "$rc" = 130
expect_log 'machine remove ru-builder-1' "L12 interrupt removes ru-builder-1"
expect_log 'machine remove ru-builder-2' "L12 interrupt removes ru-builder-2"
expect_true "L12 interrupt stops the agents" bash -c "for f in cp/exec-*.pid; do ! kill -0 \$(cat \$f) 2>/dev/null || exit 1; done"

# A name that already exists is someone else's: an interrupted swarm must not remove it.
new_repo; export BOXD_LOCK_WAIT=30; echo p >p2.md; touch cp/alive-ru-builder-1
STUB_CLAUDE_SLEEP=30 loop/boxd.sh swarm build prompt.md p2.md >out 2>err &
swarm_pid=$!
for _ in $(seq 100); do [ -e cp/exec-ru-builder-2.pid ] && break; sleep 0.1; done
kill -TERM "$swarm_pid"; wait "$swarm_pid" || true
expect_true "L12 interrupt leaves a VM the swarm did not create" test -e cp/alive-ru-builder-1
expect_true "L12 interrupt removes the VM the swarm created" test ! -e cp/alive-ru-builder-2
if grep -q 'machine remove ru-builder-1 ' log; then echo "FAIL: L12 interrupt removed a foreign VM"; failures=$((failures + 1)); else echo "ok:   L12 interrupt never asks to remove a foreign VM"; fi

new_repo; STUB_MODE=mixed loop/boxd.sh status >out 2>err
expect_true "L13 status shows agent-running" grep -qx 'ru-builder-1 agent-running' out
expect_true "L13 status shows idle" grep -qx 'ru-reviewer-2 idle' out
expect_true "L13 status shows unreachable" grep -qx 'ru-x8-1 unreachable' out
expect_true "L13 status lists every ru- VM" test "$(wc -l <out | tr -d ' ')" = 8

new_repo; STUB_MODE=mixed loop/boxd.sh kill all >out 2>err
expect_log 'machine remove ru-builder-1' "L14 kill all removes swarm builders"
expect_log 'machine remove ru-reviewer-2' "L14 kill all removes swarm reviewers"
for foreign in ru-x8-1 ru-builder-1x ru-builderx ru-x-builder-9 ru-builder-x; do
  if grep -q "machine remove $foreign " log; then echo "FAIL: L14 kill all removed $foreign"; failures=$((failures + 1)); else echo "ok:   L14 kill all leaves $foreign"; fi
done
new_repo
if STUB_MODE=mixed STUB_FAIL_REMOVE=ru-builder-1 loop/boxd.sh kill all >out 2>err; then echo "FAIL: L14 failed removal hidden"; failures=$((failures + 1)); else echo "ok:   L14 a failed removal fails kill all"; fi
expect_true "L14 failed removal is named" grep -q '^FAILED' out
new_repo; STUB_MODE=mixed loop/boxd.sh kill all >out 2>err
new_repo; loop/boxd.sh kill foo >out 2>err; expect_log 'machine remove ru-foo' "L14 kill <name> removes ru-<name>"
new_repo
if loop/boxd.sh kill '../x' >out 2>err; then echo "FAIL: L14 hostile kill name accepted"; failures=$((failures + 1)); else echo "ok:   L14 hostile kill name refused"; fi

# L21: every event lands in the run's event stream as it happens; the verdict, cost line and L4's pause come only
# from the stream's last event, and that event must be a result event.
new_repo; loop/boxd.sh build t prompt.md >out 2>err
expect_true "L21 the event stream file is named like L17's run log" bash -c 'ls loop/out/runs/t-*.jsonl >/dev/null'
expect_true "L21 the stream holds every event claude produced, not just the last" test "$(wc -l <"$(ls loop/out/runs/t-*.jsonl)" | tr -d ' ')" -ge 2
expect_log 'output-format stream-json --verbose' "L21 claude is run with streamed, verbose output"

# Run in the background so the stream can be read while the fake claude (events-forever) is still running: the
# stub's exec-<vm>.pid file is the bound the stub emits for "still running".
new_repo
BOXD_AGENT_TIMEOUT=3 STUB_MODE=events-forever loop/boxd.sh build t prompt.md >out 2>err &
build_pid=$!
stream=
for _ in $(seq 100); do
  stream=$(find loop/out/runs -name 't-*.jsonl' 2>/dev/null | head -1) || true
  [ -n "$stream" ] && [ -s "$stream" ] && [ -e cp/exec-ru-t.pid ] && break
  sleep 0.1
done
expect_true "L21 the first event is in the event stream while the stub still runs" bash -c "[ -n '$stream' ] && [ -s '$stream' ] && kill -0 \$(cat cp/exec-ru-t.pid) 2>/dev/null"
got=0; wait "$build_pid" || got=$?
if [ "$got" -eq 1 ]; then echo "ok:   L21 an agent still producing events at the timeout exits 1"; else echo "FAIL: L21 an agent still producing events at the timeout exits 1 (wanted exit 1, got $got)"; failures=$((failures + 1)); fi
expect_true "L21 the timeout message names the seconds and the stream" grep -qE 'agent timed out after 3 s: loop/out/runs/t-.*\.jsonl' err
expect_true "L21 a timed-out build skips the check" bash -c '! grep -q "just check" log'
expect_true "L21 a timed-out build saves the diff so far as a partial patch" test -e loop/out/patches/t.partial.patch
expect_true "L21 a timed-out build writes no final patch" bash -c '! test -e loop/out/patches/t.patch'
expect_true "L21 no PAUSED from a timed-out run" bash -c '! test -e loop/out/PAUSED'
expect_true "L21 the partial patch diffs against base, so a commit the Builder already made is in it" grep -qF partial-diff-against-base loop/out/patches/t.partial.patch

# boxd's own --timeout is a no-output deadline: a stub that keeps streaming, and never stops itself, stands in for
# a real boxd that would otherwise run forever. The outer `timeout 15` only bounds this test against a regression;
# the assertion is that the run ends at BOXD_AGENT_TIMEOUT=3, not at 15.
new_repo; got=0; BOXD_AGENT_TIMEOUT=3 STUB_MODE=events-forever-ignoring-boxd-timeout timeout 15 loop/boxd.sh build t prompt.md >out 2>err || got=$?
if [ "$got" -eq 1 ]; then echo "ok:   L21 a wall-clock cutoff ends a run boxd's own --timeout never would"; else echo "FAIL: L21 a wall-clock cutoff ends a run boxd's own --timeout never would (wanted exit 1, got $got)"; failures=$((failures + 1)); fi
expect_true "L21 that cutoff reports a timeout, naming the seconds" grep -qE 'agent timed out after 3 s: loop/out/runs/t-.*\.jsonl' err
expect_true "L21 that cutoff still saves the diff so far as a partial patch" test -e loop/out/patches/t.partial.patch

new_repo; got=0; STUB_MODE=progress-then-result loop/boxd.sh review r prompt.md >out 2>err || got=$?
expect_true "L21 a review with an earlier non-result event still exits 0" test "$got" -eq 0
expect_true "L21 the verdict is the last event's result, not an earlier event" test "$(cat loop/out/verdicts/r.md)" = "final answer"
# shellcheck disable=SC2016 # the $1.23 is the literal cost line boxd.sh prints, not a shell variable
expect_true "L21 the review cost line reports the last event's turns, duration and cost" grep -qF '7 turns, 5s, $1.23 notional' out

new_repo; STUB_MODE=progress-then-result loop/boxd.sh build t prompt.md >out 2>err
# shellcheck disable=SC2016 # the $1.23 is the literal cost line boxd.sh prints, not a shell variable
expect_true "L21 the build cost line reports the last event's turns, duration and cost" grep -qF '7 turns, 5s, $1.23 notional' out

new_repo; STUB_MODE=events-then-exit expect_code 1 "L21 an agent that ends with no result event exits 1"
expect_true "L21 the no-result message names the stream" grep -qE 'agent ended without a result: loop/out/runs/t-.*\.jsonl' err
expect_true "L21 a result-less build skips the check" bash -c '! grep -q "just check" log'
expect_true "L21 a result-less build saves the diff so far as a partial patch" test -e loop/out/patches/t.partial.patch
expect_true "L21 no PAUSED from a result-less run" bash -c '! test -e loop/out/PAUSED'

# A timed-out or result-less run removes the stable result file an earlier successful run left, and a later
# successful run removes the stale partial, so the result file is always from the latest run.
new_repo; STUB_MODE='' loop/boxd.sh build t prompt.md >out 2>err
expect_true "L21 setup: a successful build writes the stable patch" test -e loop/out/patches/t.patch
BOXD_AGENT_TIMEOUT=3 STUB_MODE=events-forever loop/boxd.sh build t prompt.md >out 2>err || true
expect_true "L21 a timed-out run after a successful one removes the stale patch" bash -c '! test -e loop/out/patches/t.patch'
STUB_MODE='' loop/boxd.sh build t prompt.md >out 2>err
expect_true "L21 a successful run after a timed-out one removes the stale partial patch" bash -c '! test -e loop/out/patches/t.partial.patch'
expect_true "L21 the later successful run's patch is the latest result" test -e loop/out/patches/t.patch

new_repo; loop/boxd.sh review r prompt.md >out 2>err
expect_true "L21 setup: a successful review writes the stable verdict" test -e loop/out/verdicts/r.md
got=0; STUB_MODE=events-then-exit loop/boxd.sh review r prompt.md >out 2>err || got=$?
expect_true "L21 a result-less review exits 1" test "$got" -eq 1
expect_true "L21 a result-less review after a successful one removes the stale verdict" bash -c '! test -e loop/out/verdicts/r.md'

# A stale result file must go even on an exit path that never reaches build()'s or review()'s own cleanup branch:
# a failed check (not a timed-out or result-less run), a no-output run and a paused run all exit before that point.
new_repo; STUB_MODE='' loop/boxd.sh build t prompt.md >out 2>err
expect_true "L21 setup: a successful build writes the stable patch (2)" test -e loop/out/patches/t.patch
got=0; STUB_MODE=check-fails loop/boxd.sh build t prompt.md >out 2>err || got=$?
expect_true "L21 a failed check keeps its exit code (2)" test "$got" -eq 7
expect_true "L21 a failed check after a successful build removes the stale patch" bash -c '! test -e loop/out/patches/t.patch'

new_repo; BOXD_AGENT_TIMEOUT=3 STUB_MODE=events-forever loop/boxd.sh build t prompt.md >out 2>err || true
expect_true "L21 setup: a timed-out build writes the partial patch" test -e loop/out/patches/t.partial.patch
got=0; STUB_MODE=check-fails loop/boxd.sh build t prompt.md >out 2>err || got=$?
expect_true "L21 a failed check after a timed-out build removes the stale partial patch" bash -c '! test -e loop/out/patches/t.partial.patch'

new_repo; loop/boxd.sh review r prompt.md >out 2>err
expect_true "L21 setup: a successful review writes the stable verdict (2)" test -e loop/out/verdicts/r.md
got=0; STUB_MODE=no-output loop/boxd.sh review r prompt.md >out 2>err || got=$?
expect_true "L19 a no-output review exits 1" test "$got" -eq 1
expect_true "L21 a no-output review after a successful one removes the stale verdict" bash -c '! test -e loop/out/verdicts/r.md'

new_repo; loop/boxd.sh review r prompt.md >out 2>err
expect_true "L21 setup: a successful review writes the stable verdict (3)" test -e loop/out/verdicts/r.md
got=0; STUB_MODE=limit loop/boxd.sh review r prompt.md >out 2>err || got=$?
expect_true "L4 a paused review exits 75" test "$got" -eq 75
expect_true "L4 a paused review after a successful one removes the stale verdict" bash -c '! test -e loop/out/verdicts/r.md'

new_repo; STUB_MODE=limit-then-more expect_code 1 "L21 an event after the result event is read as ended without a result"
expect_true "L21 no pause when the 429 result is not the stream's last event" bash -c '! test -e loop/out/PAUSED'
expect_true "L21 the trailing event is reported as ended without a result" grep -q 'agent ended without a result' err

new_repo; STUB_MODE=retry-429-then-ok expect_code 0 "L21 a non-final 429 event before a successful result never pauses"
expect_true "L21 no PAUSED from a non-final 429 retry event" bash -c '! test -e loop/out/PAUSED'

new_repo; STUB_MODE=ok-quotes-limit expect_code 0 "L4 a successful result that quotes the limit-phrase case never pauses"
expect_true "L4 no PAUSED from a successful result quoting the limit-phrase case" bash -c '! test -e loop/out/PAUSED'

new_repo; got=0; STUB_MODE=ok-quotes-limit loop/boxd.sh review r prompt.md >out 2>err || got=$?
expect_true "L21 a review whose result quotes the limit-phrase case exits 0" test "$got" -eq 0
expect_true "L4 no PAUSED from a review quoting the limit-phrase case" bash -c '! test -e loop/out/PAUSED'
phrase="usage"" limit and rate"" limit"
expect_true "L21 the verdict still holds the result text when it quotes the limit-phrase case" grep -qF "mentions the $phrase case but succeeded" loop/out/verdicts/r.md

new_repo; STUB_MODE=limit-text expect_code 75 "L4 an error result naming the limit-phrase case without a 429 status pauses"
expect_true "L4 PAUSED written for the limit-phrase case" test -e loop/out/PAUSED

new_repo; STUB_MODE=error-no-limit expect_code 1 "L4 an error result with no limit phrase and no 429 status fails the build"
expect_true "L4 no PAUSED from a plain error result" bash -c '! test -e loop/out/PAUSED'
expect_true "L21 no patch is written for a plain error result" bash -c '! test -e loop/out/patches/t.patch'

[ "$failures" -eq 0 ] || { echo "$failures failed"; exit 1; }
