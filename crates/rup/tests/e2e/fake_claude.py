#!/usr/bin/env python3
"""Stands in for `claude` in the end-to-end tests.

It does only what Claude Code does that roundup can see: it runs the hook command from the
`--settings` file once per payload, and talks MCP to the `roundup` server from the `--mcp-config`
file. What it does comes from the environment, so one script serves every scenario:

  FAKE_CLAUDE_PAYLOADS     a JSONL file of recorded hook payloads
  FAKE_CLAUDE_EVENTS       comma-separated hook event names; the first recorded payload of each is played
  FAKE_CLAUDE_PROMPT       replaces the prompt of the played UserPromptSubmit
  FAKE_CLAUDE_TOOL         JSON {"name", "arguments"}: a tool to call on the `roundup` server
  FAKE_CLAUDE_TOOLS        JSON list of {"name", "arguments"}: tools to call in order on the `roundup`
                           server; the string "$id" in an argument is the `id` of the previous result
  FAKE_CLAUDE_BRIEF_REPORT a file path: written with JSON {"brief": the text of the file named by
                           `--append-system-prompt-file`, "tools": the names the server lists}, as
                           the Brief and the real MCP list stand together
  FAKE_CLAUDE_LOOP         a count N: run a tool-use loop of N iterations, each trying the hook
                           command for PreToolUse then PostToolUse (H15, scenario control.md);
                           an event with no entry in `--settings` (a dropped one) is skipped, the
                           same way Claude Code itself never runs a command for an unregistered
                           event
  FAKE_CLAUDE_LOOP_REPORT  a file path: after the loop, written with a JSON list of the event
                           names whose hook command actually ran, in order, between a UserPromptSubmit and a final Stop

It then sleeps until the Daemon stops it.
"""
import json
import os
import subprocess
import sys
import time


def flag(name):
    return sys.argv[sys.argv.index(name) + 1]


def play_hooks():
    settings = json.load(open(flag("--settings")))
    events = os.environ["FAKE_CLAUDE_EVENTS"].split(",")
    recorded = {}
    for line in open(os.environ["FAKE_CLAUDE_PAYLOADS"]):
        payload = json.loads(line)["payload"]
        recorded.setdefault(payload["hook_event_name"], payload)
    for event in events:
        payload = dict(recorded[event])
        if event == "UserPromptSubmit" and "FAKE_CLAUDE_PROMPT" in os.environ:
            payload["prompt"] = os.environ["FAKE_CLAUDE_PROMPT"]
        command = settings["hooks"][event][0]["hooks"][0]["command"]
        subprocess.run(command, shell=True, input=json.dumps(payload), text=True, check=True)


def play_loop():
    settings = json.load(open(flag("--settings")))
    count = int(os.environ["FAKE_CLAUDE_LOOP"])
    ran = []
    start = settings["hooks"]["UserPromptSubmit"][0]["hooks"][0]["command"]
    subprocess.run(start, shell=True, input=json.dumps({"hook_event_name": "UserPromptSubmit", "prompt": "loop"}), text=True, check=True)
    for _ in range(count):
        for event in ("PreToolUse", "PostToolUse"):
            hooks = settings["hooks"].get(event)
            if not hooks:
                continue
            payload = json.dumps({"hook_event_name": event, "tool_name": "Bash"})
            command = hooks[0]["hooks"][0]["command"]
            subprocess.run(command, shell=True, input=payload, text=True, check=True)
            ran.append(event)
    stop = settings["hooks"]["Stop"][0]["hooks"][0]["command"]
    subprocess.run(stop, shell=True, input=json.dumps({"hook_event_name": "Stop"}), text=True, check=True)
    if "FAKE_CLAUDE_LOOP_REPORT" in os.environ:
        with open(os.environ["FAKE_CLAUDE_LOOP_REPORT"], "w") as report:
            json.dump(ran, report)


def mcp_session(calls, report=None):
    config = json.load(open(flag("--mcp-config")))["mcpServers"]["roundup"]
    server = subprocess.Popen(
        [config["command"], *config["args"]],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        text=True,
    )

    def send(message):
        server.stdin.write(json.dumps({"jsonrpc": "2.0", **message}) + "\n")
        server.stdin.flush()

    def request(id, method, params):
        send({"id": id, "method": method, "params": params})
        reply = json.loads(server.stdout.readline())
        assert reply["id"] == id and "error" not in reply, reply
        return reply["result"]

    request(1, "initialize", {
        "protocolVersion": "2025-06-18",
        "capabilities": {},
        "clientInfo": {"name": "fake-claude", "version": "0"},
    })
    send({"method": "notifications/initialized"})
    if report:
        names = [t["name"] for t in request(2, "tools/list", {})["tools"]]
        brief = open(flag("--append-system-prompt-file")).read()
        with open(report, "w") as out:
            json.dump({"brief": brief, "tools": names}, out)
    last = None
    for n, tool in enumerate(calls):
        arguments = {
            key: last if value == "$id" else value
            for key, value in tool.get("arguments", {}).items()
        }
        result = request(10 + n, "tools/call", {**tool, "arguments": arguments})
        assert not result.get("isError"), result
        last = json.loads(result["content"][0]["text"]).get("id")


if "FAKE_CLAUDE_EVENTS" in os.environ:
    play_hooks()
if "FAKE_CLAUDE_LOOP" in os.environ:
    play_loop()
if any(k in os.environ for k in ("FAKE_CLAUDE_TOOL", "FAKE_CLAUDE_TOOLS", "FAKE_CLAUDE_BRIEF_REPORT")):
    calls = json.loads(os.environ.get("FAKE_CLAUDE_TOOLS", "[]"))
    if "FAKE_CLAUDE_TOOL" in os.environ:
        calls.insert(0, json.loads(os.environ["FAKE_CLAUDE_TOOL"]))
    mcp_session(calls, os.environ.get("FAKE_CLAUDE_BRIEF_REPORT"))
time.sleep(3600)
