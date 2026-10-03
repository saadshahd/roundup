#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
head=${QA_SWEEP_HEAD:-$(git rev-parse origin/main)}
if [[ -z ${QA_SWEEP_HEAD:-} && $(git rev-parse HEAD) != "$head" ]]; then
  printf 'L52: checkout is not origin/main; sweep the latest main\n' >&2
  exit 1
fi
started=$(date -u +%Y-%m-%dT%H:%M:%SZ)
out=${QA_SWEEP_OUT:-loop/out/qa}
port=${QA_SWEEP_PORT:-5199}
url=${QA_SWEEP_URL:-http://localhost:$port/harness.html?seed=tree-40}
browser_name="l52-$$"
mkdir -p "$out"
out=$(cd "$out" && pwd)
work=$(mktemp -d)
shots="$out/sweep-$(date -u +%Y%m%dT%H%M%SZ)-$$"
mkdir -p "$shots"
: > "$work/findings"
: > "$work/viewports"
: > "$work/scenarios"
server_pid=
opened=0
finding_count=0

record_finding() {
  finding_count=$((finding_count + 1))
  jq -nc \
    --arg id "l52-$(date -u +%Y%m%dT%H%M%S)-$$-$finding_count" \
    --arg text "$1" \
    --arg at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    '{id:$id,text:$text,at:$at,scenario:null,test:null}' >> "$work/findings"
  printf 'L52: %s\n' "$1" >&2
}

fail() {
  record_finding "$1"
  exit 1
}

finish() {
  code=$?
  trap - EXIT
  if (( opened )); then agent-browser --session "$browser_name" close >/dev/null 2>&1 || true; fi
  if [[ -n $server_pid ]]; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  record="$out/sweep-$(date -u +%Y%m%dT%H%M%SZ)-$$.json"
  jq -n \
    --arg head "$head" --arg started "$started" \
    --slurpfile viewports "$work/viewports" \
    --slurpfile scenarios "$work/scenarios" \
    --slurpfile findings "$work/findings" \
    '{head:$head,started:$started,viewports:$viewports,scenarios:$scenarios,findings:$findings}' > "$record"
  rm -rf "$work"
  printf 'L52 record: %s\n' "$record"
  exit "$code"
}
trap finish EXIT

if [[ -z ${QA_SWEEP_URL:-} ]]; then
  if curl -fsS --max-time 2 "$url" >/dev/null 2>&1; then
    fail "port $port already answers before just harness starts"
  fi
  just harness tree-40 "$port" > "$work/harness.log" 2>&1 &
  server_pid=$!
  ready=0
  for _ in {1..40}; do
    if curl -fsS --max-time 1 "$url" >/dev/null 2>&1; then ready=1; break; fi
    if ! kill -0 "$server_pid" 2>/dev/null; then break; fi
    sleep 0.25
  done
  if (( !ready )); then
    cat "$work/harness.log" >&2
    fail "just harness did not answer on port $port"
  fi
fi

browser() { agent-browser --session "$browser_name" "$@"; }
browser open "$url" >/dev/null || fail "agent-browser could not open $url"
opened=1
browser set media light reduced-motion >/dev/null || fail "agent-browser could not set reduced motion"
browser wait --fn 'document.querySelector("[aria-label=rail]") && document.querySelector("[aria-label=todos]") && document.querySelector("[aria-label=pads]")' >/dev/null || fail "harness did not render Rail and Shelf"

geometry='(() => {
  const box = element => { const r = element.getBoundingClientRect(); return {left:r.left,right:r.right,width:r.width,top:r.top,bottom:r.bottom,height:r.height}; };
  const rail = document.querySelector("[aria-label=rail]");
  const centre = document.querySelector("[aria-label=centre]");
  const shelf = document.querySelector("[aria-label=shelf]");
  const drawer = document.querySelector("[aria-label=drawer]");
  const overflow = [];
  if (document.documentElement.scrollWidth > innerWidth + 1) overflow.push("document wider than viewport");
  for (const [name, element] of [["Rail",rail],["centre",centre],["Shelf",shelf]]) {
    if (element && box(element).width > innerWidth + 1) overflow.push(name + " wider than viewport");
  }
  const open = drawer && !drawer.hasAttribute("inert");
  if (open) {
    const r = box(drawer);
    if (r.right > innerWidth + 1 || r.left < -1 || r.width > innerWidth + 1) overflow.push("Drawer outside viewport");
  }
  return {
    blank: !rail || !centre || !shelf || !document.body.innerText.trim(),
    viewport:{width:innerWidth,height:innerHeight},
    boxes:{rail:rail && box(rail),centre:centre && box(centre),shelf:shelf && box(shelf)},
    overflow,
    drawer:open ? box(drawer) : null
  };
})()'

