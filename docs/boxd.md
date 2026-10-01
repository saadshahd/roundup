# boxd use cases

boxd is optional: nothing on the critical path depends on it (merging needs GitHub's macOS `check` and local runs; the `loop` job on ubuntu is advisory until it is made a required check). This file says what a boxd VM adds, what it cannot do, and what was actually measured. "Verified" means observed on 2026-10-01; the commands and raw numbers are in `spikes/boxd/REPORT.md` and `loop/boxd.sh`. Anything else is marked unverified.

## What a VM adds

| Use case | What local cannot do | Recipe | Status |
|---|---|---|---|
| **Unattended Builder** | Run `claude -p --dangerously-skip-permissions` without exposing your laptop's files and credentials. The VM is the sandbox, and `--isolated` removes the connected integration tokens and the in-VM `boxd` CLI. | `loop/boxd.sh build <name> <prompt-file>` | Verified, including `--isolated` created from the snapshot with the prompt on stdin: 5 turns, 56 s wall for a small task, patch applied cleanly. The limit-pause and cap paths are stub-tested only (`loop/boxd.test.sh`). |
| **Parallel Builders** | Run N Builders with no shared disk, ports or caches. | One VM per Builder, started in parallel. | Verified at 4 (small tasks, 21–27 s wall each, no errors). Not tested above 4 or with large tasks. |
| **Linux check run** | Run `just check` on a clean Linux box. CI's macOS job also runs the `rup ping` round trip, which `just check` does not; CI uses Node 22 and the VM Node 24. | `loop/boxd.sh check <pr-number\|branch>`: merges the ref into `origin/main` on the laptop, uploads that tree to an `--isolated` VM from the snapshot and runs `just check` there (the justfile is the single definition of `check`). | Verified 2026-10-01: about 2 minutes wall per PR on a 2-vCPU VM, with the warmed snapshot. Catches platform-neutral breakage before a PR. It is not the macOS gate. |
| **QA screenshots** | Render the web UI where an agent can drive it and nobody's windows get hijacked. | `agent-browser` in the VM, screenshots to `artifacts/ux/<scenario>/<step>.png`, then `boxd machine cp` out. | Pipeline verified with a static page, not with roundup's UI (none exists yet). |
| **Watch an agent live** | Let a human see the VM's screen. | `boxd machine desktop <vm> --json` returns a `desktop_url`. | URL issued on snapshot-based and `--isolated` VMs. Not viewed by a human, so what it shows is unverified. |
| **Sandbox for untrusted code** | Run extension code with no credentials and no in-VM `boxd`. | `boxd machine new --isolated` | Verified that `claude -p` and `agent-browser` work in an isolated VM and that egress is `unrestricted`. Inbound isolation and escape resistance not tested. Not needed until extensions exist. |

The strongest reason to use a VM is the first row: unattended permission-skipping. Parallelism alone is not an argument, because git worktrees already isolate files.

## GitHub on a VM

- An `--isolated` VM has no GitHub login: `gh auth status` says "not logged in" and `git ls-remote` on the private repo asks for a username. `loop/boxd.sh build`, `review` and `check` rely on that: git work (the merge into `origin/main`) happens on the laptop and only the tree is uploaded, so code from a PR (install scripts, `build.rs`, tests) runs where there is no GitHub token to read.
- A non-isolated VM gets `/usr/local/bin/gh`, a boxd wrapper that runs the stock `/usr/bin/gh` with the account's personal OAuth token (`repo` and `user:email`, so read/write on every private repo of the account). Code running there can read it (`gh auth token`, `boxd-github-token` and `git credential fill` all return it) and outbound HTTPS works. Use a non-isolated VM only for code you wrote and reviewed, and for read-only work. The stock `/usr/bin/gh` has no token, so `gh auth setup-git` leaves plain `git` without a credential; a credential helper must point at the wrapper.

## tsc on the VM

A VM restored from the snapshot hangs `pnpm typecheck` (native tsc 7.0.2, Go) until it is rebooted; `start_vm` reboots it. The mechanism, measured: at startup the Go runtime calls `fanotify_init` and closes the fd, and on the restored guest kernel (6.1.0+) that `close` never returns (the process sits in `flush_work`; `dmesg` shows `fsnotify` workers stuck in `synchronize_srcu`), even for `tsc --version`. Denying the call (`systemd-run -p SystemCallFilter=~fanotify_init -p SystemCallErrorNumber=ENOSYS`) also unblocks it.

## What `bake` adds

Beyond the toolchain: the WebKitGTK libraries (`apt-get` must run with `NEEDRESTART_SUSPEND=1`, or needrestart restarts the boxd agent and the exec dies with "lost the connection to the machine"), `CARGO_TARGET_DIR=~/cargo-target` in `~/.cargo/env`, and a warmed `~/cargo-target` and pnpm store from the lockfiles at bake time (clippy and test builds of the dependencies, not the repo). The bake VM is `--isolated`, so dependency build scripts run with no GitHub login. Bake refuses to save if a scan of the VM's home, `/etc`, `/usr/local`, `/root` and `/var/lib` (skipping the Bun cache, whose docs contain placeholder tokens) finds a GitHub or Anthropic token pattern. Rebake when the lockfiles change a lot; a stale warm cache only costs build time.

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

- The Claude token is the boxd secret `CLAUDE_CODE_OAUTH_TOKEN`, host-scoped. Inside a VM the variable holds a placeholder (`bxds_...`); boxd swaps in the real token on requests to `*.anthropic.com`, `*.claude.com` and `claude.ai`, so the real token never reaches the VM. Set `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` for `claude`: without it the process hangs about 90 s after answering (some other host is blocked), with it `claude -p "say hi"` takes 2 s.
- Normal machines expose connected integration credentials to any code running in them. Use `--isolated` when running code you do not trust.

## Cost and quota

- Per run, notional list price: ping $0.09; small Sonnet task $0.08–0.14; Opus one-line review $0.17. On Max this is plan usage, not cash. Each fresh `claude -p` writes about 20k cache tokens, so prefer fewer, larger tasks.
- boxd credit stayed at about €30 across all spikes (29.997 on one reading, 30.00 after rounding).
- The JSON output has no quota field. `loop/boxd.sh` pauses on `api_error_status` 429 or "usage limit" / "rate limit" in the result by writing `loop/out/PAUSED` and exiting 75. Delete the file to resume. This path is covered by a stub test (`loop/boxd.test.sh`) but has never met a real limit error.
- Cap: 4 concurrent Builder VMs, the largest number measured. Raise it only after a run with realistic task sizes shows no limit errors.

## Cleanup

Every VM that `loop/boxd.sh` creates is destroyed by its exit trap, which reports a leaked VM if removal fails. VMs made by hand (QA runs) carry only the auto-destroy timer, so remove them yourself. After any session, `boxd machine list` should show no `ru-` machines. The only standing resource is the `ru-toolchain` snapshot.
