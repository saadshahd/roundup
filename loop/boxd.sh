#!/usr/bin/env bash
# Run one Builder task in a disposable, isolated boxd VM. Usage:
#   loop/boxd.sh bake                       # build the ru-toolchain snapshot (once per toolchain change)
#   loop/boxd.sh build <name> <prompt-file> # Builder run; writes loop/out/patches/<name>.patch
#   loop/boxd.sh review <name> <prompt-file> [ref] # Reviewer run on ref (default HEAD) against its merge-base with origin/main;
#                                           # writes the Reviewer's answer to loop/out/verdicts/<name>.md. The checkout is a
#                                           # git repo with tag `base`, so the Reviewer can run `loop/rules.sh size base`.
#   loop/boxd.sh check <pr-number|branch>   # merge it into origin/main here, run `just check` on a fresh isolated VM
#   loop/boxd.sh swarm <build|review> <prompt-file>[@<ref>]...  # one isolated VM per prompt (ru-builder-<n> / ru-reviewer-<n>), at most BOXD_MAX_VMS at once; one status line each; a review prompt takes its own ref, else BOXD_REF
#   loop/boxd.sh status                      # every ru- VM with what it is doing
#   loop/boxd.sh kill <name|all>             # remove ru-<name>; `all` removes only swarm VMs (ru-builder-*, ru-reviewer-*)
# review checks out its ref argument (default HEAD; a swarm review without `@<ref>` uses $BOXD_REF). A swarm child waits up to $BOXD_SLOT_WAIT s (default 900) for a free VM slot; a single run does not wait. Every run's log is <name>-<UTC time>-<pid>.* under loop/out/runs; loop/out/events.log records wedges, no-output and deadline events with the VM and phase. BOXD_MAX_VMS caps concurrent ru- VMs (default 12). BOXD_AGENT_TIMEOUT (seconds, at most 1800) bounds the agent run. BOXD_MODEL overrides the model (build: sonnet, review: opus).
# Auth is the boxd secret CLAUDE_CODE_OAUTH_TOKEN (sealed, host-scoped): each VM sees only a placeholder, boxd swaps in the real token for *.anthropic.com, *.claude.com and claude.ai.
# Exit codes: 0 ok, 1 failure, 75 paused (limit hit, or loop/out/PAUSED exists).
set -euo pipefail
cd "$(dirname "$0")/.."

SNAPSHOT=ru-toolchain
OUT=loop/out
PAUSED=$OUT/PAUSED
EX_PAUSED=75
MAX_VMS=${BOXD_MAX_VMS:-12}
# The agent and the check each get up to AGENT_TIMEOUT / 1800 s; VM_TTL must outlast both plus the reboot wait (at most 60 attempts of 5 s plus 1 s, 360 s), upload and the patch step.
AGENT_TIMEOUT=${BOXD_AGENT_TIMEOUT:-1800}
VM_TTL=4200
SLOT_WAIT=${BOXD_SLOT_WAIT:-0}
REBOOT_ATTEMPTS=${BOXD_REBOOT_ATTEMPTS:-60}
EVENTS=$OUT/events.log
RUN_ID=$(date -u +%Y%m%dT%H%M%SZ)-$$
PROVISION_RETRY_WITHIN=${BOXD_RETRY_WITHIN:-30}
VM=
LOCK=$OUT/lock
LOCK_WAIT=${BOXD_LOCK_WAIT:-60}
lock_held=0
# The ru-toolchain snapshot carries a stray /node_modules/@types/node at the filesystem root (observed on ru-tsc-gate):
# tsc's ambient @types lookup walks up every parent of ~/roundup and finds it, so a package that never declares
# @types/node itself would still typecheck clean there although a clean runner's tsc fails it. run_check removes it
# first, on every VM, so the check it observes never depends on anything outside the checkout.
STRAY_NODE_MODULES=/node_modules

cleanup() {
  local rc=$?
  if [ "$lock_held" -eq 1 ]; then rmdir "$LOCK" || true; fi
  if [ -n "$VM" ]; then
    boxd machine remove "$VM" -y >/dev/null </dev/null || echo "boxd.sh: LEAKED VM $VM; remove it by hand" >&2
  fi
  exit "$rc"
}
trap cleanup EXIT

