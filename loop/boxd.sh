#!/usr/bin/env bash
# Run one Builder task in a disposable, isolated boxd VM. Usage:
#   loop/boxd.sh bake                       # build the ru-toolchain snapshot (once per toolchain change)
#   loop/boxd.sh build <name> <prompt-file> # Builder run; writes loop/out/patches/<name>.patch
#   loop/boxd.sh review <name> <prompt-file> [ref] # Reviewer run on ref (default HEAD) against its merge-base with origin/main;
#                                           # writes the Reviewer's answer to loop/out/verdicts/<name>.md. The checkout is a
#                                           # git repo with tag `base`, so the Reviewer can run `loop/rules.sh size base`.
# BOXD_MAX_VMS caps concurrent ru- VMs (default 12). BOXD_MODEL overrides the model (build: sonnet, review: opus).
# Auth is the boxd secret CLAUDE_CODE_OAUTH_TOKEN (sealed, host-scoped): each VM sees only a placeholder, boxd swaps in the real token for *.anthropic.com, *.claude.com and claude.ai.
# Exit codes: 0 ok, 1 failure, 75 paused (limit hit, or loop/out/PAUSED exists).
set -euo pipefail
cd "$(dirname "$0")/.."

SNAPSHOT=ru-toolchain
OUT=loop/out
PAUSED=$OUT/PAUSED
EX_PAUSED=75
MAX_VMS=${BOXD_MAX_VMS:-12}
VM=
LOCK=$OUT/lock
LOCK_WAIT=${BOXD_LOCK_WAIT:-60}
lock_held=0

cleanup() {
  local rc=$?
  if [ "$lock_held" -eq 1 ]; then rmdir "$LOCK" || true; fi
  if [ -n "$VM" ]; then
    boxd machine remove "$VM" -y >/dev/null </dev/null || echo "boxd.sh: LEAKED VM $VM; remove it by hand" >&2
  fi
  exit "$rc"
}
trap cleanup EXIT

bake() {
  # Own the name only once `new` succeeds, so a failed create never removes another bake's VM.
  boxd machine new ru-bake --auto-suspend-timeout 0 --auto-destroy-timeout 3600 >/dev/null </dev/null
  VM=ru-bake
  local rust pnpm
  rust="$(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml)"
  pnpm="$(jq -r '.packageManager | sub("pnpm@"; "")' package.json)"
  boxd machine exec "$VM" --timeout 580 -- "set -e
    curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain $rust -c clippy -c rustfmt >/dev/null 2>&1
    . ~/.cargo/env
    curl -LsSf https://get.nexte.st/latest/linux | tar zxf - -C ~/.cargo/bin
    sudo apt-get update -qq && sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libsoup-3.0-dev pkg-config >/dev/null
    cargo install cargo-machete just --locked >/dev/null 2>&1
    npm i -g pnpm@$pnpm >/dev/null 2>&1
    sudo ln -sf \"\$(ls -d ~/.nvm/versions/node/*/bin/pnpm | head -1)\" /usr/local/bin/pnpm
    pnpm --version" </dev/null
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
  [[ $MAX_VMS =~ ^[0-9]+$ ]] || { echo "boxd.sh: BOXD_MAX_VMS must be a number, got: $MAX_VMS" >&2; exit 2; }
  local name=$1 prompt=$2
  [[ $name =~ ^[a-z0-9][a-z0-9-]*$ ]] || { echo "boxd.sh: name must match ^[a-z0-9][a-z0-9-]*\$, got: $name" >&2; exit 2; }
  [[ $prompt != -* ]] || { echo "boxd.sh: prompt file must not start with '-': $prompt" >&2; exit 2; }
  [ -f "$prompt" ] || { echo "boxd.sh: no prompt file: $prompt" >&2; exit 2; }
}

vm_count() { boxd machine list --json </dev/null | jq '[.[] | select(.name | startswith("ru-"))] | length'; }

# Create the isolated VM for <name>, enforcing pause, token and cap. Sets $VM.
start_vm() {
  local name=$1
  boxd env list --json </dev/null | jq -e 'any(.[]; .name == "CLAUDE_CODE_OAUTH_TOKEN")' >/dev/null || { echo "boxd.sh: boxd secret CLAUDE_CODE_OAUTH_TOKEN is missing (boxd env set --secret, allowed for *.anthropic.com, *.claude.com, claude.ai)" >&2; exit 1; }
  mkdir -p "$OUT/patches" "$OUT/runs" "$OUT/verdicts"
  [ ! -e "$PAUSED" ] || pause "paused since $(cat "$PAUSED")"
  acquire_lock
  [ "$(vm_count)" -lt "$MAX_VMS" ] || { echo "boxd.sh: $MAX_VMS ru- VMs already exist" >&2; exit 1; }
  # Own the name only once `new` succeeds, so a failed create never removes someone else's VM.
  boxd machine new "ru-$name" --from-snapshot "$SNAPSHOT" --isolated --auto-suspend-timeout 0 --auto-destroy-timeout 1800 >/dev/null </dev/null
  VM="ru-$name"
  release_lock
  # A VM restored from a memory snapshot wedges claude and tsc until it is rebooted (measured: tsc hangs before, 0.4 s after).
  boxd machine reboot "$VM" >/dev/null </dev/null
  local _
  for _ in $(seq 60); do
    boxd machine exec "$VM" --timeout 5 -- true </dev/null >/dev/null 2>&1 && return 0
    sleep 1
  done
  echo "boxd.sh: $VM did not answer after reboot" >&2
  exit 1
}

