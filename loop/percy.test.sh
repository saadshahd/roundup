#!/usr/bin/env bash
# Tests for loop/percy.sh (L36) with fake `just` and `pnpm`. Usage: loop/percy.test.sh
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
mkdir -p "$dir/bin" "$dir/site"
echo '<div id="root"></div>' >"$dir/site/harness.html"
# The harness: a static server on the asked port, or one that dies at once.
cat >"$dir/bin/just" <<'JUST'
#!/usr/bin/env bash
echo "$*" >>"$TRACE"
[ "$CASE" != harness_dies ] || exit 1
cd "$SITE" && exec python3 -m http.server "$3" --bind 127.0.0.1
JUST
cat >"$dir/bin/pnpm" <<'PNPM'
#!/usr/bin/env bash
echo "$*" >>"$TRACE"
cp "$4" "$SNAPSHOTS"
[ "$CASE" = no_link ] || echo "[percy] Finalized build #7: https://percy.io/abc/web/roundup/builds/42"
[ "$CASE" != percy_fails ]
PNPM
chmod +x "$dir/bin/just" "$dir/bin/pnpm"
export PATH="$dir/bin:$PATH" SITE="$dir/site" TRACE="$dir/trace" SNAPSHOTS="$dir/snapshots.yml" CASE
export GITHUB_STEP_SUMMARY="$dir/summary" HARNESS_WAIT=10
failures=0
port=$((20000 + RANDOM % 20000))

l36_run() {
  local name=$1 want=$2
  CASE=$name
  : >"$TRACE"
  rm -f "$SNAPSHOTS" "$GITHUB_STEP_SUMMARY"
  set +e
  out=$("$root/loop/percy.sh" "$port" 2>&1)
  code=$?
  set -e
  if [ "$code" -eq "$want" ]; then echo "ok:   L36 $name"; else echo "FAIL: L36 $name (wanted exit $want, got $code: $out)"; failures=$((failures + 1)); fi
}
check() {
  if eval "$2"; then echo "ok:   L36 $1"; else echo "FAIL: L36 $1"; failures=$((failures + 1)); fi
}

PERCY_TOKEN='' l36_run no_token 1
check 'no token never starts the harness or Percy' '[ ! -s "$TRACE" ]'
check 'no token names the missing secret' '[[ $out == *PERCY_TOKEN* ]]'

export PERCY_TOKEN=fake
l36_run snapshots_every_seed 0
check 'serves the harness on the port' 'grep -qx "harness first-run $port" "$TRACE"'
check 'snapshots against the served port' 'grep -q "exec percy snapshot .* --base-url http://localhost:$port\$" "$TRACE"'
seeds=$(sed -n '/^| Seed |/,/^$/p' "$root/.agents/data/harness.md" | sed -nE 's/^\| `([a-z0-9-]+)` \|.*/\1/p' | sort)
check 'harness.md lists the five seeds' '[ "$(printf "%s\n" "$seeds" | wc -l | tr -d " ")" = 5 ]'
check 'one snapshot per harness.md seed' '[ "$(sed -n "s/^- name: //p" "$SNAPSHOTS" | sort)" = "$seeds" ]'
check 'each snapshot loads its seed' '[ "$(grep -c "url: /harness.html?seed=" "$SNAPSHOTS")" = 5 ]'
check 'each snapshot is 1280 wide in Chrome only' '[ "$(grep -c "^  widths: \[1280\]$" "$SNAPSHOTS")" = 5 ] && [ "$(grep -c "^  browsers: \[chrome\]$" "$SNAPSHOTS")" = 5 ]'
check 'build link reaches the job summary' 'grep -q "https://percy.io/abc/web/roundup/builds/42" "$GITHUB_STEP_SUMMARY"'

l36_run harness_dies 1
check 'a dead harness never reaches Percy' '! grep -q "exec percy" "$TRACE"'
l36_run percy_fails 1
l36_run no_link 1

workflow="$root/.github/workflows/visual.yml"
check 'the percy job runs on PRs touching apps/desktop/src' 'grep -q "^  pull_request:" "$workflow" && grep -q "apps/desktop/src/\*\*" "$workflow" && grep -q "^  percy:" "$workflow"'
check 'pushes to main touching apps/desktop/src build the baseline' 'grep -A2 "^  push:" "$workflow" | grep -q "branches: \[main\]" && [ "$(grep -c "apps/desktop/src/\*\*" "$workflow")" = 2 ]'
check 'the percy job gets the PERCY_TOKEN secret and runs loop/percy.sh' 'grep -q "PERCY_TOKEN: \${{ secrets.PERCY_TOKEN }}" "$workflow" && grep -q "run: loop/percy.sh" "$workflow"'
check 'merge-ready recomputes when the visual workflow completes' 'grep -q "workflows: \[check, loop, visual\]" "$root/.github/workflows/merge-ready.yml"'

[ "$failures" -eq 0 ] || { echo "$failures failed"; exit 1; }