measure() {
  label=$1 width=$2 height=$3 open=$4
  screenshot="$shots/$label-${width}x$height.png"
  browser screenshot "$screenshot" >/dev/null || fail "agent-browser screenshot failed for $label at ${width}×${height}"
  [[ -s $screenshot ]] || fail "agent-browser wrote no screenshot for $label at ${width}×${height}"
  metrics=$(browser --json eval "$geometry" | jq -e '.data.result') || fail "agent-browser geometry failed for $label at ${width}×${height}"
  jq -e '.overflow | type == "array"' <<< "$metrics" >/dev/null || fail "agent-browser returned no overflow boxes for $label"
  jq -e --argjson width "$width" --argjson height "$height" '.viewport == {width:$width,height:$height}' <<< "$metrics" >/dev/null || fail "viewport was not ${width}×${height} for $label"
  if [[ $(jq -r '.blank' <<< "$metrics") == true ]]; then fail "blank page at ${width}×${height}"; fi
  actual=$(jq -r '.drawer != null' <<< "$metrics")
  if [[ $actual != "$open" ]]; then fail "Drawer state missing for $label at ${width}×${height}"; fi
  while IFS= read -r defect; do
    [[ -z $defect ]] || record_finding "$label at ${width}×${height}: $defect"
  done < <(jq -r '.overflow[]' <<< "$metrics")
}

for size in 640x400 1280x800; do
  width=${size%x*}
  height=${size#*x}
  browser set viewport "$width" "$height" >/dev/null || fail "agent-browser could not set ${width}×${height}"
  measure workspace "$width" "$height" false

  browser click '[aria-label="todos"] [data-id="1"] .row-head button' >/dev/null || fail "Todo Drawer could not open at ${width}×${height}"
  measure todo "$width" "$height" true
  browser click '.drawer .close' >/dev/null || fail "Todo Drawer could not close at ${width}×${height}"
  browser wait --fn 'document.querySelector("[aria-label=drawer]").hasAttribute("inert")' >/dev/null || fail "Todo Drawer remained open at ${width}×${height}"

  browser scrollintoview '[aria-label="pads"] button[title="pad-3"]' >/dev/null || fail "Pad row could not be reached at ${width}×${height}"
  browser click '[aria-label="pads"] button[title="pad-3"]' >/dev/null || fail "Pad Drawer could not open at ${width}×${height}"
  measure pad "$width" "$height" true
  browser click '.drawer .close' >/dev/null || fail "Pad Drawer could not close at ${width}×${height}"
  browser wait --fn 'document.querySelector("[aria-label=drawer]").hasAttribute("inert")' >/dev/null || fail "Pad Drawer remained open at ${width}×${height}"
  jq -nc --arg size "${width}×${height}" '$size' >> "$work/viewports"
done

for id in L52 U5 U15 U20; do jq -nc --arg id "$id" '$id' >> "$work/scenarios"; done
printf 'l52_two_viewports_and_each_drawer_passed: %s findings\n' "$finding_count"