# Put <ref> in ~/roundup on the VM as a git repo: tag `base` is <base-ref>'s tree, HEAD is <ref>'s tree.
upload_checkout() {
  local base_ref=$1 ref=$2 prompt=$3
  git archive --format=tar.gz --end-of-options "$base_ref" | boxd machine cp - "$VM:/tmp/base.tgz" >/dev/null
  git archive --format=tar.gz --end-of-options "$ref" | boxd machine cp - "$VM:/tmp/src.tgz" >/dev/null
  boxd machine cp "$prompt" "$VM:/tmp/prompt.md" >/dev/null </dev/null
  boxd machine exec "$VM" -- 'mkdir -p ~/roundup && tar xzf /tmp/base.tgz -C ~/roundup && cd ~/roundup &&
    git init -q && git add -A >/dev/null && git -c user.email=builder@roundup -c user.name=builder commit -qm base && git tag base &&
    git rm -rqf . && tar xzf /tmp/src.tgz -C ~/roundup && git add -A >/dev/null &&
    { git diff --cached --quiet || git -c user.email=builder@roundup -c user.name=builder commit -qm head; }' </dev/null
}

# Run claude on the VM with /tmp/prompt.md; JSON lands in <result>. Exits unless the run succeeded.
run_agent() {
  local model=$1 result=$2
  # claude exits non-zero on an API error; keep going so the limit guard can see it.
  # Without CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC, claude waits ~90 s on blocked hosts after it has already answered.
  boxd machine exec "$VM" --timeout 570 -e CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1 -- \
    "cd ~/roundup && . ~/.cargo/env && claude -p --model $model --output-format json --dangerously-skip-permissions 2>/dev/null </tmp/prompt.md" </dev/null >"$result" || true
  [ -s "$result" ] || { echo "boxd.sh: agent produced no output" >&2; exit 1; }
  guard_limits "$result"
  jq -e '.is_error == false' "$result" >/dev/null || { echo "boxd.sh: agent failed: $(jq -r .result "$result")" >&2; exit 1; }
}

review() {
  local name=$1 prompt=$2 ref=${3:-HEAD}
  validate_args "$name" "$prompt"
  local result="$OUT/runs/$name.json" verdict="$OUT/verdicts/$name.md" base
  base="$(git merge-base --end-of-options origin/main "$ref")" || { echo "boxd.sh: no merge-base of origin/main and $ref; git fetch origin" >&2; exit 1; }
  start_vm "$name"
  upload_checkout "$base" "$ref" "$prompt"
  run_agent "${BOXD_MODEL:-opus}" "$result"
  jq -r .result "$result" >"$verdict"
  jq -r '"boxd.sh: \(.num_turns) turns, \(.duration_ms / 1000 | floor)s, $\(.total_cost_usd) notional"' "$result"
}

build() {
  local name=$1 prompt=$2
  validate_args "$name" "$prompt"
  local result="$OUT/runs/$name.json" patch="$OUT/patches/$name.patch" checklog="$OUT/runs/$name.check.log"
  start_vm "$name"
  upload_checkout HEAD HEAD "$prompt"
  run_agent "${BOXD_MODEL:-sonnet}" "$result"
  # The observer is our own check, not the Builder's claim that it passed.
  local rc=0
  boxd machine exec "$VM" --timeout 570 -- 'cd ~/roundup && . ~/.cargo/env && pnpm install --frozen-lockfile && just check' </dev/null >"$checklog" 2>&1 || rc=$?
  tail -n 15 "$checklog" >&2
  [ "$rc" -eq 0 ] || exit "$rc"
  boxd machine exec "$VM" -- 'cd ~/roundup && git add -A && { git diff --cached --quiet || git -c user.email=builder@roundup -c user.name=builder commit -qm "builder: task" -m "Author-Agent: builder"; } && git format-patch base --stdout' </dev/null >"$patch"
  jq -r '"boxd.sh: \(.num_turns) turns, \(.duration_ms / 1000 | floor)s, $\(.total_cost_usd) notional"' "$result"
}

case "${1:-}" in
  bake) bake ;;
  build) build "${2:?name}" "${3:?prompt-file}" ;;
  review) review "${2:?name}" "${3:?prompt-file}" "${4:-HEAD}" ;;
  *) sed -n '2,12p' "$0" >&2; exit 2 ;;
esac
