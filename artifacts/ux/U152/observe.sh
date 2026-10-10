#!/bin/bash
# usage: observe.sh W theme   (needs `just harness tree-40 5199`; fake Daemon, so the restart of rupd in U155 is not covered)
W=$1; T=$2; S=obs-$W-$T
ab(){ agent-browser --session $S "$@" 2>&1 | tail -1; }
ev(){ agent-browser --session $S eval "$1" 2>&1 | tail -1; }
load(){ ab open "http://localhost:5199/harness.html?seed=tree-40" >/dev/null; sleep 2; }
ids='JSON.stringify([...document.querySelectorAll("[data-shelf-row]")].map(r=>r.dataset.todo).slice(0,3))'
tops='JSON.stringify([...document.querySelectorAll("[data-shelf-row]")].map(r=>Math.round(r.getBoundingClientRect().top)))'
body=0; chk(){ [ "$(ev 'document.activeElement.tagName')" = '"BODY"' ] && body=1; }
ab set viewport $W 800 >/dev/null; ab set media $T >/dev/null; load
echo "== $W $T dark=$(ev 'matchMedia("(prefers-color-scheme: dark)").matches')"
# U153
ab hover body >/dev/null
echo "U153 row/list/shelf: $(ev 'const v=k=>getComputedStyle(document.documentElement).getPropertyValue(k).trim(),r=document.querySelector("[data-todo=\"4\"]"),c=getComputedStyle(r),rs=[...document.querySelectorAll("[data-shelf-row]")],l=r.parentElement,b=r.querySelector(".todo-row-button"),w=r.querySelector("[data-todo-waits]"),h=r.querySelector("[data-todo-home]");const t=rs.map(x=>x.getBoundingClientRect());JSON.stringify({bg:c.backgroundColor,ground:v("--ground"),radius:[c.borderRadius,v("--radius-row")],padBlock:[c.paddingTop,v("--space-1")],padInline:[c.paddingLeft,v("--space-2")],gap:t[1].top-t[0].bottom,listPad:[getComputedStyle(l).padding,v("--space-2")],shelf:getComputedStyle(l.closest("[data-shelf]")||l.parentElement).backgroundColor,sunken:v("--sunken"),btnH:b.getBoundingClientRect().height,btnFont:[getComputedStyle(b).fontSize,v("--text-body")],lineFont:[getComputedStyle(w).fontSize,getComputedStyle(h).fontSize,v("--text-caption")],lineColor:[getComputedStyle(w).color,getComputedStyle(h).color],border:c.borderTopWidth,shadow:c.boxShadow,transition:c.transitionDuration})')"
ab hover "[data-todo='2'] .todo-row-button" >/dev/null
echo "U153 hovered wash over --ground: $(ev 'getComputedStyle(document.querySelector("[data-todo=\"2\"]")).backgroundImage') hover token $(ev 'getComputedStyle(document.documentElement).getPropertyValue("--hover")')"
# U154
echo "U154 clipped elements in rows (scrollWidth > clientWidth): $(ev 'JSON.stringify([...document.querySelectorAll("[data-todo][data-shelf-row], [data-todo][data-shelf-row] *")].filter(e=>e.scrollWidth>e.clientWidth+0).map(e=>e.className||e.tagName))') row wider than list: $(ev '[...document.querySelectorAll("[data-todo][data-shelf-row]")].some(r=>r.getBoundingClientRect().right>r.parentElement.getBoundingClientRect().right+0.5)')"
base=$(ev "$tops"); ab hover "[data-todo='3'] .todo-row-button" >/dev/null
echo "U154 tops same when hovered: $([ "$base" = "$(ev "$tops")" ] && echo yes || echo no) $base"
# U155 pointer: drag third row above first
load; ab hover body >/dev/null
echo "U155 start: $(ev "$ids")"
x=$(ev 'Math.round(document.querySelector("[data-todo=\"3\"] .todo-row-button").getBoundingClientRect().left+12)'); y3=$(ev 'Math.round(document.querySelector("[data-todo=\"3\"] .todo-row-button").getBoundingClientRect().bottom-5)'); y1=$(ev 'Math.round(document.querySelector("[data-todo=\"1\"] .todo-row-button").getBoundingClientRect().top+2)')
# the pointer presses the title's last line: on hover `move` and `complete` cover the first line (U35)
ab mouse move $x $y3 >/dev/null; ab mouse down >/dev/null; ab mouse move $x $((y3-20)) >/dev/null; ab mouse move $x $y1 >/dev/null; ab mouse up >/dev/null; sleep 1
echo "U155 after drag: $(ev "$ids") calls $(ev 'JSON.stringify(window.__fake.app.calls.filter(c=>c.method=="todo.reorder"))') a row selected (Drawer open): $(ev '!!document.querySelector("[data-todo][data-selected=true]")')"
# U155 keyboard (fresh load) with U156 frames
load
ab focus "[data-todo='3'] .todo-row-button" >/dev/null
ev 'window.__s=[];const f=()=>{window.__s.push([performance.now(),...[...document.querySelectorAll("[data-shelf-row]")].slice(0,4).map(r=>r.getBoundingClientRect().top)]);window.__raf=requestAnimationFrame(f)};f();0' >/dev/null
ab press Alt+ArrowUp >/dev/null; sleep 0.9; ev 'cancelAnimationFrame(window.__raf);0' >/dev/null
echo "U156 frames (ms since first, tops of rows 1-4 by place), first 14: $(ev 'const s=window.__s,t0=s[0][0];JSON.stringify(s.slice(0,40).filter((x,i)=>i==0||x.slice(1).join()!=s[i-1].slice(1).join()).slice(0,14).map(x=>[Math.round(x[0]-t0),...x.slice(1).map(Math.round)]))')"
echo "U156 ms from first moved frame to final top: $(ev '(()=>{const s=window.__s,k=x=>x.slice(1).join(),l=k(s[s.length-1]),a=s.findIndex(x=>k(x)!=k(s[0]));let z=a;while(k(s[z])!=l)z++;return Math.round(s[z][0]-s[a][0])})()')"
ab press Alt+ArrowUp >/dev/null; sleep 0.9; chk
echo "U155 after two Alt+Up: $(ev "$ids") active: $(ev 'document.activeElement.textContent.slice(0,6)') calls $(ev 'JSON.stringify(window.__fake.app.calls.filter(c=>c.method=="todo.reorder"))')"
ab reload >/dev/null; sleep 2
echo "U155 after reload (fake Daemon is rebuilt, so this is the seed order): $(ev "$ids")"
# U152 by keyboard: unblock, block, complete
load
ab focus "[data-todo='4'] .todo-row-button" >/dev/null; ab press Enter >/dev/null; sleep 0.5; chk
echo "U152 drawer waits-on: $(ev 'JSON.stringify([...document.querySelectorAll("aside button")].map(b=>b.getAttribute("aria-label")||b.textContent).filter(Boolean))')"
ab focus "aside button[aria-label='remove blocker #3']" >/dev/null; ab press Enter >/dev/null; sleep 0.8; chk
echo "U152 remove by keyboard: row 4 waits=$(ev 'document.querySelector("[data-todo=\"4\"] [data-todo-waits]")?.textContent') call=$(ev 'JSON.stringify(window.__fake.app.calls.filter(c=>c.method=="todo.setBlockers"))') active=$(ev 'document.activeElement.tagName')"
ab click "aside button[aria-label='remove blocker #5']" >/dev/null; sleep 0.8
echo "U152 remove by pointer: row 4 waits=$(ev 'document.querySelector("[data-todo=\"4\"] [data-todo-waits]")?.textContent ?? null')"
ab click "aside button[aria-label='close']" >/dev/null; sleep 0.3
done0=$(ev 'document.querySelector(".shelf").innerText.match(/(\d+) done/)?.[1]')
ab hover "[data-todo='2'] .todo-row-button" >/dev/null; ab click "[data-todo='2'] .complete" >/dev/null; sleep 0.8
echo "U152 complete by pointer: row 2 present=$(ev '!!document.querySelector("[data-todo=\"2\"][data-shelf-row]")') done $done0 -> $(ev 'document.querySelector(".shelf").innerText.match(/(\d+) done/)?.[1]')"
ab focus "[data-todo='1'] .todo-row-button" >/dev/null
for i in 1 2 3; do ab press Tab >/dev/null; chk; [ "$(ev 'document.activeElement.textContent')" = '"complete"' ] && break; done
echo "U152 Tab from the row reaches: $(ev 'document.activeElement.textContent')"
ab press Enter >/dev/null; sleep 0.8; chk
echo "U152 complete by keyboard: row 1 present=$(ev '!!document.querySelector("[data-todo=\"1\"][data-shelf-row]")') done -> $(ev 'document.querySelector(".shelf").innerText.match(/(\d+) done/)?.[1]') active=$(ev 'document.activeElement.tagName')"
# add by pointer, block by keyboard
load
ab click "button[aria-label='add todo']" >/dev/null; sleep 0.3
ab type "input, textarea" "ship it" >/dev/null 2>&1; ab press Enter >/dev/null; sleep 0.8
echo "U152 add by pointer: new row text=$(ev '[...document.querySelectorAll("[data-todo][data-shelf-row]")].map(r=>r.querySelector(".todo-row-button").textContent.trim()).filter(t=>/ship it/.test(t)).join()')"
ab focus "[data-todo='2'] .todo-row-button" >/dev/null; ab press Enter >/dev/null; sleep 0.5; chk
ab focus "aside button:has(svg.lucide-plus)" >/dev/null 2>&1 || ab focus "aside p button" >/dev/null; ab press Enter >/dev/null; sleep 0.3; chk
for i in 1 2 3 4; do [ "$(ev 'document.activeElement.textContent.trim().startsWith("#1 ")')" = true ] && break; ab press Tab >/dev/null; chk; done
ab press Enter >/dev/null; sleep 0.8; chk
echo "U152 block by keyboard: row 2 waits=$(ev 'document.querySelector("[data-todo=\"2\"] [data-todo-waits]")?.textContent') call=$(ev 'JSON.stringify(window.__fake.app.calls.filter(c=>c.method=="todo.setBlockers").slice(-1))') active=$(ev 'document.activeElement.tagName')"
echo "activeElement was body at a keyboard step: $body (1 = yes)"