# A token baked into a snapshot reaches every VM made from it, so refuse to save one that contains any. The scan skips
# the Bun package cache, whose docs hold placeholder strings that match the pattern.
TOKEN_PATTERN='gho_[A-Za-z0-9]{20,}|ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|sk-ant-[A-Za-z0-9_-]{20,}'

# Install, then clippy and test builds of the lockfile's dependencies, so a later `check` only compiles the workspace crates.
# The target dir lives outside the checkout (see CARGO_TARGET_DIR in ~/.cargo/env below). Workspace members are then cleaned:
# their artifacts hold /tmp/warm in env!("CARGO_MANIFEST_DIR") and dep-info, and cargo judges a path crate fresh by mtime, so a
# tree uploaded elsewhere with older mtimes (git archive stamps commit time) could run the baked code instead of its own.
# shellcheck disable=SC2016 # runs on the VM, so nothing may expand here
WARM='set -e; . ~/.cargo/env; mkdir /tmp/warm && tar xzf /tmp/warm.tgz -C /tmp/warm && cd /tmp/warm
pnpm install --frozen-lockfile >/dev/null
cargo clippy --all-targets >/dev/null 2>&1
cargo nextest run --no-run >/dev/null 2>&1
for member in $(cargo metadata --no-deps --format-version 1 | jq -r ".packages[].name"); do cargo clean -p "$member" -q; done
cd; rm -rf /tmp/warm /tmp/warm.tgz'

# Fails (printing the offending files) when anything under the warm target still names the bake checkout.
# shellcheck disable=SC2016 # runs on the VM, so nothing may expand here
ASSERT_COLD_WORKSPACE='! grep -rlF /tmp/warm ~/cargo-target | head -5 | grep .'

bake() {
  mkdir -p "$OUT"
  # Isolated: the dependency build scripts run here, so no GitHub login may be in reach.
  # Own the name only once `new` succeeds, so a failed create never removes another bake's VM.
  boxd machine new ru-bake --isolated --auto-suspend-timeout 0 --auto-destroy-timeout 7200 >/dev/null </dev/null
  VM=ru-bake
  local rust pnpm
  rust="$(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml)"
  pnpm="$(jq -r '.packageManager | sub("pnpm@"; "")' package.json)"
  # NEEDRESTART_SUSPEND stops apt restarting the boxd agent mid-exec ("lost the connection to the machine").
  boxd machine exec "$VM" --timeout 580 -- "set -e
    curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain $rust -c clippy -c rustfmt >/dev/null 2>&1
    . ~/.cargo/env
    curl -LsSf https://get.nexte.st/latest/linux | tar zxf - -C ~/.cargo/bin
    sudo apt-get update -qq && sudo NEEDRESTART_SUSPEND=1 DEBIAN_FRONTEND=noninteractive apt-get install -y -qq libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libsoup-3.0-dev pkg-config >/dev/null
    cargo install cargo-machete just --locked >/dev/null 2>&1
    npm i -g pnpm@$pnpm >/dev/null 2>&1
    sudo ln -sf \"\$(ls -d ~/.nvm/versions/node/*/bin/pnpm | head -1)\" /usr/local/bin/pnpm
    echo 'export CARGO_TARGET_DIR=\$HOME/cargo-target' >>~/.cargo/env
    pnpm --version" </dev/null
  git archive --format=tar.gz HEAD | boxd machine cp - "$VM:/tmp/warm.tgz" >/dev/null
  boxd machine exec "$VM" --timeout 1800 -- "$WARM" </dev/null
  boxd machine exec "$VM" --timeout 300 -- "$ASSERT_COLD_WORKSPACE" </dev/null || { echo "boxd.sh: the warm target still holds workspace artifacts from /tmp/warm; not saving the snapshot" >&2; exit 1; }
  local scan=0
  # shellcheck disable=SC2016 # $HOME must expand inside the VM
  boxd machine exec "$VM" --timeout 580 -- "sudo grep -rIlE --exclude-dir=proc --exclude-dir=sys --exclude-dir=cargo-target --exclude-dir=.pnpm-store --exclude-dir=.bun -- '$TOKEN_PATTERN' \$HOME /etc /usr/local /root /var/lib 2>/dev/null" </dev/null >"$OUT/bake-scan.txt" || scan=$?
  [ "$scan" -eq 1 ] || { echo "boxd.sh: token scan of the bake VM failed or found a token (exit $scan); not saving the snapshot; files named in $OUT/bake-scan.txt" >&2; exit 1; }
  boxd snapshots save "$VM" "$SNAPSHOT" </dev/null
}

