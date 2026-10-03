#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
head=$(git rev-parse HEAD)
if [[ ${QA_SWEEP_TEST_MODE:-0} != 1 && $head != $(git rev-parse origin/main) ]]; then
  printf 'L52: checkout is not origin/main; sweep the latest main\n' >&2
  exit 1
fi
if [[ ${QA_SWEEP_TEST_MODE:-0} != 1 && -n ${QA_SWEEP_URL:-} ]]; then
  printf 'L52: alternate URL is for test mode only\n' >&2
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
    kill -TERM -- "-$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
    for _ in {1..10}; do
      if ! curl -fsS --max-time 1 "$url" >/dev/null 2>&1; then break; fi
      sleep 0.1
    done
    if curl -fsS --max-time 1 "$url" >/dev/null 2>&1; then
      record_finding "harness listener survived cleanup on port $port"
      code=1
    fi
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
  python3 -c 'import os, sys; os.setsid(); os.execvp("just", ["just", "harness", "tree-40", sys.argv[1]])' "$port" > "$work/harness.log" 2>&1 &
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
  const unmodifiedPaint = element => {
    for (let node = element; node; node = node.parentElement) {
      const style = getComputedStyle(node);
      if (style.clipPath !== "none" || (style.maskImage && style.maskImage !== "none") || (style.filter && style.filter !== "none")) return false;
    }
    return true;
  };
  const box = element => { const r = element.getBoundingClientRect(); return {left:r.left,right:r.right,width:r.width,top:r.top,bottom:r.bottom,height:r.height,visible:element.checkVisibility({checkOpacity:true,checkVisibilityCSS:true}) && unmodifiedPaint(element)}; };
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
  const closedDrawerVisible = !!(drawer && !open && (() => { const r = box(drawer); return r.width > 0 && r.height > 0 && r.left < innerWidth - 1 && r.right > 1 && r.top < innerHeight - 1 && r.bottom > 1; })());
  if (closedDrawerVisible) overflow.push("closed Drawer remains on-screen");
  if (open) {
    const r = box(drawer);
    if (r.width <= 0 || r.height <= 0 || r.right > innerWidth + 1 || r.left < -1 || r.bottom > innerHeight + 1 || r.top < -1) overflow.push("Drawer outside viewport");
  }
  return {
    blank: !rail || !centre || !shelf || !document.body.innerText.trim(),
    viewport:{width:innerWidth,height:innerHeight},
    boxes:{rail:rail && box(rail),centre:centre && box(centre),shelf:shelf && box(shelf)},
    overflow,
    drawer:open ? box(drawer) : null,
    closedDrawerVisible
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
  if ! jq -e --argjson width "$width" --argjson height "$height" '[.boxes.rail,.boxes.centre,.boxes.shelf] | all(.visible == true and .width > 0 and .height > 0 and .left < $width and .right > 0 and .top < $height and .bottom > 0)' <<< "$metrics" >/dev/null; then
    record_finding "$label at ${width}×${height}: Rail, centre or Shelf is hidden or outside the viewport"
  fi
  actual=$(jq -r '.drawer != null' <<< "$metrics")
  if [[ $actual != "$open" ]]; then fail "Drawer state missing for $label at ${width}×${height}"; fi
  centre_box=$(jq -c '.boxes.centre' <<< "$metrics")
  if [[ $open == false ]]; then
    baseline_centre_box=$centre_box
  elif ! centre_unchanged "$metrics"; then
    record_finding "$label at ${width}×${height}: Drawer moved or resized centre"
  fi
  while IFS= read -r defect; do
    [[ -z $defect ]] || record_finding "$label at ${width}×${height}: $defect"
  done < <(jq -r '.overflow[]' <<< "$metrics")
}

centre_unchanged() {
  jq -e --argjson base "$baseline_centre_box" 'def near($a;$b): (($a-$b)|abs) <= 1; .boxes.centre as $now | near($now.left;$base.left) and near($now.top;$base.top) and near($now.width;$base.width) and near($now.height;$base.height)' <<< "$1" >/dev/null
}

verify_closed() {
  label=$1 width=$2 height=$3
  metrics=$(browser --json eval "$geometry" | jq -e '.data.result') || fail "agent-browser geometry failed after closing $label at ${width}×${height}"
  if [[ $(jq -r '.closedDrawerVisible' <<< "$metrics") != false ]]; then
    record_finding "$label at ${width}×${height}: closed Drawer remains on-screen"
  fi
  if ! centre_unchanged "$metrics"; then
    record_finding "$label at ${width}×${height}: centre did not return after Drawer closed"
  fi
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
  verify_closed todo "$width" "$height"

  browser scrollintoview '[aria-label="pads"] button[title="pad-3"]' >/dev/null || fail "Pad row could not be reached at ${width}×${height}"
  browser click '[aria-label="pads"] button[title="pad-3"]' >/dev/null || fail "Pad Drawer could not open at ${width}×${height}"
  measure pad "$width" "$height" true
  browser click '.drawer .close' >/dev/null || fail "Pad Drawer could not close at ${width}×${height}"
  browser wait --fn 'document.querySelector("[aria-label=drawer]").hasAttribute("inert")' >/dev/null || fail "Pad Drawer remained open at ${width}×${height}"
  verify_closed pad "$width" "$height"
  jq -nc --arg size "${width}×${height}" '$size' >> "$work/viewports"
done

for id in L52 U5 U15 U20; do jq -nc --arg id "$id" '$id' >> "$work/scenarios"; done
printf 'l52_two_viewports_and_each_drawer_passed: %s findings\n' "$finding_count"
if (( finding_count )); then exit 1; fi
