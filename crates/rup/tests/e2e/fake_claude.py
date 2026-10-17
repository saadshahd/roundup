#!/usr/bin/env python3
"""Stands in for `claude` in the end-to-end tests.

It does only what Claude Code does that roundup can see: it runs the hook command from the
`--settings` file, and talks MCP to the `roundup` server from the `--mcp-config`
file. What it does comes from the environment, so one script serves every scenario:

  FAKE_CLAUDE_BRIEF_REPORT a file path: written with JSON {"brief": the text of the file named by
                           `--append-system-prompt-file`, "tools": the names the server lists}, as
                           the Brief and the real MCP list stand together
  FAKE_CLAUDE_PROMPT_LOG   a file path (A32): each typed prompt is appended as
                           one JSON line {"resume": whether this run has `--resume`, "prompt": the text}
  FAKE_CLAUDE_BLOCKED_LOG  a file path (B39): each typed prompt whose UserPromptSubmit hook exited 2 is appended as one
                           JSON line {"prompt": the text}; Claude Code runs no turn for it, so neither does this
  FAKE_CLAUDE_HOLD_PROMPT  a file path (B40): a typed prompt that carries Messages is logged, then its UserPromptSubmit waits until the file
                           exists, as a Daemon that is killed meanwhile never gets that Signal
  FAKE_CLAUDE_CHILD_PROCESS / FAKE_CLAUDE_DOOR_PROCESS
                           JSON argv (H21): started as a child of this process, with the environment of the `--settings`
                           file added, by a run with no `--tools` in its argv / with it; a process below `claude`
  FAKE_CLAUDE_HELD_START   a file path (A32): created when HOLD_START blocks before SessionStart
  FAKE_CLAUDE_LAUNCH_LOG   a file path (A36, A37): each start appends one JSON line {"argv", "env"}: the arguments it was
                           started with and the ANTHROPIC_MODEL and CLAUDE_CODE_EFFORT_LEVEL it saw. The run then plays
                           SessionStart (naming the conversation `--resume` gave, else a fixed one), idles, and appends
                           each line typed into the Terminal to FAKE_CLAUDE_INPUT_LOG, when that is set
  FAKE_CLAUDE_RUN          JSON list (F2, F4): each item is run with the `env` of the `--settings` file added
                           to the environment, as Claude Code does; a string by `sh -c`, a list directly
  FAKE_CLAUDE_RUN_REPORT   a file path: written with JSON [{"code", "stdout", "stderr"}] for each run
  FAKE_CLAUDE_STRAY        a script path (F5): started as `sh <path>`, a child of this process
  FAKE_CLAUDE_MCP_STRAY    a script path (F5): a fake MCP server is started as a child of this process and
                           told to call a tool, which starts `sh <path>` under it

It then sleeps until the Daemon stops it.
"""
import json
import os
import re
import subprocess
import sys
import time


# The fake models the vendor capability probe; real loading has its own Q11 observer.
if sys.argv[1:3] == ["plugin", "test"]:
    print("Q11 runtime hook closure: 1 pass")
    sys.exit(0)


def flag(name):
    return sys.argv[sys.argv.index(name) + 1]


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


def run_hook(command, payload, blocks=False, **options):
    """Run one hook command as Claude Code does: any failure but exit 2 goes to stderr and the session goes on,
    as after a `rup signal` that timed out on a busy Daemon. Exit 2 would block, which `rup` asks for only to drop a
    prompt that repeats delivered Messages (B39), so it raises unless the caller blocks."""
    ran = subprocess.run(command, shell=True, input=payload, text=True, **options)
    if ran.returncode == 2 and not blocks:
        ran.check_returncode()
    if ran.returncode:
        sys.stderr.write(f"hook exited {ran.returncode}: {command}\n")
    return ran


BLOCKED = []


def hook(event, extra=None):
    """Play one hook event. A UserPromptSubmit that exits 2 blocks its prompt (B39): it is kept in BLOCKED."""
    command = json.load(open(flag("--settings")))["hooks"][event][0]["hooks"][0]["command"]
    payload = {"hook_event_name": event, **(extra or {})}
    ran = run_hook(command, json.dumps(payload), blocks=event == "UserPromptSubmit", stdout=subprocess.PIPE)
    if ran.returncode == 2:
        BLOCKED.append(payload["prompt"])


def split_line(pending):
    """The first typed line of `pending` and the rest; a newline inside a bracketed paste does not end it."""
    inside = False
    for at in range(len(pending)):
        if pending.startswith(b"\x1b[200~", at):
            inside = True
        elif pending.startswith(b"\x1b[201~", at):
            inside = False
        elif not inside and pending[at] in b"\r\n":
            return pending[:at], pending[at + 1 :]
    return None


