#!/usr/bin/env bash
# Run one Builder task in a disposable, isolated boxd VM. Usage:
#   loop/boxd.sh bake                       # build the ru-toolchain snapshot (once per toolchain change)
#   loop/boxd.sh build <name> <prompt-file> # Builder run; writes loop/out/patches/<name>.patch
#   loop/boxd.sh review <name> <prompt-file> [ref] # Reviewer run on ref (default HEAD) against its merge-base with origin/main;
#                                           # writes the Reviewer's answer to loop/out/verdicts/<name>.md. The checkout is a
#                                           # git repo with tag `base`, so the Reviewer can run `loop/rules.sh size base`.
#   loop/boxd.sh check <pr-number|branch>   # merge it into origin/main here, run `just check` on a fresh isolated VM
# BOXD_MODEL overrides the model (build: sonnet, review: opus).
# Auth is the boxd secret CLAUDE_CODE_OAUTH_TOKEN (sealed, host-scoped): each VM sees only a placeholder, boxd swaps in the real token for *.anthropic.com, *.claude.com and claude.ai.
# Exit codes: 0 ok, 1 failure, 75 paused (limit hit, or loop/out/PAUSED exists).
set -euo pipefail
cd "$(dirname "$0")/.."

SNAPSHOT=ru-toolchain
OUT=loop/out
PAUSED=$OUT/PAUSED
EX_PAUSED=75
MAX_VMS=4
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

# A token baked into a snapshot reaches every VM made from it, so refuse to save one that contains any. The scan skips
# the Bun package cache, whose docs hold placeholder strings that match the pattern.
TOKEN_PATTERN='gho_[A-Za-z0-9]{20,}|ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|sk-ant-[A-Za-z0-9_-]{20,}'

# Install, then clippy and test builds of the lockfile's dependencies, so a later `check` only compiles the workspace crates.
# The target dir lives outside the checkout (see CARGO_TARGET_DIR in ~/.cargo/env below).
WARM='set -e; . ~/.cargo/env; mkdir /tmp/warm && tar xzf /tmp/warm.tgz -C /tmp/warm && cd /tmp/warm
pnpm install --frozen-lockfile >/dev/null
cargo clippy --all-targets >/dev/null 2>&1
cargo nextest run --no-run >/dev/null 2>&1
cd; rm -rf /tmp/warm /tmp/warm.tgz'

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

# Create the isolated VM for <name>, enforcing the cap. Sets $VM.
start_vm() {
  local name=$1
  mkdir -p "$OUT/runs"
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

# Put <ref> in ~/roundup on the VM as a git repo: tag `base` is <base-ref>'s tree, HEAD is <ref>'s tree. A non-empty
# <prompt> file is copied to /tmp/prompt.md.
upload_checkout() {
  local base_ref=$1 ref=$2 prompt=$3
  git archive --format=tar.gz --end-of-options "$base_ref" | boxd machine cp - "$VM:/tmp/base.tgz" >/dev/null
  git archive --format=tar.gz --end-of-options "$ref" | boxd machine cp - "$VM:/tmp/src.tgz" >/dev/null
  [ -z "$prompt" ] || boxd machine cp "$prompt" "$VM:/tmp/prompt.md" >/dev/null </dev/null
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

# The check both `build` and `check` observe: install from the lockfile, then the repo's own definition of check.
run_check() {
  boxd machine exec "$VM" --timeout 1800 -- 'cd ~/roundup && . ~/.cargo/env && pnpm install --frozen-lockfile && just check' </dev/null
}

review() {
  local name=$1 prompt=$2 ref=${3:-HEAD}
  validate_args "$name" "$prompt"
  local result="$OUT/runs/$name.json" verdict="$OUT/verdicts/$name.md" base
  base="$(git merge-base --end-of-options origin/main "$ref")" || { echo "boxd.sh: no merge-base of origin/main and $ref; git fetch origin" >&2; exit 1; }
  require_claude
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
  require_claude
  start_vm "$name"
  upload_checkout HEAD HEAD "$prompt"
  run_agent "${BOXD_MODEL:-sonnet}" "$result"
  # The observer is our own check, not the Builder's claim that it passed.
  local rc=0
  run_check >"$checklog" 2>&1 || rc=$?
  tail -n 15 "$checklog" >&2
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
  log="$OUT/runs/check-$slug.log"
  # Per-run ref names (not origin/main, not FETCH_HEAD) so checks started in parallel from one clone cannot race.
  git fetch -q origin -- "+refs/heads/main:refs/boxd-check/$slug-base" "+$refspec:refs/boxd-check/$slug"
  base=$(git rev-parse "refs/boxd-check/$slug-base")
  sha=$(git rev-parse "refs/boxd-check/$slug")
  git update-ref -d "refs/boxd-check/$slug-base"
  git update-ref -d "refs/boxd-check/$slug"
  tree=$(git merge-tree --write-tree "$base" "$sha")
  start_vm "chk-$slug"
  upload_checkout "$base" "$tree" ""
  local rc=0
  run_check 2>&1 | mask_github_tokens >"$log" || rc=${PIPESTATUS[0]}
  tail -n 25 "$log" >&2
  echo "boxd.sh: check $ref (merged into origin/main) exit $rc; full log $log" >&2
  exit "$rc"
}

case "${1:-}" in
  bake) bake ;;
  build) build "${2:?name}" "${3:?prompt-file}" ;;
  check) check "${2:?pr-number-or-branch}" ;;
  review) review "${2:?name}" "${3:?prompt-file}" "${4:-HEAD}" ;;
  *) sed -n '2,12p' "$0" >&2; exit 2 ;;
esac
