#!/usr/bin/env bash
# L36: serve `just harness` and snapshot every `.agents/data/harness.md` seed in Percy. Usage: loop/percy.sh [port]
# Exits 1 without PERCY_TOKEN (Percy would skip and pass), when the harness never answers, or without a build link.
set -euo pipefail
cd "$(dirname "$0")/.."
[ -n "${PERCY_TOKEN:-}" ] || { echo "L36 percy: PERCY_TOKEN is not set; a skipped Percy build is never visual proof" >&2; exit 1; }
port=${1:-5199}
wait=${HARNESS_WAIT:-120}
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