def typed_prompts():
    """Submitted prompts, keeping a bracketed paste whole and excluding roundup Context from the scripted work."""
    pending = b""
    while True:
        chunk = os.read(0, 4096)
        if not chunk:
            return
        pending += chunk
        while (split := split_line(pending)) is not None:
            line, pending = split
            text = line.replace(b"\x1b[200~", b"").replace(b"\x1b[201~", b"").decode(errors="replace").strip()
            # H20: the typed lead-in of a Steer, outside the paste; the prompt is the pasted body.
            text = text.removeprefix("Follow this instruction: ")
            text = text.split("\n\n<roundup-context>\n", 1)[0]
            # The Daemon heads a Message with `[from <sender>, <kind>] `; the prompt is its body.
            text = re.sub(r"^\[[^\]]*\] ", "", text)
            if text:
                yield text


def play_prompts():
    hold = os.environ.get("FAKE_CLAUDE_HOLD_START")
    if hold and not os.path.exists(hold) and "FAKE_CLAUDE_HELD_START" in os.environ:
        with open(os.environ["FAKE_CLAUDE_HELD_START"], "w"):
            pass
    while hold and not os.path.exists(hold):
        time.sleep(0.02)
    if "--resume" in sys.argv:
        # U177: a reopened Door is acknowledged by a SessionStart that names the conversation it resumed.
        hook("SessionStart", {"session_id": flag("--resume"), "source": "resume"})
    else:
        hook("SessionStart", {"session_id": "00000000-0000-4000-8000-000000000146", "source": "startup"})
    sys.stdout.write("\033]0;\u2733 claude\007")  # the star title that opens H19's gate, after SessionStart, so an idle child already has its conversation saved
    sys.stdout.flush()
    for prompt in typed_prompts():
        if "FAKE_CLAUDE_PROMPT_LOG" in os.environ:
            with open(os.environ["FAKE_CLAUDE_PROMPT_LOG"], "a") as log:
                log.write(json.dumps({"resume": "--resume" in sys.argv, "prompt": prompt}) + "\n")
        gate = os.environ.get("FAKE_CLAUDE_HOLD_PROMPT")
        while gate and "[rup messages: " in prompt and not os.path.exists(gate):
            time.sleep(0.02)
        hook("UserPromptSubmit", {"prompt": prompt})
        if BLOCKED:
            # B39: the prompt is blocked, so no turn runs and no Stop follows.
            BLOCKED.clear()
            if "FAKE_CLAUDE_BLOCKED_LOG" in os.environ:
                with open(os.environ["FAKE_CLAUDE_BLOCKED_LOG"], "a") as log:
                    log.write(json.dumps({"prompt": prompt}) + "\n")
            continue
        hook("Stop")


def brief_report(report):
    """Talk MCP to the `roundup` server and write what it lists with the Brief the Terminal was given."""
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
    names = [t["name"] for t in request(2, "tools/list", {})["tools"]]
    brief = open(flag("--append-system-prompt-file")).read()
    with open(report, "w") as out:
        json.dump({"brief": brief, "tools": names}, out)


def log_launch():
    seen = {name: os.environ[name] for name in ("ANTHROPIC_MODEL", "CLAUDE_CODE_EFFORT_LEVEL") if name in os.environ}
    with open(os.environ["FAKE_CLAUDE_LAUNCH_LOG"], "a") as log:
        log.write(json.dumps({"argv": sys.argv[1:], "env": seen, "functionHooks": os.environ.get("CLAUDE_CODE_ENABLE_FUNCTION_HOOKS")}) + "\n")


def play_session():
    """A36, A37: SessionStart, so the Daemon saves the conversation (A21), then record what is typed."""
    sys.stdout.write("\033]0;\u2733 claude\007")
    sys.stdout.flush()
    if "--resume" in sys.argv:
        hook("SessionStart", {"session_id": flag("--resume"), "source": "resume"})
    else:
        hook("SessionStart", {"session_id": "00000000-0000-4000-8000-000000000036", "source": "startup"})
    for line in typed_prompts():
        if "FAKE_CLAUDE_INPUT_LOG" in os.environ:
            with open(os.environ["FAKE_CLAUDE_INPUT_LOG"], "a") as log:
                log.write(line + "\n")


if "FAKE_CLAUDE_LAUNCH_LOG" in os.environ:
    log_launch()
if "FAKE_CLAUDE_RUN" in os.environ:
    run_commands()
stray = [subprocess.Popen(["sh", os.environ["FAKE_CLAUDE_STRAY"]])] if "FAKE_CLAUDE_STRAY" in os.environ else []
process = os.environ.get("FAKE_CLAUDE_DOOR_PROCESS" if "--tools" in sys.argv else "FAKE_CLAUDE_CHILD_PROCESS")
if process:
    settings_env = json.load(open(flag("--settings"))).get("env", {})
    stray.append(subprocess.Popen(json.loads(process), env={**os.environ, **settings_env}))
if "FAKE_CLAUDE_MCP_STRAY" in os.environ:
    stray.append(play_mcp_stray(os.environ["FAKE_CLAUDE_MCP_STRAY"]))
if "FAKE_CLAUDE_BRIEF_REPORT" in os.environ:
    brief_report(os.environ["FAKE_CLAUDE_BRIEF_REPORT"])
if "FAKE_CLAUDE_PROMPT_LOG" in os.environ:
    play_prompts()
if "FAKE_CLAUDE_LAUNCH_LOG" in os.environ:
    play_session()
time.sleep(3600)
