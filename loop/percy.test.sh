#!/usr/bin/env bash
# Tests for loop/percy.sh (L36, L24) with fake `just`, `pnpm`, `curl` and `sleep`. Usage: loop/percy.test.sh
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
[ "$CASE" != harness_silent ] || exec sleep 30
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
  out=$("$root/loop/percy.sh" snapshot "$port" 2>&1)
  code=$?
  set -e
  if [ "$code" -eq "$want" ]; then echo "ok:   L36 $name"; else echo "FAIL: L36 $name (wanted exit $want, got $code: $out)"; failures=$((failures + 1)); fi
}
# The scenario id a check proves.
id=L36
holds_eval() {
  if eval "$2"; then echo "ok:   $id $1"; else echo "FAIL: $id $1"; failures=$((failures + 1)); fi
}

PERCY_TOKEN='' l36_run no_token 1
holds_eval 'no token never starts the harness or Percy' '[ ! -s "$TRACE" ]'
holds_eval 'no token names the missing secret' '[[ $out == *PERCY_TOKEN* ]]'

export PERCY_TOKEN=fake
l36_run snapshots_every_seed 0
holds_eval 'serves the harness on the port' 'grep -qx "harness first-run $port" "$TRACE"'
holds_eval 'snapshots against the served port' 'grep -q "exec percy snapshot .* --base-url http://localhost:$port\$" "$TRACE"'
seeds=$(sed -n '/^| Seed |/,/^$/p' "$root/.agents/data/harness.md" | sed -nE 's/^\| `([a-z0-9-]+)` \|.*/\1/p' | sort)
holds_eval 'harness.md lists the five seeds' '[ "$(printf "%s\n" "$seeds" | wc -l | tr -d " ")" = 5 ]'
holds_eval 'one snapshot per harness.md seed' '[ "$(sed -n "s/^- name: //p" "$SNAPSHOTS" | sort)" = "$seeds" ]'
holds_eval 'each snapshot loads its seed' '[ "$(grep -c "url: /harness.html?seed=" "$SNAPSHOTS")" = 5 ]'
holds_eval 'each snapshot is 1280 wide in Chrome only' '[ "$(grep -c "^  widths: \[1280\]$" "$SNAPSHOTS")" = 5 ] && [ "$(grep -c "^  browsers: \[chrome\]$" "$SNAPSHOTS")" = 5 ]'
holds_eval 'build link reaches the job summary' 'grep -q "https://percy.io/abc/web/roundup/builds/42" "$GITHUB_STEP_SUMMARY"'

l36_run harness_dies 1
holds_eval 'a dead harness never reaches Percy' '! grep -q "exec percy" "$TRACE"'
HARNESS_WAIT=2 l36_run harness_silent 1
holds_eval 'a silent harness never reaches Percy' '! grep -q "exec percy" "$TRACE"'
l36_run percy_fails 1
l36_run no_link 1

workflow="$root/.github/workflows/visual.yml"
holds_eval 'the percy job runs on PRs touching apps/desktop/src' 'grep -q "^  pull_request:" "$workflow" && grep -q "apps/desktop/src/\*\*" "$workflow" && grep -q "^  percy:" "$workflow"'
holds_eval 'pushes to main touching apps/desktop/src build the baseline' 'grep -A2 "^  push:" "$workflow" | grep -q "branches: \[main\]" && [ "$(grep -c "apps/desktop/src/\*\*" "$workflow")" = 2 ]'
holds_eval 'the percy job gets the PERCY_TOKEN secret and runs loop/percy.sh' 'grep -q "PERCY_TOKEN: \${{ secrets.PERCY_TOKEN }}" "$workflow" && grep -q "run: loop/percy.sh snapshot" "$workflow"'
holds_eval 'merge-ready recomputes when the visual workflow completes' 'grep -q "workflows: \[check, loop, visual\]" "$root/.github/workflows/merge-ready.yml"'

holds_eval 'the percy job pins the build to the PR head, so the Reviewer finds it by SHA' 'grep -q "PERCY_COMMIT: \${{ github.event.pull_request.head.sha || github.sha }}" "$workflow"'

id=L24
# L24 build: the Percy build of a head, found by its SHA. A fake curl answers from $ANSWERS, one line per call.
mkdir -p "$dir/api"
cat >"$dir/api/curl" <<'CURL'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$TRACE"
[ "$CASE" != api_fails ] || { echo '{"errors":[{"status":"401"}]}'; exit 22; }
n=$(grep -c . "$TRACE")
sed -n "${n}p" "$ANSWERS"
CURL
printf '#!/usr/bin/env bash\n' >"$dir/api/sleep"
chmod +x "$dir/api/curl" "$dir/api/sleep"
export ANSWERS="$dir/answers"
sha=$(printf 'a%.0s' {1..40})
l24_run() {
  local name=$1 want=$2
  shift 2
  CASE=$name
  : >"$TRACE"
  set +e
  out=$(PATH="$dir/api:$PATH" "$@" 2>&1)
  code=$?
  set -e
  if [ "$code" -eq "$want" ]; then echo "ok:   L24 $name"; else echo "FAIL: L24 $name (wanted exit $want, got $code: $out)"; failures=$((failures + 1)); fi
}
printf '{"data":[]}\n{"data":[{"id":"41823"}]}\n' >"$ANSWERS"
l24_run build_appears 0 "$root/loop/percy.sh" build "$sha"
holds_eval 'build prints the id once visual has started it' '[ "$out" = 41823 ]'
holds_eval 'build asks Percy for the head SHA with the token' 'grep -qF "filter[sha]=$sha" "$TRACE" && grep -q "Authorization: Token token=fake" "$TRACE"'
printf '{"data":[]}\n' >"$ANSWERS"
PERCY_WAIT=0 l24_run no_build 1 "$root/loop/percy.sh" build "$sha"
holds_eval 'no build names the SHA' '[[ $out == *"no Percy build for $sha"* ]]'
l24_run api_fails 4 "$root/loop/percy.sh" build "$sha"
PERCY_TOKEN='' l24_run build_without_token 2 "$root/loop/percy.sh" build "$sha"
l24_run build_of_no_sha 2 "$root/loop/percy.sh" build main

[ "$failures" -eq 0 ] || { echo "$failures failed"; exit 1; }
