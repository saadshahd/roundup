#!/usr/bin/env bash
# Percy builds of the harness. Usage: loop/percy.sh snapshot [port] | build <sha>
# L36 snapshot: serve `just harness` and snapshot every `.agents/data/harness.md` seed in Percy. Exits 1 without
# PERCY_TOKEN (Percy would skip and pass), when the harness never answers, or without a build link.
# L24 build: print the id of the Percy build made for commit <sha>. Exits 1 when none exists, 4 when the Percy API fails.
set -euo pipefail
cd "$(dirname "$0")/.."

snapshot() {
  [ -n "${PERCY_TOKEN:-}" ] || { echo "L36 percy: PERCY_TOKEN is not set; a skipped Percy build is never visual proof" >&2; exit 1; }
  local port=${1:-5199} wait=${HARNESS_WAIT:-120} seeds seed link
  # Global, since the EXIT trap reads them after this function returns.
  work=$(mktemp -d)

  seeds=$(sed -n '/^| Seed |/,/^$/p' .agents/data/harness.md | sed -nE 's/^\| `([a-z0-9-]+)` \|.*/\1/p')
  [ -n "$seeds" ] || { echo "L36 percy: no seeds in .agents/data/harness.md" >&2; exit 1; }
  for seed in $seeds; do
    printf -- '- name: %s\n  url: /harness.html?seed=%s\n  widths: [1280]\n  minHeight: 800\n  browsers: [chrome]\n  waitForSelector: "#root > *"\n' "$seed" "$seed"
  done >"$work/snapshots.yml"

  # Job control gives the harness its own process group, so one kill stops just, pnpm and Vite.
  set -m
  just harness first-run "$port" >"$work/harness.log" 2>&1 &
  harness=$!
  trap 'kill -- -"$harness" 2>/dev/null || true; rm -rf "$work"' EXIT
  for _ in $(seq "$wait"); do
    curl -sf -o /dev/null "http://localhost:$port/harness.html" && break
    kill -0 "$harness" 2>/dev/null || { cat "$work/harness.log" >&2; echo "L36 percy: the harness exited" >&2; exit 1; }
    sleep 1
  done
  curl -sf -o /dev/null "http://localhost:$port/harness.html" || { echo "L36 percy: the harness did not answer in ${wait}s" >&2; exit 1; }

  pnpm exec percy snapshot "$work/snapshots.yml" --base-url "http://localhost:$port" | tee "$work/percy.log"
  link=$(grep -oE 'https://percy\.io/[^ ]+/builds/[0-9]+' "$work/percy.log" | tail -n1) || true
  [ -n "$link" ] || { echo "L36 percy: no Percy build link in the output" >&2; exit 1; }
  [ -z "${GITHUB_STEP_SUMMARY:-}" ] || echo "Percy: $link" >>"$GITHUB_STEP_SUMMARY"
}

# visual pins each build's commit to the PR head (PERCY_COMMIT), so a head's build is found by its SHA. The Reviewer
# starts when a PR opens, before visual has started its build, so it asks PERCY_WAIT seconds (default 0) and never delays the model.
build() {
  [ -n "${PERCY_TOKEN:-}" ] || { echo "L24 percy: PERCY_TOKEN is not set" >&2; exit 2; }
  local sha=$1 wait=${PERCY_WAIT:-0} waited=0 found id
  while :; do
    found=$(curl -sS -g --fail-with-body --max-time 30 -H "Authorization: Token token=$PERCY_TOKEN" \
      "https://percy.io/api/v1/builds?filter[sha]=$sha&page[limit]=1") || { echo "L24 percy: the Percy API failed: $found" >&2; exit 4; }
    id=$(jq -r '.data[0].id // empty' <<<"$found") || { echo "L24 percy: the Percy API answered no JSON: $found" >&2; exit 4; }
    [ -z "$id" ] || { echo "$id"; return 0; }
    [ "$waited" -lt "$wait" ] || { echo "L24 percy: no Percy build for $sha after ${wait}s" >&2; exit 1; }
    sleep 15
    waited=$((waited + 15))
  done
}

case "${1:-}" in
  snapshot) snapshot "${2:-}" ;;
  build) [[ ${2:-} =~ ^[0-9a-f]{40}$ ]] || { sed -n '2p' "$0" >&2; exit 2; }; build "$2" ;;
  *) sed -n '2p' "$0" >&2; exit 2 ;;
esac
