#!/bin/bash
# usage: observe-real.sh W theme   (needs `just harness-real <project> 5199` on a rupd holding Rooms auth-refactor, docs and Todos 1 to 3; Todo 3 is blocked by #1 and #2)
W=$1; T=$2; S=real-$W-$T
ab(){ agent-browser --session $S "$@" 2>&1 | tail -1; }
ev(){ agent-browser --session $S eval "$1" 2>&1 | tail -1; }
rpc(){ curl -s localhost:5199/__daemon/rpc -H 'content-type: application/json' -d "$1"; echo; }
dhome(){ rpc '{"method":"todo.list","params":{}}' | jq -c '.result[]|select(.id==2)|.home'; }
ab set viewport $W 800 >/dev/null
ab set media $T >/dev/null
ab open "http://localhost:5199/harness.html?daemon=1" >/dev/null; sleep 2
home(){ ev "document.querySelector('[data-todo=\"$1\"] [data-todo-home]').textContent"; }
echo "== $W $T"
echo "theme dark: $(ev 'matchMedia("(prefers-color-scheme: dark)").matches')"
rpc '{"method":"todo.move","params":{"id":2,"home":null}}' >/dev/null
ab hover "body" >/dev/null
echo "U151 pointer parked outside .shelf: $(ev 'JSON.stringify([...document.querySelectorAll("[data-shelf-row]")].map(r=>({btn:r.querySelector(".todo-row-button")?.textContent,waits:r.querySelector("[data-todo-waits]")?.textContent??null,home:r.querySelector("[data-todo-home]")?.textContent})))')"
echo "U151 text of row 3 is exactly the three parts: $(ev 'document.querySelector("[data-todo=\"3\"]").innerText.replace(/\n/g," | ")')"
tops='JSON.stringify([...document.querySelectorAll("[data-shelf-row]")].map(r=>Math.round(r.getBoundingClientRect().top)))'
echo "tops at rest: $(ev "$tops")"
echo "start: $(home 2)"
ab hover "[data-todo='2'] .todo-row-button" >/dev/null
echo "tops hovered, move shown: $(ev "$tops")"
ab click "[data-todo='2'] button[aria-label='move #2']" >/dev/null
echo "tops list open: $(ev "$tops")"
echo "offers: $(ev 'JSON.stringify([...document.querySelectorAll("[data-todo=\"2\"] .todo-home-offer")].map(e=>e.textContent))')"
ab click "[data-todo='2'] .todo-home-offer:nth-child(1)" >/dev/null; sleep 1
echo "after pointer move: $(home 2) active=$(ev 'document.activeElement.className')"
echo "daemon todo 2 home: $(dhome)"
ab reload >/dev/null; sleep 2
echo "after reload: $(home 2)"
never=0
chk(){ [ "$(ev 'document.activeElement.tagName')" = '"BODY"' ] && never=1; }
ab focus "[data-todo='2'] .todo-row-button" >/dev/null; ab press Enter >/dev/null; sleep 0.5; chk
ab focus "aside .todo-move" >/dev/null; ab press Enter >/dev/null; chk
echo "drawer offers: $(ev 'JSON.stringify([...document.querySelectorAll("aside .todo-home-offer")].map(e=>e.textContent))')"
for i in 1 2 3 4 5; do [ "$(ev 'document.activeElement.textContent')" = '"project root"' ] && break; ab press Tab >/dev/null; chk; done
echo "focused offer: $(ev 'document.activeElement.textContent')"
ab press Enter >/dev/null; sleep 1; chk
echo "after keyboard move: $(home 2) active=$(ev 'document.activeElement.className')"
ab reload >/dev/null; sleep 2
echo "after reload: $(home 2)"
echo "daemon todo 2 home: $(dhome)"
# U158: a Room made for this run, Todo 2 moved to it over the Daemon, then renamed and removed
id=$(rpc "{\"method\":\"rail.createRoom\",\"params\":{\"name\":\"first-$S\",\"parent\":null}}" | jq -r .result.id)
rpc "{\"method\":\"todo.move\",\"params\":{\"id\":2,\"home\":\"$id\"}}" >/dev/null; sleep 1
echo "in the run's Room: $(home 2)"
rpc "{\"method\":\"rail.rename\",\"params\":{\"id\":\"$id\",\"name\":\"renamed-room\"}}" >/dev/null; sleep 1
echo "after rename: $(home 2)"
rpc "{\"method\":\"rail.remove\",\"params\":{\"id\":\"$id\"}}" >/dev/null; sleep 1
echo "after remove: $(home 2)"
ab reload >/dev/null; sleep 2
echo "after remove and reload: $(home 2)"
echo "activeElement was body at a keyboard step: $never (1 = yes)"
