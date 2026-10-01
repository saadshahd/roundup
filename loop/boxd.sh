#!/usr/bin/env bash
# Run one Builder task in a disposable boxd VM. Usage:
#   loop/boxd.sh bake                       # build the ru-toolchain snapshot (once per toolchain change)
#   loop/boxd.sh build <name> <prompt-file> # Builder run; writes loop/out/patches/<name>.patch
# The token is read from $CLAUDE_CODE_OAUTH_TOKEN and passed per call, never stored on the VM.
set -euo pipefail
cd "$(dirname "$0")/.."

SNAPSHOT=ru-toolchain
PAUSED=loop/out/PAUSED
EX_PAUSED=75
VM=

bake() {
  VM=ru-bake
  boxd machine new "$VM" --auto-suspend-timeout 0 --auto-destroy-timeout 3600 >/dev/null
  trap 'boxd machine remove "$VM" -y >/dev/null' EXIT
  local rust
  rust="$(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml)"
  local pnpm
  pnpm="$(jq -r '.packageManager | sub("pnpm@"; "")' package.json)"
  boxd machine exec "$VM" --timeout 580 -- "set -e
    curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain $rust -c clippy -c rustfmt >/dev/null 2>&1
    . ~/.cargo/env
    curl -LsSf https://get.nexte.st/latest/linux | tar zxf - -C ~/.cargo/bin
    cargo install cargo-machete --locked >/dev/null 2>&1
    cargo install just --locked >/dev/null 2>&1
    npm i -g pnpm@$pnpm >/dev/null 2>&1
    sudo ln -sf \"\$(ls -d ~/.nvm/versions/node/*/bin/pnpm | head -1)\" /usr/local/bin/pnpm
    pnpm --version"
  boxd snapshots save "$VM" "$SNAPSHOT"
}

# Pause on the first rate-limit or usage-limit answer. The JSON has no quota field, so the error is the signal.
guard_limits() {
  local result=$1
  if jq -e '.api_error_status == 429 or ((.result // "") | test("usage limit|rate limit"; "i"))' "$result" >/dev/null; then
    date -u +%FT%TZ >"$PAUSED"
    echo "boxd.sh: limit hit, wrote $PAUSED; delete it to resume" >&2
    exit "$EX_PAUSED"
  fi
}

build() {
  local name=$1 prompt=$2
  VM="ru-$1"
  : "${CLAUDE_CODE_OAUTH_TOKEN:?set CLAUDE_CODE_OAUTH_TOKEN (claude setup-token)}"
  [ ! -e "$PAUSED" ] || { echo "boxd.sh: paused since $(cat "$PAUSED")" >&2; exit "$EX_PAUSED"; }
  mkdir -p loop/out/patches loop/out/runs

  boxd machine new "$VM" --from-snapshot "$SNAPSHOT" --auto-suspend-timeout 0 --auto-destroy-timeout 1800 >/dev/null
  trap 'boxd machine remove "$VM" -y >/dev/null' EXIT

  git archive --format=tar.gz HEAD | boxd machine cp - "$VM:/tmp/src.tgz" >/dev/null
  boxd machine cp "$prompt" "$VM:/tmp/prompt.md" >/dev/null
  boxd machine exec "$VM" -- 'mkdir -p ~/roundup && tar xzf /tmp/src.tgz -C ~/roundup && cd ~/roundup &&
    git init -q && git add -A >/dev/null && git -c user.email=builder@roundup -c user.name=builder commit -qm base'

  local result="loop/out/runs/$name.json"
  # shellcheck disable=SC2016 # $(cat ...) must expand inside the VM
  boxd machine exec "$VM" --timeout 570 -e CLAUDE_CODE_OAUTH_TOKEN="$CLAUDE_CODE_OAUTH_TOKEN" -- \
    'cd ~/roundup && . ~/.cargo/env && claude -p --model sonnet --output-format json --dangerously-skip-permissions "$(cat /tmp/prompt.md)" </dev/null 2>/dev/null' >"$result"
  guard_limits "$result"
  jq -e '.is_error == false' "$result" >/dev/null || { echo "boxd.sh: Builder failed: $(jq -r .result "$result")" >&2; exit 1; }

  # The observer is our own check, not the Builder's claim that it passed.
  boxd machine exec "$VM" --timeout 570 -- 'cd ~/roundup && . ~/.cargo/env && pnpm install --frozen-lockfile && just check'

  boxd machine exec "$VM" -- 'cd ~/roundup && git add -A && git -c user.email=builder@roundup -c user.name=builder commit -qm "builder: task" -m "Author-Agent: builder" && git format-patch -1 --stdout' >"loop/out/patches/$name.patch"
  jq -r '"boxd.sh: \(.num_turns) turns, \(.duration_ms / 1000 | floor)s, $\(.total_cost_usd) notional"' "$result"
}

case "${1:-}" in
  bake) bake ;;
  build) build "${2:?name}" "${3:?prompt-file}" ;;
  *) sed -n '2,6p' "$0" >&2; exit 2 ;;
esac
