#!/usr/bin/env python3
"""A user's own MCP server for the F5 test: its tool starts the program named by FAKE_STRAY, which it
leaves running, as a tool that runs a command under the hood does. It speaks one JSON line per call."""
import json
import os
import subprocess
import sys

children = []
for line in sys.stdin:
    if json.loads(line).get("method") == "tools/call":
        children.append(subprocess.Popen(["sh", os.environ["FAKE_STRAY"]]))
    print(json.dumps({"result": "ok"}), flush=True)