pause() {
  [ -e "$PAUSED" ] || date -u +%FT%TZ >"$PAUSED"
  echo "boxd.sh: $1; wrote $PAUSED, delete it to resume" >&2
  exit "$EX_PAUSED"
}

# The JSON has no quota field, so a limit shows up only as an error status or message.
guard_limits() {
  jq -e '.api_error_status == 429 or ((.result // "") | test("usage limit|rate limit"; "i"))' "$1" >/dev/null || return 0
  pause "limit hit"
}

# Serialise "count, then create" so parallel starts cannot all pass the cap.
acquire_lock() {
  local _
  for _ in $(seq "$LOCK_WAIT"); do
    if mkdir "$LOCK" 2>/dev/null; then lock_held=1; return 0; fi
    sleep 1
  done
  echo "boxd.sh: $LOCK is held; remove it if no other run is active" >&2
  exit 1
}

release_lock() {
  rmdir "$LOCK"
  lock_held=0
}

# <name> becomes a VM name and a file name; <prompt-file> is passed to `boxd machine cp` as a positional argument.
validate_args() {
  [[ $AGENT_TIMEOUT =~ ^[0-9]+$ && $AGENT_TIMEOUT -le 1800 ]] || { echo "boxd.sh: BOXD_AGENT_TIMEOUT must be a number of seconds, at most 1800, got: $AGENT_TIMEOUT" >&2; exit 2; }
  [[ $SLOT_WAIT =~ ^[0-9]+$ ]] || { echo "boxd.sh: BOXD_SLOT_WAIT must be a number of seconds, got: $SLOT_WAIT" >&2; exit 2; }
  [[ $REBOOT_ATTEMPTS =~ ^[0-9]+$ && $REBOOT_ATTEMPTS -ge 1 && $REBOOT_ATTEMPTS -le 60 ]] || { echo "boxd.sh: BOXD_REBOOT_ATTEMPTS must be 1 to 60, got: $REBOOT_ATTEMPTS" >&2; exit 2; }
  [[ $MAX_VMS =~ ^[0-9]+$ ]] || { echo "boxd.sh: BOXD_MAX_VMS must be a number, got: $MAX_VMS" >&2; exit 2; }
  local name=$1 prompt=$2
  [[ $name =~ ^[a-z0-9][a-z0-9-]*$ ]] || { echo "boxd.sh: name must match ^[a-z0-9][a-z0-9-]*\$, got: $name" >&2; exit 2; }
  [[ $prompt != -* ]] || { echo "boxd.sh: prompt file must not start with '-': $prompt" >&2; exit 2; }
  [ -f "$prompt" ] || { echo "boxd.sh: no prompt file: $prompt" >&2; exit 2; }
}

vm_count() { boxd machine list --json </dev/null | jq '[.[] | select(.name | startswith("ru-"))] | length'; }

# Stop before any VM exists unless a Claude run can start: the boxd secret is set and no limit pause is active.
require_claude() {
  boxd env list --json </dev/null | jq -e 'any(.[]; .name == "CLAUDE_CODE_OAUTH_TOKEN")' >/dev/null || { echo "boxd.sh: boxd secret CLAUDE_CODE_OAUTH_TOKEN is missing (boxd env set --secret, allowed for *.anthropic.com, *.claude.com, claude.ai)" >&2; exit 1; }
  mkdir -p "$OUT/patches" "$OUT/runs" "$OUT/verdicts"
  [ ! -e "$PAUSED" ] || pause "paused since $(cat "$PAUSED")"
}

