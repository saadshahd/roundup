# boxd use cases

boxd is optional: nothing on the critical path depends on it (merging needs GitHub's macOS `check`, the `loop` rules job on ubuntu, and local runs). This file says what a boxd VM adds, what it cannot do, and what was actually measured. "Verified" means observed on 2026-10-01; the commands and raw numbers are in `spikes/boxd/REPORT.md` and `loop/boxd.sh`. Anything else is marked unverified.

## What a VM adds

| Use case | What local cannot do | Recipe | Status |
|---|---|---|---|
| **Unattended Builder** | Run `claude -p --dangerously-skip-permissions` without exposing your laptop's files and credentials. The VM is the sandbox, and `--isolated` removes the connected integration tokens and the in-VM `boxd` CLI. | `loop/boxd.sh build <name> <prompt-file>` | Verified: 4 turns, 29 s wall for a small task, patch applied cleanly to `origin/main`. |
| **Parallel Builders** | Run N Builders with no shared disk, ports or caches. | One VM per Builder, started in parallel. | Verified at 4 (small tasks, 21–27 s wall each, no errors). Not tested above 4 or with large tasks. |
| **Linux check run** | Run `just check` on a clean Linux box in seconds. CI's macOS job also runs the `rup ping` round trip, which `just check` does not; CI uses Node 22 and the VM Node 24. | `just check` on a VM from the snapshot (the justfile is the single definition of `check`). | Verified: a whole `loop/boxd.sh build` (Builder run plus `just check`) took 29–37 s wall on a fresh VM from the snapshot. An earlier, shorter check plus the `rup ping` round trip took 15 s (see the report). Catches platform-neutral breakage before a PR. It is not the macOS gate. |
| **QA screenshots** | Render the web UI where an agent can drive it and nobody's windows get hijacked. | `agent-browser` in the VM, screenshots to `artifacts/ux/<scenario>/<step>.png`, then `boxd machine cp` out. | Pipeline verified with a static page, not with roundup's UI (none exists yet). |
| **Watch an agent live** | Let a human see the VM's screen. | `boxd machine desktop <vm> --json` returns a `desktop_url`. | URL issued on snapshot-based and `--isolated` VMs. Not viewed by a human, so what it shows is unverified. |
| **Sandbox for untrusted code** | Run extension code with no credentials and no in-VM `boxd`. | `boxd machine new --isolated` | Verified that `claude -p` and `agent-browser` work in an isolated VM and that egress is `unrestricted`. Inbound isolation and escape resistance not tested. Not needed until extensions exist. |

The strongest reason to use a VM is the first row: unattended permission-skipping. Parallelism alone is not an argument, because git worktrees already isolate files.

## What a VM cannot do

- Run the shipped webview. A Linux VM uses Chromium (agent-browser) or WebKitGTK, never macOS WKWebView.
- Produce perf numbers (cold start, keystroke latency, RSS) that mean anything for macOS.
- Exercise darwin PTY behaviour, the Dock badge, or anything native.
- Compare screenshots to a macOS baseline. Fonts differ (in the VM, `⏸` renders as a fallback glyph and `✕` as `×`), so visual baselines must be kept per platform.
- Raise the Max quota. Every VM shares one token and one usage window.

## Defaults for every VM

- Name prefix `ru-`, so a sweep can find them: `boxd machine list | grep '^ru-'`. `loop/boxd.sh build` refuses to start when 4 `ru-` VMs exist.
- `--auto-destroy-timeout 1800` (leak guard; `bake` uses 3600) and `--auto-suspend-timeout 0`, because CPU-only builds look idle and would be suspended mid-run. Both confirmed with `boxd machine get`. Auto-hibernate defaults to 14400 s of no network traffic; a job longer than 4 hours needs it set to 0.
- Create from the `ru-toolchain` snapshot (Rust from `rust-toolchain.toml`, clippy, rustfmt, nextest, cargo-machete, just, pnpm from `package.json`, Node 24). Boot reports 4–5 ms; `machine new` takes about 2.2 s wall. Desktop URLs and `agent-browser` work on snapshot-based machines. Rebake with `loop/boxd.sh bake` when `rust-toolchain.toml` or the pnpm pin changes. The snapshot is about 11.5 GB and is kept; `loop/boxd.sh bake` replaces it.
- Upload the commit under test with `git archive`, never a recursive copy (that would ship `node_modules/`, `target/` and other agents' state).
- Bring results back as a patch (`git format-patch` then `boxd machine cp`) and push from the laptop. GitHub credentials never go to the VM.

## Secrets

- Pass the Claude token per call: `boxd machine exec <vm> -e CLAUDE_CODE_OAUTH_TOKEN=... -- ...`. `boxd env set --secret` is account-wide and puts the token on every machine, so do not use it.
- Observed: after a Builder run, a search of the VM's home, `/tmp` and `/etc` found no copy of the token, and there was no `~/.claude/.credentials.json`. `boxd env list` was empty.
- By design the Builder holds the token in its environment, so `loop/boxd.sh` deletes any result or patch that contains it and refuses to print it. The token is also in `boxd`'s argument list on the laptop while a call runs (`ps` shows it), and in the VM's process environment. Treat the VM as trusted for the length of the call.
- Normal machines expose connected integration credentials to any code running in them. Use `--isolated` when running code you do not trust.

## Cost and quota

- Per run, notional list price: ping $0.09; small Sonnet task $0.08–0.14; Opus one-line review $0.17. On Max this is plan usage, not cash. Each fresh `claude -p` writes about 20k cache tokens, so prefer fewer, larger tasks.
- boxd credit stayed at about €30 across all spikes (29.997 on one reading, 30.00 after rounding).
- The JSON output has no quota field. `loop/boxd.sh` pauses on `api_error_status` 429 or "usage limit" / "rate limit" in the result by writing `loop/out/PAUSED` and exiting 75. Delete the file to resume. This path is covered by a stub test (`loop/boxd.test.sh`) but has never met a real limit error.
- Cap: 4 concurrent Builder VMs, the largest number measured. Raise it only after a run with realistic task sizes shows no limit errors.

## Cleanup

Every VM that `loop/boxd.sh` creates is destroyed by its exit trap, which reports a leaked VM if removal fails. VMs made by hand (QA runs) carry only the auto-destroy timer, so remove them yourself. After any session, `boxd machine list` should show no `ru-` machines. The only standing resource is the `ru-toolchain` snapshot.
