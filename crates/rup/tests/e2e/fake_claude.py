#!/usr/bin/env python3
"""Stands in for `claude` in the end-to-end tests.

It does only what Claude Code does that roundup can see: it runs the hook command from the
`--settings` file once per payload, and talks MCP to the `roundup` server from the `--mcp-config`
file. What it does comes from the environment, so one script serves every scenario:

  FAKE_CLAUDE_PAYLOADS  a JSONL file of recorded hook payloads
  FAKE_CLAUDE_EVENTS    comma-separated hook event names; the first recorded payload of each is played
  FAKE_CLAUDE_PROMPT    replaces the prompt of the played UserPromptSubmit
  FAKE_CLAUDE_TOOL      JSON {"name", "arguments"}: a tool to call on the `roundup` server

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


def call_tool(tool):
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
    result = request(2, "tools/call", tool)
    assert not result.get("isError"), result


if "FAKE_CLAUDE_EVENTS" in os.environ:
    play_hooks()
if "FAKE_CLAUDE_TOOL" in os.environ:
    call_tool(json.loads(os.environ["FAKE_CLAUDE_TOOL"]))
time.sleep(3600)
