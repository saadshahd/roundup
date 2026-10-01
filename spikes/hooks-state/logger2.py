#!/usr/bin/env python3
import sys, json, time
raw = sys.stdin.read()
try: payload = json.loads(raw)
except Exception: payload = {"_unparsed": raw}
with open(sys.argv[1], "a") as f:
    f.write(json.dumps({"ts": time.time(), "payload": payload}) + "\n")