# Append one line to the events log: UTC time, VM, phase, event.
record_event() {
  mkdir -p "$OUT"
  printf '%s %s %s %s\n' "$(date -u +%FT%TZ)" "${VM:-none}" "$1" "$2" >>"$EVENTS"
}

# Create the isolated VM for <name>, enforcing the cap. Sets $VM. Waits up to SLOT_WAIT s for a free slot, then exits 1 naming the cap.
create_vm() {
  local name=$1 waited=0
  mkdir -p "$OUT/runs"
  acquire_lock
  while [ "$(vm_count)" -ge "$MAX_VMS" ]; do
    if [ "$waited" -ge "$SLOT_WAIT" ]; then echo "boxd.sh: $MAX_VMS ru- VMs already exist (cap BOXD_MAX_VMS=$MAX_VMS); no slot freed within ${SLOT_WAIT} s" >&2; exit 1; fi
    release_lock
    sleep 1
    waited=$((waited + 1))
    acquire_lock
  done
  # The timer outlasts everything that runs after creation (reboot wait of at most 360 s, upload, an agent run of up to AGENT_TIMEOUT, then run_check's 1800 s).
  # Own the name only once `new` succeeds, so a failed create never removes someone else's VM.
  boxd machine new "ru-$name" --from-snapshot "$SNAPSHOT" --isolated --auto-suspend-timeout 0 --auto-destroy-timeout $VM_TTL >/dev/null </dev/null
  VM="ru-$name"
  release_lock
}

# A VM restored from a memory snapshot wedges claude and tsc until it is rebooted (measured: tsc hangs before, 0.4 s after).
# Returns 3 when the VM never answers (at most REBOOT_ATTEMPTS of a 5 s exec plus 1 s).
await_reboot() {
  boxd machine reboot "$VM" >/dev/null </dev/null || return 3
  local _
  for _ in $(seq "$REBOOT_ATTEMPTS"); do
    boxd machine exec "$VM" --timeout 5 -- true </dev/null >/dev/null 2>&1 && return 0
    sleep 1
  done
  return 3
}

# Put <ref> in ~/roundup on the VM as a git repo: tag `base` is <base-ref>'s tree, HEAD is <ref>'s tree. A non-empty
# <prompt> file is copied to /tmp/prompt.md.
upload_checkout() {
  local base_ref=$1 ref=$2 prompt=$3
  git archive --format=tar.gz --end-of-options "$base_ref" | boxd machine cp - "$VM:/tmp/base.tgz" >/dev/null
  git archive --format=tar.gz --end-of-options "$ref" | boxd machine cp - "$VM:/tmp/src.tgz" >/dev/null
  [ -z "$prompt" ] || boxd machine cp "$prompt" "$VM:/tmp/prompt.md" >/dev/null </dev/null
  # Replay the PR's own commits, so a Reviewer sees the real messages, authors and trailers. A series cannot carry a merge
  # commit, so a range with one (or a series that does not apply) falls back to one `head` commit holding the ref's tree.
  if [ -n "$(git rev-list --merges --end-of-options "$base_ref..$ref")" ]; then
    echo "boxd.sh: $ref contains a merge commit; the checkout gets one commit named head instead of the PR's commits" >&2
  else
    git format-patch --stdout --binary --end-of-options "$base_ref..$ref" | boxd machine cp - "$VM:/tmp/series.mbox" >/dev/null
  fi
  boxd machine exec "$VM" -- 'mkdir -p ~/roundup && tar xzf /tmp/base.tgz -C ~/roundup && cd ~/roundup &&
    git init -q && git add -A >/dev/null && git -c user.email=builder@roundup -c user.name=builder commit -qm base && git tag base &&
    { [ ! -s /tmp/series.mbox ] || git -c user.email=builder@roundup -c user.name=builder am -q --empty=keep /tmp/series.mbox ||
      { git am --abort; echo "boxd.sh: the PR commits did not replay; the checkout gets one commit named head" >&2; }; } &&
    git rm -rqf . && tar xzf /tmp/src.tgz -C ~/roundup && git add -A >/dev/null &&
    { git diff --cached --quiet || git -c user.email=builder@roundup -c user.name=builder commit -qm head; }' </dev/null
}

