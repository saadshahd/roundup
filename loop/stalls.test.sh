#!/usr/bin/env bash
# Tests for loop/stalls.sh (L28), each in a throwaway git repo with a fake gh and a stubbed loop/rules.sh. Usage: loop/stalls.test.sh
set -euo pipefail

script="$(cd "$(dirname "$0")" && pwd)/stalls.sh"
failures=0
head_sha=1111111111111111111111111111111111111111
other_sha=2222222222222222222222222222222222222222

ago() { date -u -d "-$1 minutes" +%Y-%m-%dT%H:%M:%SZ; }
ok() { echo "ok:   $1"; }
fail() { echo "FAIL: $1"; failures=$((failures + 1)); }
expect() { if "${@:2}"; then ok "$1"; else fail "$1"; fi; }

# A repo whose stalls.sh landed three hours ago; fixtures live in $fix, read by the fake gh and the stub rules.sh.
new_repo() {
  dir=$(mktemp -d)
  fix=$dir/fix
  mkdir -p "$fix" "$dir/bin" "$dir/repo/loop"
  cd "$dir/repo"
  git init -q -b main
  git config user.email t@t
  git config user.name t
  cp "$script" loop/stalls.sh
  git add -A
  GIT_COMMITTER_DATE=$(ago 180) git commit -qm "stalls landed"
  cat >"$dir/bin/gh" <<'GH'
#!/usr/bin/env bash
route=$2
[ ! -e "$FIX/hang" ] || sleep 5
case $route in
  *check-runs*) f=checks ;;
  *pulls?state=open*) f=open ;;
  *pulls?state=closed*) f=closed ;;
  */pulls/*/commits*) f=commits-$(sed 's|.*/pulls/\([0-9]*\)/.*|\1|' <<<"$route") ;;
  *) exit 1 ;;
esac
[ ! -e "$FIX/fail-$f" ] || { echo "boom" >&2; exit 1; }
cat "$FIX/$f.json" 2>/dev/null || echo '[[]]'
GH
  cat >loop/rules.sh <<'RULES'
#!/usr/bin/env bash
echo "$1 $2" >>"$FIX/calls"
f=$FIX/$1-$2
cat "$f.out" 2>/dev/null || true
cat "$f.err" >&2 2>/dev/null || true
exit "$(cat "$f.code" 2>/dev/null || echo 0)"
RULES
  chmod +x "$dir/bin/gh" loop/rules.sh
  export FIX=$fix PATH="$dir/bin:$PATH"
  checks "$(ago 5)" success
  echo '[[]]' >"$fix/open.json"
  echo '[[]]' >"$fix/closed.json"
}

# checks <completed_at> <conclusion>: the newest completed run of `check` on main.
checks() {
  printf '[{"check_runs":[{"name":"check","status":"completed","conclusion":"%s","completed_at":"%s"},{"name":"rules","status":"in_progress","conclusion":null,"completed_at":null},{"name":"other","status":"completed","conclusion":"failure","completed_at":"%s"}]}]\n' \
    "$2" "$1" "$(ago 1)" >"$fix/checks.json"
}
# open_pr <n>: one open PR with head $head_sha, authored by builder-1.
open_pr() {
  printf '[[{"number":%s,"head":{"sha":"%s"}}]]\n' "$1" "$head_sha" >"$fix/open.json"
  printf '[[{"commit":{"message":"x\\n\\nAuthor-Agent: builder-1"}},{"commit":{"message":"y\\n\\nAuthor-Agent: builder-2\\nAuthor-Agent: builder-1"}}]]\n' >"$fix/commits-$1.json"
}
closed_pr() { printf '[[{"number":%s,"merged_at":"%s","head":{"sha":"%s"}}]]\n' "$1" "$2" "$head_sha" >"$fix/closed.json"; }
stub() { # stub <cmd> <n> <code> <stdout> <stderr>
  printf '%s' "$4" >"$fix/$1-$2.out"
  printf '%s' "$5" >"$fix/$1-$2.err"
  echo "$3" >"$fix/$1-$2.code"
}

