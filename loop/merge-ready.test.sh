#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
mkdir -p "$dir/bin" "$dir/loop"
# Execute the trusted workflow itself so a regression cannot hide in a copied guard.
sed -n '/^        run: |$/,$ { /^          /s/^          //p; }' \
  "$root/.github/workflows/merge-ready.yml" >"$dir/compute.sh"
test -s "$dir/compute.sh"
cat >"$dir/bin/gh" <<'GH'
#!/usr/bin/env bash
set -euo pipefail
case "$*" in
  'pr view 281 --json headRefOid --jq .headRefOid') echo head ;;
  'pr diff 281 --name-only') echo "$PR_PATH" ;;
  'pr view 281 --json autoMergeRequest --jq .autoMergeRequest != null')
    echo read >>"$TRACE"
    if [ "$CASE" = read_error ]; then echo 'GitHub read failed' >&2; exit 1; fi
    printf '{"autoMergeRequest":%s}\n' "$AUTO_MERGE" | jq -r "$7"
    ;;
  'pr merge 281 --disable-auto')
    echo disarm >>"$TRACE"
    if [ "$AUTO_MERGE" = null ] || [ "$CASE" = disarm_error ]; then exit 1; fi
    ;;
  'api repos/test/repo/statuses/head -f state='*)
    printf '%s\n' "$4" >>"$TRACE"
    ;;
  *) echo "unexpected gh call: $*" >&2; exit 1 ;;
esac
GH
cat >"$dir/loop/rules.sh" <<'RULES'
#!/usr/bin/env bash
set -euo pipefail
if [ "$*" = touches ]; then exec bash "$ROOT/loop/rules.sh" touches; fi
[ "$*" = 'merge-ready 281' ]
echo gates >>"$TRACE"
echo 'ready head'
RULES
chmod +x "$dir/bin/gh" "$dir/loop/rules.sh"
export PATH="$dir/bin:$PATH" ROOT="$root" REPO=test/repo PR_FROM_PR=281 PR_FROM_COMMENT='' PR_FROM_DISPATCH='' PR_FROM_RUN=''
export TRACE="$dir/trace" AUTO_MERGE PR_PATH CASE
cd "$dir"
failures=0
l78_expect() {
  local name=$1 want=$2 expected=$3 message=${4:-} code=0
  : >"$TRACE"
  bash -eo pipefail compute.sh >output 2>&1 || code=$?
  if [ "$code" -eq "$want" ] && [ "$(cat "$TRACE")" = "$expected" ] &&
      { [ -z "$message" ] || grep -Fq "$message" output; }; then
    echo "ok: l78_$name"
  else
    echo "FAIL: l78_$name (exit $code, wanted $want)"
    cat "$TRACE" output
    failures=$((failures + 1))
  fi
}
PR_PATH=.github/workflows/check.yml
CASE=null AUTO_MERGE=null
l78_expect null_skips_disarm 0 $'read\ngates\nstate=success'
CASE=enabled AUTO_MERGE='{"enabledAt":"2026-10-04T00:00:00Z"}'
l78_expect enabled_disarms_before_gates 0 $'read\ndisarm\ngates\nstate=success'
CASE=read_error
l78_expect read_error_fails_closed 1 $'read\nstate=failure' 'merge-ready: cannot read auto-merge on a loop PR'
CASE=disarm_error
l78_expect disarm_error_fails_closed 1 $'read\ndisarm\nstate=failure' 'merge-ready: cannot disarm auto-merge on a loop PR'
for PR_PATH in loop/rules.sh .agents/builder.md AGENTS.md; do
  CASE=enabled AUTO_MERGE='{"enabledAt":"2026-10-04T00:00:00Z"}'
  l78_expect "enabled_disarms_${PR_PATH//[^A-Za-z]/_}" 0 $'read\ndisarm\ngates\nstate=success'
done
CASE=outside PR_PATH=$'docs/perf.md\ncrates/rupd/src/main.rs'
l78_expect other_paths_leave_auto_merge_alone 0 $'gates\nstate=success'
[ "$failures" -eq 0 ]
