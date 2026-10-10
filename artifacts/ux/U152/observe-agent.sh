#!/bin/bash
# usage: observe-agent.sh W theme   (needs `just harness tree-40 5199`)
# U155 under an Agent that created Todos 1 to 3 (U60): the fake seed's Todos are the user's, so `todo.list` is wrapped in the page to give them creator agent-1, then a `todo.unblocked` event makes the list refetch.
W=$1; T=$2; S=obsag-$W-$T
ab(){ agent-browser --session $S "$@" 2>&1 | tail -1; }
ev(){ agent-browser --session $S eval "$1" 2>&1 | tail -1; }
ids='JSON.stringify([...document.querySelectorAll("[data-pads] [data-todo]")].map(r=>r.dataset.todo))'
calls='JSON.stringify(window.__fake.app.calls.filter(c=>c.method=="todo.reorder").map(c=>c.params))'
stage(){
  ab open "http://localhost:5199/harness.html?seed=tree-40" >/dev/null; sleep 2
  ev 'const a=window.__fake.app,o=a.rpc.bind(a);a.rpc=async(m,...r)=>{const v=await o(m,...r);return m==="todo.list"?v.map(t=>t.id<=3?{...t,creator:{kind:"agent",id:"agent-1",parent:null}}:t):v};window.__fake.emit({actor:{kind:"user"},name:"todo.unblocked",data:0});0' >/dev/null; sleep 1
  ev 'document.querySelector("[data-pads-control][aria-expanded=false]")?.click();0' >/dev/null; sleep 0.5
}
ab set viewport $W 800 >/dev/null; ab set media $T >/dev/null
stage
echo "== $W $T dark=$(ev 'matchMedia("(prefers-color-scheme: dark)").matches') (Agent-created Todos, expanded)"
echo "start: $(ev "$ids") rail width $(ev 'Math.round(document.querySelector(".rail-pads").getBoundingClientRect().width)')"
ab focus "[data-pads] [data-todo='3'] .todo-row-button" >/dev/null
ab press Alt+ArrowUp >/dev/null; sleep 0.9; ab press Alt+ArrowUp >/dev/null; sleep 0.9
echo "keyboard, Alt+ArrowUp twice on #3: $(ev "$ids") calls $(ev "$calls") active is #3's row button: $(ev 'document.activeElement.closest("[data-todo]")?.dataset.todo==="3"&&document.activeElement.classList.contains("todo-row-button")')"
stage
echo "pointer start: $(ev "$ids")"
x=$(ev 'Math.round(document.querySelector("[data-pads] [data-todo=\"3\"] .todo-row-button").getBoundingClientRect().left+12)'); y3=$(ev 'Math.round(document.querySelector("[data-pads] [data-todo=\"3\"] .todo-row-button").getBoundingClientRect().bottom-5)'); y1=$(ev 'Math.round(document.querySelector("[data-pads] [data-todo=\"1\"] .todo-row-button").getBoundingClientRect().top+2)')
ab mouse move $x $y3 >/dev/null; ab mouse down >/dev/null; ab mouse move $x $((y3-20)) >/dev/null; ab mouse move $x $y1 >/dev/null; ab mouse up >/dev/null; sleep 1
echo "pointer, drag #3 above #1: $(ev "$ids") calls $(ev "$calls") a row selected (Drawer open): $(ev '!!document.querySelector("[data-todo][data-selected=true]")')"
