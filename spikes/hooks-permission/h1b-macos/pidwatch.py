"""usage: pidwatch.py <log_dir>
Every 0.05 s, logs to <log_dir>/pids.jsonl the time each hook pid and sleep pid in hook.jsonl stops existing (kill -0)."""
import json, os, sys, time

d = sys.argv[1]
started, gone = set(), set()
while True:
    try:
        with open(f"{d}/hook.jsonl") as f:
            for line in f:
                try:
                    r = json.loads(line)
                except ValueError:
                    continue
                if r["ev"] == "start":
                    started |= {("hook", r["pid"]), ("sleep", r["sleep_pid"])}
    except FileNotFoundError:
        pass
    for kind, pid in started - gone:
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            gone.add((kind, pid))
            with open(f"{d}/pids.jsonl", "a") as f:
                f.write(json.dumps({"t": round(time.time(), 3), kind: pid, "ev": "gone"}) + "\n")
    time.sleep(0.05)