# Create <name>'s VM and put the checkout on it. A VM that never answers after its reboot, or an upload that fails within
# PROVISION_RETRY_WITHIN s, is retried once on a fresh VM, before any agent has run (retrying after one ran would cost a second verdict).
provision() {
  local name=$1 base_ref=$2 ref=$3 prompt=$4 attempt reason rc started
  for attempt in 1 2; do
    create_vm "$name"
    reason='' rc=0
    if ! await_reboot; then
      reason="$VM did not answer after reboot"
      rc=1
    else
      started=$SECONDS
      set +e
      ( set -e; upload_checkout "$base_ref" "$ref" "$prompt" )
      rc=$?
      set -e
      if [ "$rc" -ne 0 ]; then
        [ $((SECONDS - started)) -le "$PROVISION_RETRY_WITHIN" ] || { echo "boxd.sh: the upload to $VM failed (exit $rc)" >&2; exit "$rc"; }
        reason="the upload to $VM failed within ${PROVISION_RETRY_WITHIN} s (exit $rc)"
      fi
    fi
    [ "$rc" -ne 0 ] || return 0
    record_event provision "$reason"
    [ "$attempt" -eq 1 ] || { echo "boxd.sh: $reason; giving up after one retry" >&2; exit 1; }
    echo "boxd.sh: $reason; retrying once on a fresh VM" >&2
    boxd machine remove "$VM" -y >/dev/null </dev/null || echo "boxd.sh: LEAKED VM $VM; remove it by hand" >&2
    VM=
  done
}

# Run claude on the VM with /tmp/prompt.md; JSON lands in <result>. Exits unless the run succeeded.
run_agent() {
  local model=$1 result=$2
  # A feature-sized task outlasts 570 s (measured: V7 Todo triage hit it and returned nothing), so wait as long as the check may.
  # claude exits non-zero on an API error; keep going so the limit guard can see it.
  # Without CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC, claude waits ~90 s on blocked hosts after it has already answered.
  boxd machine exec "$VM" --timeout "$AGENT_TIMEOUT" -e CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1 -- \
    "cd ~/roundup && . ~/.cargo/env && claude -p --model $model --output-format json --dangerously-skip-permissions 2>/dev/null </tmp/prompt.md" </dev/null >"$result" 2>"${result%.json}.err" || true
  cat "${result%.json}.err" >&2
  ! grep -q DeadlineExceeded "${result%.json}.err" || record_event agent deadline-exceeded
  [ -s "$result" ] || { record_event agent no-output; echo "boxd.sh: agent produced no output" >&2; exit 1; }
  guard_limits "$result"
  jq -e '.is_error == false' "$result" >/dev/null || { echo "boxd.sh: agent failed: $(jq -r .result "$result")" >&2; exit 1; }
}

# The check both `build` and `check` observe: remove the stray node_modules (above), then install from the lockfile,
# then the repo's own definition of check.
run_check() {
  boxd machine exec "$VM" --timeout 1800 -- "cd ~/roundup && . ~/.cargo/env && sudo rm -rf $STRAY_NODE_MODULES && pnpm install --frozen-lockfile && just check" </dev/null
}

review() {
  local name=$1 prompt=$2 ref=${3:-HEAD}
  validate_args "$name" "$prompt"
  local result="$OUT/runs/$name-$RUN_ID.json" verdict="$OUT/verdicts/$name.md" base
  base="$(git merge-base --end-of-options origin/main "$ref")" || { echo "boxd.sh: no merge-base of origin/main and $ref; git fetch origin" >&2; exit 1; }
  require_claude
  provision "$name" "$base" "$ref" "$prompt"
  run_agent "${BOXD_MODEL:-opus}" "$result"
  jq -r .result "$result" >"$verdict"
  jq -r '"boxd.sh: \(.num_turns) turns, \(.duration_ms / 1000 | floor)s, $\(.total_cost_usd) notional"' "$result"
}

