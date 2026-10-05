#!/usr/bin/env bash
# Run one Builder or Reviewer in a disposable boxd VM. Usage:
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
# GNU timeout bounds the agent run by wall clock; stock macOS has none (coreutils installs it as gtimeout). BOXD_TIMEOUT_CMD overrides.
TIMEOUT_CMD=${BOXD_TIMEOUT_CMD:-$(command -v timeout || command -v gtimeout || true)}
need_timeout() {
  command -v "${TIMEOUT_CMD:-}" >/dev/null 2>&1 || { echo "boxd.sh: needs GNU timeout or gtimeout (brew install coreutils); found '${TIMEOUT_CMD:-}'" >&2; exit 2; }
}
SLOT_WAIT=${BOXD_SLOT_WAIT:-0}
REBOOT_ATTEMPTS=${BOXD_REBOOT_ATTEMPTS:-60}
EVENTS=$OUT/events.log
RUN_ID=$(date -u +%Y%m%dT%H%M%SZ)-$$
PROVISION_RETRY_WITHIN=${BOXD_RETRY_WITHIN:-30}
VM=
AGENT_KIND=claude
PARTIAL_TMP=
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
  [ -z "$PARTIAL_TMP" ] || rm -f "$PARTIAL_TMP"
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

# The JSON has no quota field, so a limit shows up only as an error status or message; the caller passes only the
# stream's last (result) event, so a phrase quoted anywhere else, or in a successful result, never pauses.
guard_limits() {
  jq -e '.api_error_status == 429 or (.is_error == true and ((.result // "") | test("usage limit|rate limit"; "i")))' <<<"$1" >/dev/null || return 0
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

require_codex() {
  mkdir -p "$OUT/patches" "$OUT/runs" "$OUT/verdicts"
  [ ! -e "$PAUSED" ] || pause "paused since $(cat "$PAUSED")"
  [[ ${BOXD_CODEX_AUTH_VM:-} =~ ^ru-[a-z0-9][a-z0-9-]*$ ]] || { echo "boxd.sh: BOXD_CODEX_AUTH_VM must name a ru- login holder" >&2; exit 2; }
  boxd machine exec "$BOXD_CODEX_AUTH_VM" -- 'test -s ~/.codex/auth.json && codex login status >/dev/null' </dev/null >/dev/null || { echo "boxd.sh: Codex login is unavailable on $BOXD_CODEX_AUTH_VM" >&2; exit 1; }
}

require_agent() {
  case ${BOXD_AGENT:-claude} in
    codex) AGENT_KIND=codex; require_codex ;;
    claude) require_claude ;;
    *) echo "boxd.sh: BOXD_AGENT must be codex or claude" >&2; exit 2 ;;
  esac
}

