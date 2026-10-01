# boxd use cases

boxd is optional: nothing on the critical path depends on it (merging needs GitHub's macOS `check` and local runs; the `loop` job on ubuntu is advisory until it is made a required check). This file says what a boxd VM adds, what it cannot do, and what was actually measured. "Verified" means observed on 2026-10-01; the commands and raw numbers are in `spikes/boxd/REPORT.md` and `loop/boxd.sh`. Anything else is marked unverified.

## What a VM adds

| Use case | What local cannot do | Recipe | Status |
|---|---|---|---|
| **Unattended Builder** | Run `claude -p --dangerously-skip-permissions` without exposing your laptop's files and credentials. The VM is the sandbox, and `--isolated` removes the connected integrations, the saved coding-agent logins and the in-VM `boxd` CLI (docs.boxd.sh/use-cases/sandboxes). | `loop/boxd.sh build <name> <prompt-file>` | Verified earlier, including `--isolated` created from the snapshot with the prompt on stdin: 5 turns, 56 s wall for a small task, patch applied cleanly. Since 2026-10-01 (snapshot v5, claude 2.1.286) a VM restored from the snapshot hangs without a reboot (see Defaults), and `loop/boxd.sh` still passes `-e` and has no reboot step, so it is out of date until `boxd-agents` changes it. The limit-pause, cap and token-scan paths are stub-tested only (`loop/boxd.test.sh`). |
| **Parallel Builders** | Run N Builders with no shared disk, ports or caches. | One VM per Builder, started in parallel. | Verified at 4 (small tasks, 21–27 s wall each, no errors). Not tested above 4 or with large tasks. |
| **Linux check run** | Run `just check` on a clean Linux box in seconds. CI's macOS job also runs the `rup ping` round trip, which `just check` does not; CI uses Node 22 and the VM Node 24. | `just check` on a VM from the snapshot (the justfile is the single definition of `check`). | Verified: a whole `loop/boxd.sh build` (Builder run plus `just check`) took 29–37 s wall on a fresh VM from the snapshot with Rust 1.89, and 56 s with Rust 1.99 on the current script. An earlier, shorter check plus the `rup ping` round trip took 15 s. All in `spikes/boxd/REPORT.md`. Catches platform-neutral breakage before a PR. It is not the macOS gate. |
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
- **Reboot after every `machine new --from-snapshot`** (`boxd machine reboot <vm>`, about 1 s, then wait about 5 s until `boxd machine exec <vm> -- true` answers). A VM restored from a memory snapshot wedges native code: `claude -p` prints its result but never exits, and `tsc` 7.0.2 hangs, so `boxd machine exec` blocks until its `--timeout` and returns 124. Observed 2026-10-01 on guest kernel 6.1.0+ (the `claude` JITWorker thread sits in D state in `__flush_work`). After a reboot `claude -p "say hi"` took 4 s and `pnpm typecheck` 0.38 s. `machine resize` also reboots, which is why it looked like a vCPU effect. `BUN_JSC_useJIT=0` did not help. There is no kernel or image option in the CLI (`boxd machine new --help`).
- Upload the commit under test with `git archive`, never a recursive copy (that would ship `node_modules/`, `target/` and other agents' state).
- Bring results back as a patch (`git format-patch` then `boxd machine cp`) and push from the laptop. GitHub credentials never go to the VM.

## Secrets

- The Claude token is an org secret, `CLAUDE_CODE_OAUTH_TOKEN`, scope `private`, bound to the hosts `*.anthropic.com`, `*.claude.com` and `claude.ai` (`boxd env list --json`). Create it with `boxd env set CLAUDE_CODE_OAUTH_TOKEN <value> --secret --domains '*.anthropic.com,*.claude.com,claude.ai'`; the value is write-only. The docs say the default scope is `shared`, and every `ru-` VM is private, so move it with `boxd env scope CLAUDE_CODE_OAUTH_TOKEN private` (syntax from `boxd env --help`, which lists `scope`; `env set --help` was not read). Source: docs.boxd.sh/guides/env-secrets.
- A host-bound secret never enters the VM. The VM sees an opaque placeholder (`bxds_...`, 37 characters) and the platform substitutes the real value in outbound requests to the bound hosts only. Observed: on an `--isolated` VM, `boxd machine exec` carried the placeholder and `claude -p "say hi"` returned `is_error: false`.
- Delivery: injected at boot and into login shells; on `--isolated` VMs only into `exec` and SSH sessions (per docs). Scope `private` reaches your private org-billed machines, `shared` shared machines, `all` both.
- So the Builder command takes no token: `boxd machine exec <vm> -- 'claude -p ... </dev/null'`. Do not pass `-e CLAUDE_CODE_OAUTH_TOKEN=...` (it would replace the placeholder with the real value in the VM and in `ps`), and keep stdin redirected. The token-leak scans in `loop/boxd.sh` guard a value the VM no longer holds.
- A placeholder only works against its bound hosts. Another tool in the VM that needs the token at a different host gets a useless `bxds_` string.
- An egress allowlist (`boxd machine egress <vm> host,host`) always admits the hosts of the secrets the VM holds. With only `github.com,crates.io,index.crates.io,static.crates.io,registry.npmjs.org,static.rust-lang.org` allowed, `claude -p` still answered; a `just check` under the same list is reported in the PR body. `api.github.com`, `statsig.anthropic.com`, `sentry.io` and `example.com` were unreachable under it, and `claude -p` did not need them, so `api.anthropic.com` is the only host headless Claude Code evidently needs. Interactive use (`--tty`, `boxd connect`) was not tested. Snapshots start unrestricted and forks inherit the source's list.
- GitHub: see the next section. Normal machines expose connected integration credentials to any code running in them; `--isolated` VMs have none.

## GitHub from a VM

- `--isolated` VMs have no GitHub access, no `run` CLI and no `gh` login. Only a host-bound secret could give them one; that is untested (`boxd env set` was not run for the probe).
- A normal VM gets three credentials from the connected integrations (`boxd manage integrations list`): the personal OAuth connection (`git` and `gh`, scopes `repo` and `user:email`, so every repository of the account, and `gh api repos/saadshahd/roundup` reported `admin: true`); `run github-app get-token` (an installation token, `ghs_` prefix, minted on demand; the JSON holds only the token, no permissions or installation id, so installation 166976072 is unconfirmed; the installation listed 50 repositories including `saadshahd/roundup`; creating then deleting a throwaway branch on the real `roundup` repo returned 201 and 204; creating then deleting a pending review on PR 65 worked as `boxd-app[bot]`, status 200); and the `github` toolkit.
- boxd offers no way to narrow them: one App installation per org, OAuth scope fixed at `repo`. Narrowing to `roundup` is done on GitHub (installation 166976072: select repositories, minimum permissions Contents and Pull requests write, Metadata read), or with a fine-grained token as a host-bound secret. The `.permissions` field of the installation's repository list showed all false while writes succeeded, so it is not the App's grant. The App writes as its own identity, `boxd-app[bot]`; the OAuth connection acts as the account owner, whom GitHub will not let approve their own PRs, so Reviewer approvals have to come from the App.
- Per AGENTS.md, results still come back as a patch and are pushed from the laptop; the above says what a normal VM could do, not what it should.

## Cost and quota

- Per run, notional list price: ping $0.09; small Sonnet task $0.08–0.14; Opus one-line review $0.17. On Max this is plan usage, not cash. Each fresh `claude -p` writes about 20k cache tokens, so prefer fewer, larger tasks.
- boxd credit stayed at about €30 across all spikes (29.997 on one reading, 30.00 after rounding; 29.91 on 2026-10-01 after the secret probes). Rate card (boxd.sh/pricing): €0.049 per vCPU-hour while running, €0.015 per GiB-hour of memory in use, €0.0001 per GiB-hour written to disk; €30 credit with a payment method, auto top-up €20.
- Limits (`boxd manage billing`, docs.boxd.sh/guides/resources): 50 concurrent VMs per org (2 without a payment method; forks, snapshots-created and suspended VMs count), sizes 1 vCPU/4 GiB, 2/8 and 4/16 (quota maximum 4/16), 100 GB disk, 10 checkpoints per VM, 25 API keys. `--vcpu` and `--memory` apply to fresh VMs only; a snapshot restore keeps the snapshot's size, and `boxd machine resize` reboots. Limits can be raised through support. A creation rate limit is not documented.
- The JSON output has no quota field. `loop/boxd.sh` pauses on `api_error_status` 429 or "usage limit" / "rate limit" in the result by writing `loop/out/PAUSED` and exiting 75. Delete the file to resume. This path is covered by a stub test (`loop/boxd.test.sh`) but has never met a real limit error.
- Cap: 4 concurrent Builder VMs is our own limit, the largest number measured; boxd allows 50. Raise it only after a run with realistic task sizes shows no limit errors.

## Cleanup

Every VM that `loop/boxd.sh` creates is destroyed by its exit trap, which reports a leaked VM if removal fails. VMs made by hand (QA runs) carry only the auto-destroy timer, so remove them yourself. After any session, `boxd machine list` should show no `ru-` machines. The only standing resource is the `ru-toolchain` snapshot.
