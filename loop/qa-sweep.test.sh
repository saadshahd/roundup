#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch/bin"

cat > "$scratch/bin/agent-browser" <<'BROWSER'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "$QA_SWEEP_CALLS"
case " $* " in
  *" screenshot "*) printf image > "${@: -1}" ;;
  *" set viewport "*) printf '%s %s\n' "${@: -2:1}" "${@: -1}" > "$QA_SWEEP_SIZE" ;;
  *" eval "*)
    if [[ ${QA_SWEEP_BROWSER_FAIL:-0} == 1 ]]; then exit 9; fi
    read -r width height < "$QA_SWEEP_SIZE"
    if [[ ${QA_SWEEP_BLANK:-0} == 1 ]]; then
      printf '{"success":true,"data":{"result":{"blank":true,"viewport":{"width":%s,"height":%s},"overflow":[],"drawer":null}}}\n' "$width" "$height"
    else
      if [[ $(cat "$QA_SWEEP_DRAWER" 2>/dev/null || true) == open && ${QA_SWEEP_NO_DRAWER:-0} != 1 ]]; then drawer='{"left":252,"right":640,"width":388}'; else drawer=null; fi
      if [[ ${QA_SWEEP_OVERFLOW:-0} == 1 ]]; then overflow='["Drawer outside viewport"]'; else overflow='[]'; fi
      centre_width=$((width - 252))
      if [[ ${QA_SWEEP_HIDDEN_CENTRE:-0} == 1 ]]; then centre_width=0; fi
      if [[ ${QA_SWEEP_RESIZED_CENTRE:-0} == 1 && $drawer != null ]]; then centre_width=$((centre_width - 100)); fi
      closed_visible=false
      if [[ ${QA_SWEEP_CLOSED_VISIBLE:-0} == 1 && $(cat "$QA_SWEEP_DRAWER" 2>/dev/null || true) == closed ]]; then closed_visible=true; fi
      printf '{"success":true,"data":{"result":{"blank":false,"viewport":{"width":%s,"height":%s},"boxes":{"rail":{"width":120,"height":%s},"centre":{"width":%s,"height":%s},"shelf":{"width":132,"height":%s}},"overflow":%s,"drawer":%s,"closedDrawerVisible":%s}}}\n' "$width" "$height" "$height" "$centre_width" "$height" "$height" "$overflow" "$drawer" "$closed_visible"
    fi
    ;;
  *" click "*"row-head"*) printf open > "$QA_SWEEP_DRAWER" ;;
  *" click "*"pad-3"*) printf open > "$QA_SWEEP_DRAWER" ;;
  *" click "*".drawer .close"*) printf closed > "$QA_SWEEP_DRAWER" ;;
esac
BROWSER
chmod +x "$scratch/bin/agent-browser"

export PATH="$scratch/bin:$PATH" QA_SWEEP_URL='http://127.0.0.1:5199/harness.html?seed=tree-40' QA_SWEEP_TEST_MODE=1
export QA_SWEEP_CALLS="$scratch/calls"
export QA_SWEEP_SIZE="$scratch/size" QA_SWEEP_DRAWER="$scratch/drawer"
export QA_SWEEP_OUT="$scratch/out"

if [[ -e "$root/loop/qa-sweep.sh" ]]; then
  "$root/loop/qa-sweep.sh"
  record=$(find "$QA_SWEEP_OUT" -name 'sweep-*.json' -print -quit)
  jq -e '.head | length == 40' "$record" >/dev/null
  jq -e '.viewports == ["640×400", "1280×800"] and .scenarios == ["L52", "U5", "U15", "U20"]' "$record" >/dev/null
  [[ $(grep -c ' screenshot ' "$QA_SWEEP_CALLS") == 6 ]]
  grep -q 'click.*pad-3' "$QA_SWEEP_CALLS"
  grep -q 'click.*todos' "$QA_SWEEP_CALLS"
  printf 'l52_two_viewports_and_each_drawer_passed\n'

  rm -rf "$QA_SWEEP_OUT"
  printf closed > "$QA_SWEEP_DRAWER"
  : > "$QA_SWEEP_CALLS"
  if QA_SWEEP_BLANK=1 "$root/loop/qa-sweep.sh"; then
    echo 'blank page returned green' >&2
    exit 1
  fi
  record=$(find "$QA_SWEEP_OUT" -name 'sweep-*.json' -print -quit)
  jq -e '.findings | any(.text | contains("blank"))' "$record" >/dev/null
  printf 'l52_blank_page_fails_with_a_record_passed\n'

  rm -rf "$QA_SWEEP_OUT"
  printf closed > "$QA_SWEEP_DRAWER"
  if QA_SWEEP_BROWSER_FAIL=1 "$root/loop/qa-sweep.sh"; then
    echo 'browser error returned green' >&2
    exit 1
  fi
  record=$(find "$QA_SWEEP_OUT" -name 'sweep-*.json' -print -quit)
  jq -e '.findings | any(.text | contains("geometry failed"))' "$record" >/dev/null
  printf 'l52_browser_error_fails_with_a_record_passed\n'

  rm -rf "$QA_SWEEP_OUT"
  printf closed > "$QA_SWEEP_DRAWER"
  if QA_SWEEP_NO_DRAWER=1 "$root/loop/qa-sweep.sh"; then
    echo 'missing Drawer returned green' >&2
    exit 1
  fi
  record=$(find "$QA_SWEEP_OUT" -name 'sweep-*.json' -print -quit)
  jq -e '.findings | any(.text | contains("Drawer state missing"))' "$record" >/dev/null
  printf 'l52_missing_drawer_fails_with_a_record_passed\n'

  for defect in HIDDEN_CENTRE RESIZED_CENTRE CLOSED_VISIBLE; do
    rm -rf "$QA_SWEEP_OUT"
    printf closed > "$QA_SWEEP_DRAWER"
    if env "QA_SWEEP_$defect=1" "$root/loop/qa-sweep.sh"; then
      echo "$defect returned green" >&2
      exit 1
    fi
    record=$(find "$QA_SWEEP_OUT" -name 'sweep-*.json' -print -quit)
    jq -e '.findings | length > 0' "$record" >/dev/null
    printf 'l52_%s_fails_with_a_record_passed\n' "$defect"
  done

  rm -rf "$QA_SWEEP_OUT"
  printf closed > "$QA_SWEEP_DRAWER"
  if QA_SWEEP_OVERFLOW=1 "$root/loop/qa-sweep.sh"; then
    echo 'overflow returned green' >&2
    exit 1
  fi
  record=$(find "$QA_SWEEP_OUT" -name 'sweep-*.json' -print -quit)
  jq -e '.findings | length == 6 and all(.scenario == null and .test == null)' "$record" >/dev/null
  printf 'l52_layout_findings_are_durable_and_unscoped_passed\n'

  if env -u QA_SWEEP_TEST_MODE "$root/loop/qa-sweep.sh" > "$scratch/override.log" 2>&1; then
    echo 'alternate URL returned green outside test mode' >&2
    exit 1
  fi
  grep -Eq 'checkout is not origin/main|alternate URL is for test mode only' "$scratch/override.log"
  printf 'l52_test_override_cannot_label_a_production_sweep_passed\n'
else
  echo 'l52_sweep_missing' >&2
  exit 1
fi