transfer_codex_login() {
  local source=$BOXD_CODEX_AUTH_VM
  [ "$source" != "$VM" ] || { echo "boxd.sh: Codex login holder and Builder VM must differ" >&2; return 1; }
  boxd machine cp "$source:.codex/auth.json" - 2>"$OUT/runs/$RUN_ID.auth.err" |
    boxd machine cp - "$VM:.codex/auth.json" >/dev/null 2>>"$OUT/runs/$RUN_ID.auth.err" || { echo "boxd.sh: Codex login transfer to $VM failed" >&2; return 1; }
  boxd machine exec "$VM" -- 'chmod 600 ~/.codex/auth.json && codex login status >/dev/null' </dev/null >/dev/null || { echo "boxd.sh: Codex login is unavailable on $VM" >&2; return 1; }
  boxd machine exec "$VM" -- 'if gh auth status >/dev/null 2>&1; then echo "boxd.sh: GitHub login available on Builder VM" >&2; exit 1; fi; if command -v run >/dev/null && run github-app get-token >/dev/null 2>&1; then echo "boxd.sh: GitHub App available on Builder VM" >&2; exit 1; fi' </dev/null || return 1
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
  local vm_args=("ru-$name" --from-snapshot "$SNAPSHOT")
  [ "$AGENT_KIND" = codex ] || vm_args+=(--isolated)
  vm_args+=(--auto-suspend-timeout 0 --auto-destroy-timeout "$VM_TTL")
  boxd machine new "${vm_args[@]}" >/dev/null </dev/null
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

# Put <ref> in ~/roundup on the VM as a git repo: tag `base` is <base-ref>, HEAD is <ref>. A non-empty <prompt>
# file is copied to /tmp/prompt.md. A commit ref gets its real history (L22): nothing is replayed or folded, so
# a merge, a trailer and an empty commit all survive exactly as they are on the real branch. A tree ref (the
# merged tree `check` provisions) has no history to carry, so it is just committed on top of base.
upload_checkout() {
  local name=$1 base_ref=$2 ref=$3 prompt=$4
  [ -z "$prompt" ] || boxd machine cp "$prompt" "$VM:/tmp/prompt.md" >/dev/null </dev/null
  if [ "$(git cat-file -t --end-of-options "$ref")" = commit ]; then
    upload_commit_checkout "$name" "$base_ref" "$ref"
  else
    upload_tree_checkout "$base_ref" "$ref"
  fi
}

# A bundle's refs are named, not raw SHAs, so <name>-ref and <name>-base stand in for <ref> and <base_ref> only
# long enough to build it, named per-run so parallel reviews (swarm) cannot race on the same temporary ref. The
# VM fetches the bundle, checks out <ref>'s own commit and tags <base_ref>'s commit `base`: the checkout IS the
# branch, with every commit's author, message and trailer exactly as the real branch has them.
upload_commit_checkout() {
  local name=$1 base_ref=$2 ref=$3 base_sha ref_sha
  local ref_name="refs/boxd-review/$name-ref" base_name="refs/boxd-review/$name-base"
  ref_sha=$(git rev-parse --verify --end-of-options "$ref")
  base_sha=$(git rev-parse --verify --end-of-options "$base_ref")
  git update-ref "$ref_name" "$ref_sha"
  git update-ref "$base_name" "$base_sha"
  git bundle create - "$ref_name" "$base_name" | boxd machine cp - "$VM:/tmp/r.bundle" >/dev/null
  git update-ref -d "$ref_name"
  git update-ref -d "$base_name"
  boxd machine exec "$VM" -- "mkdir -p ~/roundup && cd ~/roundup && git init -q && git fetch -q /tmp/r.bundle '+refs/*:refs/bundle/*' && git checkout -q --detach $ref_sha && git tag base $base_sha" </dev/null
}

# A tree has no history: commit it on top of base, matching what `check` provisions for a merged PR tree.
upload_tree_checkout() {
  local base_ref=$1 ref=$2
  git archive --format=tar.gz --end-of-options "$base_ref" | boxd machine cp - "$VM:/tmp/base.tgz" >/dev/null
  git archive --format=tar.gz --end-of-options "$ref" | boxd machine cp - "$VM:/tmp/src.tgz" >/dev/null
  boxd machine exec "$VM" -- 'mkdir -p ~/roundup && tar xzf /tmp/base.tgz -C ~/roundup && cd ~/roundup &&
    git init -q && git add -A >/dev/null && git -c user.email=builder@roundup -c user.name=builder commit -qm base && git tag base &&
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
      ( set -e; [ "$AGENT_KIND" != codex ] || transfer_codex_login; upload_checkout "$name" "$base_ref" "$ref" "$prompt" )
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

run_codex() {
  local stream=$1 role=$2 rc=0 last context
  if [ "$role" = build ]; then
    context='The VM checkout at ~/roundup is your dedicated worktree. Edit it directly. Do not create a second worktree or keep the only patch outside this checkout.'
  else
    context='The VM checkout at ~/roundup is the repository for this review. Do not create another worktree.'
  fi
  "$TIMEOUT_CMD" "$AGENT_TIMEOUT" boxd machine exec "$VM" --timeout "$AGENT_TIMEOUT" -- \
    "cd ~/roundup && . ~/.cargo/env && { printf '%s\\n' '$context'; cat /tmp/prompt.md; } | codex exec --json --ephemeral --ignore-user-config --dangerously-bypass-approvals-and-sandbox -" </dev/null >"$stream" 2>"${stream%.jsonl}.err" || rc=$?
  cat "${stream%.jsonl}.err" >&2
  [ -s "$stream" ] || { echo "boxd.sh: Codex produced no output" >&2; return 1; }
  last=$(tail -n 1 "$stream")
  if jq -e '.type == "turn.failed"' <<<"$last" >/dev/null 2>&1; then
    echo "boxd.sh: Codex turn failed: $stream" >&2
    return 1
  fi
  if ! jq -e '.type == "turn.completed"' <<<"$last" >/dev/null 2>&1; then
    echo "boxd.sh: Codex turn incomplete (exit $rc): $stream" >&2
    return 2
  fi
  [ "$rc" -eq 0 ] || { echo "boxd.sh: Codex exited $rc after completion" >&2; return 1; }
}

# Run claude on the VM with /tmp/prompt.md; each event it produces lands in <stream> as it happens (L21). The
# verdict, the cost line and L4's pause come only from the stream's last event, which must be a result event.
# Exits 1 when the run produced no event (even if the wall-clock timeout ended it) and when the final result is an
# error (`agent failed`). Returns 2 when it produced events but ended with no final result event (a wall-clock
# timeout among them), so the caller can save what the run got done before deciding its own exit code.
run_agent() {
  local model=$1 stream=$2 rc=0 last
  # A feature-sized task outlasts 570 s (measured: V7 Todo triage hit it and returned nothing), so wait as long as the check may.
  # claude exits non-zero on an API error; keep going so the limit guard can see it.
  # Without CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC, claude waits ~90 s on blocked hosts after it has already answered.
  # boxd's own --timeout is a no-output deadline: a run that keeps streaming events never trips it. `timeout` puts
  # the command in its own process group and signals that whole group, so a streaming agent still ends at
  # BOXD_AGENT_TIMEOUT; its exit code 124 is how the branch below tells a wall-clock cutoff from a plain early exit.
  "$TIMEOUT_CMD" "$AGENT_TIMEOUT" boxd machine exec "$VM" --timeout "$AGENT_TIMEOUT" -e CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1 -- \
    "cd ~/roundup && . ~/.cargo/env && claude -p --model $model --output-format stream-json --verbose --dangerously-skip-permissions 2>/dev/null </tmp/prompt.md" </dev/null >"$stream" 2>"${stream%.jsonl}.err" || rc=$?
  cat "${stream%.jsonl}.err" >&2
  ! grep -q DeadlineExceeded "${stream%.jsonl}.err" || record_event agent deadline-exceeded
  [ -s "$stream" ] || { record_event agent no-output; echo "boxd.sh: agent produced no output" >&2; exit 1; }
  last=$(tail -n 1 "$stream")
  if ! jq -e '.type == "result"' <<<"$last" >/dev/null 2>&1; then
    if [ "$rc" -eq 124 ]; then
      echo "boxd.sh: agent timed out after ${AGENT_TIMEOUT} s: $stream" >&2
    else
      echo "boxd.sh: agent ended without a result: $stream" >&2
    fi
    return 2
  fi
  guard_limits "$last"
  jq -e '.is_error == false' <<<"$last" >/dev/null || { echo "boxd.sh: agent failed: $(jq -r .result <<<"$last")" >&2; exit 1; }
}

# The check both `build` and `check` observe.
run_check() {
  boxd machine exec "$VM" --timeout 1800 -- "cd ~/roundup && . ~/.cargo/env && sudo rm -rf $STRAY_NODE_MODULES && pnpm install --frozen-lockfile && just check" </dev/null
}

review() {
  local name=$1 prompt=$2 ref=${3:-HEAD}
  validate_args "$name" "$prompt"
  local stream="$OUT/runs/$name-$RUN_ID.jsonl" verdict="$OUT/verdicts/$name.md" base agent_rc=0 last
  rm -f "$verdict"
  base="$(git merge-base --end-of-options origin/main "$ref")" || { echo "boxd.sh: no merge-base of origin/main and $ref; git fetch origin" >&2; exit 1; }
  require_agent
  provision "$name" "$base" "$ref" "$prompt"
  if [ "$AGENT_KIND" = codex ]; then
    run_codex "$stream" review || agent_rc=$?
    [ "$agent_rc" -eq 0 ] || exit 1
    local answer
    answer=$(jq -ers '
      if any(.[]; .type == "turn.failed" or .type == "error") then error("Codex stream failed") else
        [.[] | select(.type == "item.completed" and .item.type == "agent_message")
          | .item.text | select(type == "string") | select(test("\\S"))]
        | last // error("Codex produced no answer")
      end' "$stream") || { echo "boxd.sh: no valid Codex verdict: $stream" >&2; exit 1; }
    printf '%s\n' "$answer" >"$verdict"
    return 0
  fi
  run_agent "${BOXD_MODEL:-opus}" "$stream" || agent_rc=$?
  [ "$agent_rc" -eq 0 ] || exit 1
  last=$(tail -n 1 "$stream")
  jq -r .result <<<"$last" >"$verdict"
  jq -r '"boxd.sh: \(.num_turns) turns, \(.duration_ms / 1000 | floor)s, $\(.total_cost_usd) notional"' <<<"$last"
}

save_partial_patch() {
  local partial=$1
  PARTIAL_TMP=$partial.tmp
  if boxd machine exec "$VM" -- 'cd ~/roundup && git add -A && git diff --cached base' </dev/null >"$partial.tmp"; then
    mv "$partial.tmp" "$partial"
    PARTIAL_TMP=
  else
    rm -f "$partial.tmp"
    echo "boxd.sh: could not read the diff so far from $VM; no $partial saved" >&2
    return 1
  fi
}

build() {
  local name=$1 prompt=$2
  validate_args "$name" "$prompt"
  local stream="$OUT/runs/$name-$RUN_ID.jsonl" patch="$OUT/patches/$name.patch" partial="$OUT/patches/$name.partial.patch" checklog="$OUT/runs/$name-$RUN_ID.check.log" agent_rc=0 check_rc=0 last
  require_agent
  # A result file is always from the latest run: no-output and a pause leave run_agent through `exit`, never back to
  # this function. A timeout, or a run that produced events but ended with no result, returns 2 here instead, and
  # this function exits after saving the partial patch below, so an earlier run's stale patch can only be cleared here.
  rm -f "$patch" "$partial"
  provision "$name" HEAD HEAD "$prompt"
  if [ "$AGENT_KIND" = codex ]; then
    run_codex "$stream" build || agent_rc=$?
  else
    run_agent "${BOXD_MODEL:-sonnet}" "$stream" || agent_rc=$?
  fi
  if [ "$agent_rc" -eq 2 ]; then
    save_partial_patch "$partial" || true
    exit 1
  fi
  [ "$agent_rc" -eq 0 ] || exit "$agent_rc"
  save_partial_patch "$partial" || exit 1
  [ -s "$partial" ] || { echo "boxd.sh: $VM completed with no changes in ~/roundup; Builder output outside the checkout is not collected" >&2; exit 1; }
  # The observer is our own check, not the Builder's claim that it passed.
  run_check >"$checklog" 2>&1 || check_rc=$?
  tail -n 15 "$checklog" >&2
  ! grep -q DeadlineExceeded "$checklog" || record_event check deadline-exceeded
  [ "$check_rc" -eq 0 ] || { echo "boxd.sh: check failed; Builder changes remain in $partial" >&2; exit "$check_rc"; }
  PARTIAL_TMP=$patch.tmp
  boxd machine exec "$VM" -- 'cd ~/roundup && git add -A && { git diff --cached --quiet || git -c user.email=builder@roundup -c user.name=builder commit -qm "builder: task" -m "Author-Agent: builder"; } && git format-patch base --stdout' </dev/null >"$PARTIAL_TMP"
  [ -s "$PARTIAL_TMP" ] || { echo "boxd.sh: $VM produced no final patch despite a nonempty candidate" >&2; exit 1; }
  mv "$PARTIAL_TMP" "$patch"
  PARTIAL_TMP=
  rm -f "$partial"
  last=$(tail -n 1 "$stream")
  if [ "$AGENT_KIND" = codex ]; then
    jq -r '"boxd.sh: Codex complete, \(.usage.input_tokens // 0) input and \(.usage.output_tokens // 0) output tokens"' <<<"$last"
  else
    jq -r '"boxd.sh: \(.num_turns) turns, \(.duration_ms / 1000 | floor)s, $\(.total_cost_usd) notional"' <<<"$last"
  fi
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

# A failed `boxd machine list`, or no boxd at all, exits 1: an empty list must mean no VMs, never an unreadable one.
status() {
  local vm state vms
  vms=$(boxd machine list --json </dev/null | jq -r '.[] | select(.name | startswith("ru-")) | .name') || { echo "boxd.sh: status: boxd machine list failed" >&2; exit 1; }
  while read -r vm; do
    [ -n "$vm" ] || continue
    state=$(boxd machine exec "$vm" --timeout 10 -- 'if pgrep -x claude >/dev/null || pgrep -x codex >/dev/null; then echo agent-running; else echo idle; fi' </dev/null 2>/dev/null) || state=unreachable
    echo "$vm $state"
  done <<<"$vms"
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
  build) need_timeout; build "${2:?name}" "${3:?prompt-file}" ;;
  check) check "${2:?pr-number-or-branch}" ;;
  swarm) need_timeout; shift; swarm "$@" ;;
  status) status ;;
  kill) kill_vms "${2:?name or all}" ;;
  review) need_timeout; review "${2:?name}" "${3:?prompt-file}" "${4:-HEAD}" ;;
  *) sed -n '2,14p' "$0" >&2; exit 2 ;;
esac
