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
  FAKE_CLAUDE_CONTEXT_REPORT a file path: written with the stdout of the `rup context` command that
                           runs under SessionStart (E3), as Claude Code puts it in the session
  FAKE_CLAUDE_SHIM_DELAY_MS milliseconds to wait before the `roundup` server is started (E6)
  FAKE_CLAUDE_ON_PROMPT    JSON {"name", "arguments"} (U146): after SessionStart, once per prompt line typed into
                           the Terminal, play UserPromptSubmit, then this tool call with "$prompt" in a
                           string argument replaced by the typed text, then Stop
  FAKE_CLAUDE_ON_PROMPT_ASK JSON {"tool_name", "tool_input"} (U166), set with FAKE_CLAUDE_ON_PROMPT: after
                           UserPromptSubmit, play this PermissionRequest ("$prompt" in a string of
                           `tool_input` replaced by the typed text) and wait for its hook command to exit;
                           the tool call is made only when the Decision was allowed
  FAKE_CLAUDE_GATE         a file path: after the played PermissionRequest hook command (which waits for
                           the user's answer, H9) is started, the next event waits until the file exists,
                           as the user's own act in the Terminal (H7a) happens in its own time
  FAKE_CLAUDE_DOOR_TOOLS   like FAKE_CLAUDE_TOOLS, but made only when this run has `--tools` in its argv (F2), as a
                           Door's has and an Agent's has not
  FAKE_CLAUDE_TOOL_REPORT  a file path: written with the JSON list of what each tool call returned
  FAKE_CLAUDE_RUN          JSON list (F2, F4): each item is run with the `env` of the `--settings` file added
                           to the environment, as Claude Code does; a string by `sh -c`, a list directly
  FAKE_CLAUDE_RUN_REPORT   a file path: written with JSON [{"code", "stdout", "stderr"}] for each run
  FAKE_CLAUDE_STRAY        a script path (F5): started as `sh <path>`, a child of this process
  FAKE_CLAUDE_MCP_STRAY    a script path (F5): a fake MCP server is started as a child of this process and
                           told to call a tool, which starts `sh <path>` under it
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
import re
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
        commands = [hook["command"] for hook in settings["hooks"][event][0]["hooks"]]
        command = commands[0]
        if event == "PermissionRequest":
            # `rup permission` stays open until the Decision is answered or cleared; the later
            # Signal clears it, so the fake does not wait for it.
            hook = subprocess.Popen(command, shell=True, stdin=subprocess.PIPE, text=True)
            hook.stdin.write(json.dumps(payload))
            hook.stdin.close()
            gate = os.environ.get("FAKE_CLAUDE_GATE")
            while gate and not os.path.exists(gate):
                time.sleep(0.02)
            continue
        for command in commands:
            ran = subprocess.run(command, shell=True, input=json.dumps(payload), text=True, check=True, stdout=subprocess.PIPE)
            if " context " in command and "FAKE_CLAUDE_CONTEXT_REPORT" in os.environ:
                with open(os.environ["FAKE_CLAUDE_CONTEXT_REPORT"], "w") as out:
                    out.write(ran.stdout)


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


def run_commands():
    env = {**os.environ, **json.load(open(flag("--settings"))).get("env", {})}
    done = []
    for item in json.loads(os.environ["FAKE_CLAUDE_RUN"]):
        argv = ["sh", "-c", item] if isinstance(item, str) else item
        ran = subprocess.run(argv, env=env, capture_output=True, text=True)
        done.append({"code": ran.returncode, "stdout": ran.stdout, "stderr": ran.stderr})
    with open(os.environ["FAKE_CLAUDE_RUN_REPORT"], "w") as out:
        json.dump(done, out)


def play_mcp_stray(script):
    """A fake MCP server as a child of this process, as Claude Code starts one; its tool starts `script`."""
    server = subprocess.Popen(
        [sys.executable, os.path.join(os.path.dirname(os.path.abspath(__file__)), "fake_mcp.py")],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        text=True,
        env={**os.environ, "FAKE_STRAY": script},
    )
    server.stdin.write(json.dumps({"method": "tools/call"}) + "\n")
    server.stdin.flush()
    server.stdout.readline()
    return server


def hook(event, extra=None):
    command = json.load(open(flag("--settings")))["hooks"][event][0]["hooks"][0]["command"]
    payload = {"hook_event_name": event, **(extra or {})}
    subprocess.run(command, shell=True, input=json.dumps(payload), text=True, check=True, stdout=subprocess.PIPE)


def typed_prompts():
    """Each line typed into the Terminal, with the bracketed-paste markers of a Steer removed."""
    pending = b""
    while True:
        chunk = os.read(0, 4096)
        if not chunk:
            return
        pending += chunk
        while True:
            ends = [i for i in (pending.find(b"\r"), pending.find(b"\n")) if i >= 0]
            if not ends:
                break
            line, pending = pending[: min(ends)], pending[min(ends) + 1 :]
            text = line.replace(b"\x1b[200~", b"").replace(b"\x1b[201~", b"").decode(errors="replace").strip()
            # The Daemon heads a Message with `[from <sender>, <kind>] `; the prompt is its body.
            text = re.sub(r"^\[[^\]]*\] ", "", text)
            if text:
                yield text


def ask_permission(ask, prompt):
    """Play a PermissionRequest and wait for `rup permission` to exit; whether the Decision was allowed."""
    command = json.load(open(flag("--settings")))["hooks"]["PermissionRequest"][0]["hooks"][0]["command"]
    tool_input = {key: value.replace("$prompt", prompt) if isinstance(value, str) else value for key, value in ask.get("tool_input", {}).items()}
    payload = {"hook_event_name": "PermissionRequest", "tool_name": ask["tool_name"], "tool_input": tool_input}
    ran = subprocess.run(command, shell=True, input=json.dumps(payload), text=True, check=True, stdout=subprocess.PIPE)
    return json.loads(ran.stdout or "{}").get("hookSpecificOutput", {}).get("decision", {}).get("behavior") == "allow"


def play_prompts(tool):
    hook("SessionStart", {"session_id": "00000000-0000-4000-8000-000000000146", "source": "startup"})
    for prompt in typed_prompts():
        hook("UserPromptSubmit", {"prompt": prompt})
        if "FAKE_CLAUDE_ON_PROMPT_ASK" in os.environ and not ask_permission(json.loads(os.environ["FAKE_CLAUDE_ON_PROMPT_ASK"]), prompt):
            hook("Stop")
            continue
        arguments = {key: value.replace("$prompt", prompt) if isinstance(value, str) else value for key, value in tool.get("arguments", {}).items()}
        mcp_session([{**tool, "arguments": arguments}])
        hook("Stop")


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
    results = []
    for n, tool in enumerate(calls):
        arguments = {
            key: last if value == "$id" else value
            for key, value in tool.get("arguments", {}).items()
        }
        result = request(10 + n, "tools/call", {**tool, "arguments": arguments})
        assert not result.get("isError"), result
        last = json.loads(result["content"][0]["text"]).get("id")
        results.append(json.loads(result["content"][0]["text"]))
    if "FAKE_CLAUDE_TOOL_REPORT" in os.environ and results:
        with open(os.environ["FAKE_CLAUDE_TOOL_REPORT"], "w") as out:
            json.dump(results, out)


if "FAKE_CLAUDE_EVENTS" in os.environ:
    play_hooks()
if "FAKE_CLAUDE_RUN" in os.environ:
    run_commands()
stray = [subprocess.Popen(["sh", os.environ["FAKE_CLAUDE_STRAY"]])] if "FAKE_CLAUDE_STRAY" in os.environ else []
if "FAKE_CLAUDE_MCP_STRAY" in os.environ:
    stray.append(play_mcp_stray(os.environ["FAKE_CLAUDE_MCP_STRAY"]))
if "FAKE_CLAUDE_LOOP" in os.environ:
    play_loop()
if any(k in os.environ for k in ("FAKE_CLAUDE_TOOL", "FAKE_CLAUDE_TOOLS", "FAKE_CLAUDE_BRIEF_REPORT")):
    calls = json.loads(os.environ.get("FAKE_CLAUDE_TOOLS", "[]"))
    if "FAKE_CLAUDE_TOOL" in os.environ:
        calls.insert(0, json.loads(os.environ["FAKE_CLAUDE_TOOL"]))
    time.sleep(int(os.environ.get("FAKE_CLAUDE_SHIM_DELAY_MS", "0")) / 1000)
    mcp_session(calls, os.environ.get("FAKE_CLAUDE_BRIEF_REPORT"))
if "FAKE_CLAUDE_DOOR_TOOLS" in os.environ and "--tools" in sys.argv:
    mcp_session(json.loads(os.environ["FAKE_CLAUDE_DOOR_TOOLS"]))
if "FAKE_CLAUDE_ON_PROMPT" in os.environ:
    play_prompts(json.loads(os.environ["FAKE_CLAUDE_ON_PROMPT"]))
time.sleep(3600)
