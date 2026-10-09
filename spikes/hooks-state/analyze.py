import json,sys
for n in sys.argv[1:]:
    ev=[]
    for l in open(f"log.run{n}.jsonl"):
        r=json.loads(l);p=r["payload"];e=p["hook_event_name"]
        if e in("MessageDisplay",):continue
        ev.append((r["ts"],e+(":"+p["mark"] if e=="MARK" else "")+(" is_interrupt=%s"%p.get("is_interrupt") if e=="PostToolUseFailure" else "")+(" err=%s"%str(p.get("error"))[:50] if e=="PostToolUseFailure" else "")))
    for l in open(f"screen.run{n}.jsonl"):
        r=json.loads(l)
        if "title" in r: ev.append((r["ts"],"TITLE "+r["title"]))
    ev.sort();t0=ev[0][0]
    print("== run",n)
    for t,e in ev:
        if e.startswith("MARK:down") or "trust" in e: continue
        print("%8.3f %s"%(t-t0,e))
