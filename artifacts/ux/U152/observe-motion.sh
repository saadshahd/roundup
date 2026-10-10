#!/bin/bash
# usage: observe-motion.sh W theme   (needs `just harness tree-40 5199`)
# U156: reduced motion takes places in one frame; an interrupted second move continues from the screen position.
W=$1; T=$2; S=mot-$W-$T
ab(){ agent-browser --session $S "$@" 2>&1 | tail -1; }
ev(){ agent-browser --session $S eval "$1" 2>&1 | tail -1; }
load(){ ab open "http://localhost:5199/harness.html?seed=tree-40" >/dev/null; sleep 2; }
rec='window.__s=[];const f=()=>{window.__s.push([performance.now(),...[...document.querySelectorAll("[data-shelf-row]")].slice(0,4).map(r=>r.getBoundingClientRect().top)]);window.__raf=requestAnimationFrame(f)};f();0'
ab set viewport $W 800 >/dev/null
# reduced motion
ab set media $T reduced-motion >/dev/null; load
echo "== $W $T reduced=$(ev 'matchMedia("(prefers-reduced-motion: reduce)").matches')"
ab focus "[data-todo='3'] .todo-row-button" >/dev/null; ev "$rec" >/dev/null
ab press Alt+ArrowUp >/dev/null; sleep 0.6; ev 'cancelAnimationFrame(window.__raf);0' >/dev/null
echo "U156 reduced: distinct top sets while sampling: $(ev 'const s=window.__s,k=x=>x.slice(1).map(Math.round).join();JSON.stringify([...new Set(s.map(k))])') (two sets = one frame)"
# interruption: second move at half the distance
ab set media $T >/dev/null; load
ab focus "[data-todo='3'] .todo-row-button" >/dev/null; ev "$rec" >/dev/null
ev 'const key=()=>document.activeElement.dispatchEvent(new KeyboardEvent("keydown",{key:"ArrowUp",altKey:true,bubbles:true,cancelable:true}));key();setTimeout(()=>{window.__cut=window.__s.length;key()},75);0' >/dev/null
sleep 0.9; ev 'cancelAnimationFrame(window.__raf);0' >/dev/null
echo "U156 interrupt: row-3 top around the second press (last-before, first-after, by frame): $(ev 'const s=window.__s,c=window.__cut;JSON.stringify(s.slice(c-3,c+4).map(x=>[Math.round(x[0]-s[0][0]),...x.slice(1).map(v=>Math.round(v*10)/10)]))')"
echo "U156 interrupt final ids: $(ev 'JSON.stringify([...document.querySelectorAll("[data-shelf-row]")].map(r=>r.dataset.todo).slice(0,3))')"