build() {
  local name=$1 prompt=$2
  validate_args "$name" "$prompt"
  local result="$OUT/runs/$name-$RUN_ID.json" patch="$OUT/patches/$name.patch" checklog="$OUT/runs/$name-$RUN_ID.check.log"
  require_claude
  provision "$name" HEAD HEAD "$prompt"
  run_agent "${BOXD_MODEL:-sonnet}" "$result"
  # The observer is our own check, not the Builder's claim that it passed.
  local rc=0
  run_check >"$checklog" 2>&1 || rc=$?
  tail -n 15 "$checklog" >&2
  ! grep -q DeadlineExceeded "$checklog" || record_event check deadline-exceeded
  [ "$rc" -eq 0 ] || exit "$rc"
  boxd machine exec "$VM" -- 'cd ~/roundup && git add -A && { git diff --cached --quiet || git -c user.email=builder@roundup -c user.name=builder commit -qm "builder: task" -m "Author-Agent: builder"; } && git format-patch base --stdout' </dev/null >"$patch"
  jq -r '"boxd.sh: \(.num_turns) turns, \(.duration_ms / 1000 | floor)s, $\(.total_cost_usd) notional"' "$result"
}

# A PR number or a branch name becomes a fetch refspec; anything else is refused before it reaches git.
ref_to_refspec() {
  case "$1" in
    *[!0-9]* | '') [[ "$1" =~ ^[A-Za-z0-9][A-Za-z0-9._/-]*$ ]] || { echo "boxd.sh: bad ref '$1'" >&2; exit 2; }; echo "$1" ;;
    *) echo "pull/$1/head" ;;
  esac
}

# Output may carry a token printed by code under test; mask GitHub token shapes before it is shown or stored.
mask_github_tokens() { sed -E 's/(gh[pousr]_|github_pat_)[A-Za-z0-9_]+/\1<masked>/g'; }

# The merge into origin/main happens here with this machine's git login, so the VM never holds a GitHub credential
# and code in the PR (install scripts, build.rs, tests) runs where there is none to steal.
check() {
  local ref=$1 refspec slug log base sha tree
  refspec=$(ref_to_refspec "$ref")
  slug=$(printf '%s' "$ref" | tr -c 'A-Za-z0-9\n' - | tr '[:upper:]' '[:lower:]' | cut -c1-30)
  log="$OUT/runs/check-$slug-$RUN_ID.log"
  # Per-run ref names (not origin/main, not FETCH_HEAD) so checks started in parallel from one clone cannot race.
  git fetch -q origin -- "+refs/heads/main:refs/boxd-check/$slug-base" "+$refspec:refs/boxd-check/$slug"
  base=$(git rev-parse "refs/boxd-check/$slug-base")
  sha=$(git rev-parse "refs/boxd-check/$slug")
  git update-ref -d "refs/boxd-check/$slug-base"
  git update-ref -d "refs/boxd-check/$slug"
  tree=$(git merge-tree --write-tree "$base" "$sha") || { echo "boxd.sh: $ref conflicts with origin/main:" >&2; echo "$tree" >&2; exit 1; }
  provision "chk-$slug-$$" "$base" "$tree" ""
  local rc=0
  run_check 2>&1 | mask_github_tokens >"$log" || rc=${PIPESTATUS[0]}
  tail -n 25 "$log" >&2
  ! grep -q DeadlineExceeded "$log" || record_event check deadline-exceeded
  echo "boxd.sh: check $ref (merged into origin/main) exit $rc; full log $log" >&2
  exit "$rc"
}