stalls() { loop/stalls.sh "$@"; }
check_code() { code=0; err=$(stalls check 2>&1 >/dev/null) || code=$?; }
holds() { # holds <file> <owner>
  [ -f "loop/out/stalls/$1" ] && grep -qx "owner=$2" "loop/out/stalls/$1" && [ "$code" -eq 1 ] && [[ $err == *"loop/out/stalls/$1"* ]]
}
clear() { [ ! -e "loop/out/stalls/$1" ] && [ "$code" -eq 0 ] && [ -z "$err" ]; }
verdict_file() { mkdir -p loop/out/verdicts; echo "head $head_sha" >loop/out/verdicts/r.md; touch -d "-$1 minutes" loop/out/verdicts/r.md; }

# a: red main
new_repo; checks "$(ago 5)" failure; check_code
expect "L28 a holds when the newest completed check run on main failed, owner Triage" holds a-main Triage
checks "$(ago 5)" success; check_code
expect "L28 a clears (file deleted) when the newest completed run succeeded" clear a-main
checks "$(ago 10)" failure; printf '[{"check_runs":[{"name":"check","status":"completed","conclusion":"failure","completed_at":"%s"},{"name":"rules","status":"completed","conclusion":"success","completed_at":"%s"}]}]\n' "$(ago 10)" "$(ago 2)" >"$fix/checks.json"; check_code
expect "L28 a: an older failure under a newer success does not hold" clear a-main

# b: an unmerged approval
new_repo; open_pr 7; stub merge-ready 7 0 "ready $head_sha"$'\n' ''; stub verdicts 7 0 "approve $head_sha $(ago 25)"$'\n' ''; check_code
expect "L28 b holds when ready and the newest approve is 25 minutes old, owner Merger" holds b-7 Merger
stub verdicts 7 0 "approve $head_sha $(ago 10)"$'\n' ''; check_code
expect "L28 b clears when the approve is under 20 minutes old" clear b-7
stub verdicts 7 0 "approve $head_sha $(ago 25)"$'\n' ''; stub merge-ready 7 1 '' 'merge-ready: no'; check_code
expect "L28 b clears when merge-ready fails" clear b-7
stub merge-ready 7 0 "ready $head_sha"$'\n' ''; stub verdicts 7 0 "approve $head_sha $(ago 90)"$'\n'"reject $head_sha $(ago 80)"$'\n'"approve $head_sha $(ago 5)"$'\n' ''; check_code
expect "L28 b reads the newest approve, not an older one" clear b-7

# c: a third reject
new_repo; open_pr 7; stub rounds 7 1 '' 'rounds: round cap: 3 rejects, an Architect must pick split, amend or retire'; check_code
expect "L28 c holds on the round cap, owner is an Architect except the author ids" holds c-7 'Architect except builder-1,builder-2'
stub rounds 7 1 '' 'rounds: retired'; check_code
expect "L28 c does not hold on a retire" clear c-7
stub rounds 7 0 $'2\n' ''; check_code
expect "L28 c does not hold below the cap" clear c-7

# d: a lost verdict
new_repo; open_pr 7; stub verdicts 7 0 "approve $other_sha $(ago 90)"$'\n' ''; verdict_file 10; check_code
expect "L28 d holds when a 10-minute-old verdict file names the head and no verdict does, owner Driver" holds d-7 Driver
verdict_file 1; check_code
expect "L28 d clears when the verdict file is under 5 minutes old" clear d-7
verdict_file 10; stub verdicts 7 0 "reject $head_sha $(ago 3)"$'\n' ''; check_code
expect "L28 d clears when a verdict names the head" clear d-7
rm -r loop/out/verdicts; stub verdicts 7 0 '' ''; check_code
expect "L28 d clears when no verdict file holds the head" clear d-7

