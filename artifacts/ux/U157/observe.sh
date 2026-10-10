#!/bin/bash
# usage: run.sh W theme
W=$1; T=$2; S=obs-$W-$T
ab(){ agent-browser --session $S "$@" 2>&1 | tail -1; }
ev(){ agent-browser --session $S eval "$1" 2>&1 | tail -1; }
ab set viewport $W 800 >/dev/null
ab set media $T >/dev/null
ab open "http://localhost:5199/harness.html?seed=tree-40" >/dev/null
home='document.querySelector("[data-todo=\"2\"] [data-todo-home]").textContent'
active='document.activeElement.tagName'
tops='JSON.stringify([...document.querySelectorAll("[data-shelf-row]")].map(r=>Math.round(r.getBoundingClientRect().top)))'
echo "== $W $T"
echo "theme $(ev 'matchMedia("(prefers-color-scheme: dark)").matches')"
echo "start: $(ev "$home")"
ab hover "[data-todo='2'] .todo-row-button" >/dev/null
echo "tops hovered, move hidden/shown: $(ev "$tops")"
ab hover "body" >/dev/null
echo "tops rest: $(ev "$tops")"
ab hover "[data-todo='2'] .todo-row-button" >/dev/null
ab click "[data-todo='2'] button[aria-label='move #2']" >/dev/null
echo "tops list open: $(ev "$tops")"
echo "offers: $(ev 'JSON.stringify([...document.querySelectorAll("[data-todo=\"2\"] .todo-home-offer")].map(e=>e.textContent))')"
ab click "[data-todo='2'] .todo-home-offer:nth-child(2)" >/dev/null
echo "after pointer move: $(ev "$home") active=$(ev "$active")"
never=0
chk(){ [ "$(ev 'document.activeElement.tagName')" = '"BODY"' ] && never=1; }
ab focus "[data-todo='2'] .todo-row-button"; ab press Enter >/dev/null; chk
ab focus "aside .todo-move"; ab press Enter >/dev/null; chk
echo "drawer offers: $(ev 'JSON.stringify([...document.querySelectorAll("aside .todo-home-offer")].map(e=>e.textContent))')"
for i in 1 2 3 4 5; do [ "$(ev 'document.activeElement.textContent')" = '"project root"' ] && break; ab press Tab >/dev/null; chk; done
echo "focused offer: $(ev 'document.activeElement.textContent')"
ab press Enter >/dev/null; chk
echo "after keyboard move: $(ev "$home") active=$(ev 'document.activeElement.className')"
echo "todo.move calls: $(ev 'JSON.stringify(window.__fake.app.calls.filter(c=>c.method=="todo.move").map(c=>c.params))')"
ab press Escape >/dev/null
ab hover "[data-todo='2'] .todo-row-button" >/dev/null
ab click "[data-todo='2'] button[aria-label='move #2']" >/dev/null
ab click "[data-todo='2'] .todo-home-offer:nth-child(2)" >/dev/null
echo "again: $(ev "$home")"
ID='window.__fake.app.calls.filter(c=>c.method=="todo.move").pop().params.home' 
ev "window.__fake.app.rpc('rail.rename',{id:($ID),name:'renamed-room'}).then(()=>1)" >/dev/null; sleep 0.5
sleep 0.5
echo "after rename: $(ev "$home")"
ev "window.__fake.app.rpc('rail.remove',{id:($ID)}).then(()=>1)" >/dev/null; sleep 0.5
echo "after remove: $(ev "$home")"
echo "activeElement was body at a keyboard step: $never (1 = yes)"
