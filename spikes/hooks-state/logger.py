#!/usr/bin/env python3
import sys, json, time, os
raw = sys.stdin.read()
try: payload = json.loads(raw)
except Exception: payload = {"_unparsed": raw}
with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "log.jsonl"), "a") as f:
    f.write(json.dumps({"ts": time.time(), "payload": payload}) + "\n")