# e: an overdue post-merge review
new_repo; open_pr 7 && echo '[[]]' >"$fix/open.json"; closed_pr 9 "$(ago 90)"; stub class 9 0 $'post\n' ''; stub verdicts 9 0 '' ''; check_code
expect "L28 e holds when a post PR merged 90 minutes ago has no verdict on its head, owner Driver" holds e-9 Driver
closed_pr 9 "$(ago 30)"; check_code
expect "L28 e clears before 60 minutes" clear e-9
closed_pr 9 "$(ago 90)"; stub class 9 0 $'block\n' ''; check_code
expect "L28 e clears for a block-lane PR" clear e-9
stub class 9 0 $'post\n' ''; stub verdicts 9 0 "approve $head_sha $(ago 70)"$'\n' ''; check_code
expect "L28 e clears when a verdict names the merged head" clear e-9
stub verdicts 9 0 '' ''; closed_pr 9 "$(ago 240)"; check_code
expect "L28 e ignores a PR merged before stalls.sh landed" clear e-9

# file content, since kept, deletion, exit codes
new_repo; checks "$(ago 5)" failure; check_code
f=loop/out/stalls/a-main
since=$(sed -n 's/^since=//p' "$f"); deadline=$(sed -n 's/^deadline=//p' "$f")
expect "L28 since is UTC now and deadline is since plus 30 minutes" test "$deadline" = "$(date -u -d "$since +30 minutes" +%Y-%m-%dT%H:%M:%SZ)" -a "$since" \> "$(ago 1)"
sed -i 's/^since=.*/since=2020-01-01T00:00:00Z/' "$f"; check_code
expect "L28 since is kept while the file exists" grep -qx 'since=2020-01-01T00:00:00Z' "$f"
expect "L28 a holding Stall exits 1 and names its file on stderr" holds a-main Triage
mkdir -p loop/out/stalls; echo stale >loop/out/stalls/c-99; checks "$(ago 5)" success; check_code
expect "L28 a file whose Stall stopped holding is deleted, and the run is silent with exit 0" test ! -e loop/out/stalls/c-99 -a ! -e "$f" -a "$code" -eq 0 -a -z "$err"
code=0; loop/stalls.sh nope >/dev/null 2>&1 || code=$?
expect "L28 usage: an unknown command exits 2" test "$code" -eq 2

# check calls the verdict parser in loop/rules.sh
new_repo; open_pr 7; stub verdicts 7 0 '' ''; verdict_file 10; check_code
# shellcheck disable=SC2016 # the inner shell expands $FIX
expect "L28 check asks loop/rules.sh verdicts, rounds and merge-ready, and keeps no parser of its own" bash -c 'grep -qx "verdicts 7" "$FIX/calls" && grep -qx "rounds 7" "$FIX/calls" && grep -qx "merge-ready 7" "$FIX/calls" && ! grep -q "VERDICT" loop/stalls.sh'

# report: once per file, again after a delete and rewrite
new_repo; checks "$(ago 5)" failure; check_code
first=$(stalls report)
second=$(stalls report)
expect "L28 report prints kind, subject, owner and due once" test "$first" = "a main owner Triage due $(sed -n 's/^deadline=//p' loop/out/stalls/a-main)" -a -z "$second"
checks "$(ago 5)" success; check_code; checks "$(ago 3)" failure; check_code
expect "L28 report prints again for a file deleted and written again" test "$(stalls report | cut -d' ' -f1,2)" = "a main"

# gh failure and timeout change nothing
new_repo; checks "$(ago 5)" failure; check_code
mkdir -p loop/out/stalls; echo stale >loop/out/stalls/b-5; touch "$fix/fail-closed"; code=0; err=$(stalls check 2>&1 >/dev/null) || code=$?
expect "L28 a gh failure exits 4 naming gh and changes no file" test "$code" -eq 4 -a -f loop/out/stalls/b-5 -a "$(find loop/out/stalls -type f | wc -l)" -eq 2 -a -n "$(grep -F gh <<<"$err")"
rm "$fix/fail-closed"; touch "$fix/hang"; code=0; err=$(BOXD_GH_TIMEOUT=1 stalls check 2>&1 >/dev/null) || code=$?
expect "L28 a gh timeout exits 4 and changes no file" test "$code" -eq 4 -a -f loop/out/stalls/b-5 -a -n "$(grep -F gh <<<"$err")"

[ "$failures" -eq 0 ]