# Run one agent per prompt file, each on its own VM, never more than MAX_VMS at once.
swarm() {
  local role=${1:?build or review} prefix n=0 i
  shift
  case $role in build) prefix=builder ;; review) prefix=reviewer ;; *) echo "boxd.sh: swarm role must be build or review, got: $role" >&2; exit 2 ;; esac
  [ "$#" -gt 0 ] || { echo "boxd.sh: swarm needs at least one prompt file" >&2; exit 2; }
  local pids=() names=() prompts=() refs=() spec prompt ref
  mkdir -p "$OUT/runs"
  # `<prompt-file>@<ref>` gives that prompt its own ref; an existing file named with an `@` is still a plain prompt file.
  for spec in "$@"; do
    prompt=$spec ref=${BOXD_REF:-HEAD}
    if [ ! -f "$spec" ] && [[ $spec == *@* ]]; then prompt=${spec%@*} ref=${spec##*@}; fi
    prompts+=("$prompt")
    refs+=("$ref")
  done
  # Refuse every bad prompt file and ref before the first VM exists.
  for i in "${!prompts[@]}"; do
    validate_args "$prefix-1" "${prompts[$i]}"
    [ "$role" = build ] || git rev-parse --verify -q --end-of-options "${refs[$i]}^{commit}" >/dev/null || { echo "boxd.sh: no such ref: ${refs[$i]}" >&2; exit 2; }
  done
  # Children wait for a slot, so a VM that is not theirs cannot make the last one fail at create.
  export BOXD_SLOT_WAIT=${BOXD_SLOT_WAIT:-900}
  # Background children ignore SIGINT, so TERM them; each one's exit trap then removes the VM it created and nothing else.
  trap 'stop_swarm "${pids[@]+"${pids[@]}"}"' INT TERM
  for i in "${!prompts[@]}"; do
    n=$((n + 1))
    while [ "$(running_children ${pids[@]+"${pids[@]}"})" -ge "$MAX_VMS" ]; do sleep 1; done
    names+=("$prefix-$n")
    if [ "$role" = review ]; then "$0" review "$prefix-$n" "${prompts[$i]}" "${refs[$i]}" </dev/null >"$OUT/runs/swarm-$$-$n.log" 2>&1 &
    else "$0" build "$prefix-$n" "${prompts[$i]}" </dev/null >"$OUT/runs/swarm-$$-$n.log" 2>&1 &
    fi
    pids+=($!)
  done
  local failed=0 rc
  for i in "${!pids[@]}"; do
    rc=0
    wait "${pids[$i]}" || rc=$?
    if [ "$rc" -eq 0 ]; then echo "ru-${names[$i]} ok"; else echo "ru-${names[$i]} failed rc=$rc (see $OUT/runs/swarm-$$-$((i + 1)).log)"; failed=1; fi
  done
  exit "$failed"
}

# stop_swarm <pid>...: TERM the children and wait until their exit traps have removed their VMs.
stop_swarm() {
  trap '' INT TERM
  kill "$@" 2>/dev/null || true
  wait "$@" 2>/dev/null || true
  echo "boxd.sh: swarm interrupted; its agents are stopped and their VMs removed" >&2
  exit 130
}

running_children() {
  local count=0 pid
  for pid in "$@"; do kill -0 "$pid" 2>/dev/null && count=$((count + 1)); done
  echo "$count"
}

status() {
  local vm state
  while read -r vm; do
    state=$(boxd machine exec "$vm" --timeout 10 -- 'pgrep -x claude >/dev/null && echo agent-running || echo idle' </dev/null 2>/dev/null) || state=unreachable
    echo "$vm $state"
  done < <(boxd machine list --json </dev/null | jq -r '.[] | select(.name | startswith("ru-")) | .name')
}

# `all` is every VM named exactly ru-builder-<n> or ru-reviewer-<n>, which is what `swarm` creates, including another swarm's.
kill_vms() {
  local target=$1 vm failed=0 targets
  if [ "$target" = all ]; then
    targets=$(boxd machine list --json </dev/null | jq -r '.[] | select(.name | test("^ru-(builder|reviewer)-[0-9]+$")) | .name')
  else
    [[ $target =~ ^[a-z0-9][a-z0-9-]*$ ]] || { echo "boxd.sh: bad name: $target" >&2; exit 2; }
    targets="ru-$target"
  fi
  while read -r vm; do
    [ -n "$vm" ] || continue
    if boxd machine remove "$vm" -y </dev/null >/dev/null; then echo "removed $vm"; else echo "FAILED $vm"; failed=1; fi
  done <<<"$targets"
  exit "$failed"
}

case "${1:-}" in
  bake) bake ;;
  build) build "${2:?name}" "${3:?prompt-file}" ;;
  check) check "${2:?pr-number-or-branch}" ;;
  swarm) shift; swarm "$@" ;;
  status) status ;;
  kill) kill_vms "${2:?name or all}" ;;
  review) review "${2:?name}" "${3:?prompt-file}" "${4:-HEAD}" ;;
  *) sed -n '2,14p' "$0" >&2; exit 2 ;;
esac
