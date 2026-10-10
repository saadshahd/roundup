"""usage: settings.py <scripts_dir> <log_dir> <sleep_s> <reply_file|-> [timeout_s]
Prints a settings file: hook.sh on PermissionRequest (with timeout_s if given), loghook.sh on every other event."""
import json, sys

scripts, logs, sleep_s, reply = sys.argv[1:5]
timeout = sys.argv[5:6]
EVENTS = ["SessionStart", "UserPromptSubmit", "PreToolUse", "PostToolUse", "PostToolUseFailure",
          "PermissionDenied", "Notification", "Stop", "StopFailure", "SessionEnd"]


def hook(command, **extra):
    return [{"hooks": [{"type": "command", "command": command, **extra}]}]


hooks = {e: hook(f"{scripts}/loghook.sh {logs}") for e in EVENTS}
hooks["PermissionRequest"] = hook(f"{scripts}/hook.sh {sleep_s} {reply} {logs}",
                                  **({"timeout": int(timeout[0])} if timeout else {}))
print(json.dumps({"hooks": hooks}, indent=1))
