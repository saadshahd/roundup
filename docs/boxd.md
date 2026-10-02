# boxd use cases

boxd is optional: nothing on the critical path depends on it (merging needs GitHub's macOS `check` and local runs; the `loop` job on ubuntu is advisory until it is made a required check). This file says what a boxd VM adds, what it cannot do, and what was actually measured. "Verified" means observed on 2026-10-01; the commands and raw numbers are in `spikes/boxd/REPORT.md` and `loop/boxd.sh`. Anything else is marked unverified.

## What a VM adds

| Use case | What local cannot do | Recipe | Status |
|---|---|---|---|
| **Unattended Builder** | Run `claude -p --dangerously-skip-permissions` without exposing your laptop's files and credentials. The VM is the sandbox, and `--isolated` removes the connected integrations, the saved coding-agent logins and the in-VM `boxd` CLI (docs.boxd.sh/use-cases/sandboxes). | `loop/boxd.sh build <name> <prompt-file>` | Verified, including `--isolated` created from the snapshot with the prompt on stdin: 5 turns, 56 s wall for a small task, patch applied cleanly. The limit-pause and cap paths are stub-tested only (`loop/boxd.test.sh`). |
| **Parallel Builders** | Run N Builders with no shared disk, ports or caches. | One VM per Builder, started in parallel. | Verified at 4 (small tasks, 21–27 s wall each, no errors). Not tested above 4 or with large tasks. |
| **Linux check run** | Run `just check` on a clean Linux box. CI's macOS job also runs the `rup ping` round trip, which `just check` does not; CI uses Node 22 and the VM Node 24. | `loop/boxd.sh check <pr-number\|branch>`: merges the ref into `origin/main` on the laptop, uploads that tree to an `--isolated` VM from the snapshot and runs `just check` there (the justfile is the single definition of `check`). | Verified 2026-10-01: about 2 minutes wall per PR on a 2-vCPU VM, with the warmed snapshot. Catches platform-neutral breakage before a PR. It is not the macOS gate. |
| **QA screenshots** | Render the web UI where an agent can drive it and nobody's windows get hijacked. | `agent-browser` in the VM, screenshots to `artifacts/ux/<scenario>/<step>.png`, then `boxd machine cp` out. | Pipeline verified with a static page, not with roundup's UI (none exists yet). |
| **Watch an agent live** | Let a human see the VM's screen. | `boxd machine desktop <vm> --json` returns a `desktop_url`. | URL issued on snapshot-based and `--isolated` VMs. Not viewed by a human, so what it shows is unverified. |
| **Sandbox for untrusted code** | Run extension code with no credentials and no in-VM `boxd`. | `boxd machine new --isolated` | Verified that `claude -p` and `agent-browser` work in an isolated VM and that egress is `unrestricted`. Inbound isolation and escape resistance not tested. Not needed until extensions exist. |

The strongest reason to use a VM is the first row: unattended permission-skipping. Parallelism alone is not an argument, because git worktrees already isolate files.

## GitHub on a VM

- An `--isolated` VM has no GitHub login: `gh auth status` says "not logged in" and `git ls-remote` on the private repo asks for a username. `loop/boxd.sh build`, `review` and `check` rely on that: git work (the merge into `origin/main`) happens on the laptop and only the tree is uploaded, so code from a PR (install scripts, `build.rs`, tests) runs where there is no GitHub token to read.
- A non-isolated VM gets `/usr/local/bin/gh`, a boxd wrapper that runs the stock `/usr/bin/gh` with the account's personal OAuth token (`repo` and `user:email`, so read/write on every private repo of the account). Code running there can read it (`gh auth token`, `boxd-github-token` and `git credential fill` all return it) and outbound HTTPS works. Use a non-isolated VM only for code you wrote and reviewed, and for read-only work. The stock `/usr/bin/gh` has no token, so `gh auth setup-git` leaves plain `git` without a credential; a credential helper must point at the wrapper.
- A non-isolated VM can also run `run github-app get-token`, which mints a one-hour App installation token (`ghs_`). Probed 2026-10-01: it could create and delete a branch on `roundup` and create and delete a pending review on a PR, as `boxd-app[bot]`. The installation covered 50 repositories including `roundup`, and the token reply has no permissions or installation id, so installation 166976072 is unconfirmed. boxd cannot narrow either credential (one App installation per org; OAuth scope fixed at `repo`); narrowing to `roundup` is done on GitHub (selected repositories; Contents and Pull requests write, Metadata read). The App acts as `boxd-app[bot]`, the OAuth login as the account owner, whom GitHub will not let approve their own PRs, so a Reviewer approval has to come from the App.

## Where a Reviewer's verdict goes

A boxd Reviewer cannot post to GitHub: its VM has no login, by design (`GitHub on a VM`). `loop/boxd.sh review` writes its answer to `loop/out/verdicts/<name>.md` on the laptop, and that file is lost when `loop/out/` is cleaned, so an approval that leaves only an empty `Reviewed-by-Agent` commit leaves no record of what was checked. The record is a PR comment, and it comes before the approval commit. A Reviewer that has `gh` (one that runs on the laptop) posts its own verdict and then pushes the approval commit. For a boxd Reviewer the Driver, who launched the review on the laptop, posts the verdict and then pushes the approval commit; the VM does neither. The comment is posted with:

```
gh pr comment <pr> --body-file loop/out/verdicts/<name>.md
```

The verdict's first line is `VERDICT: approve` or `VERDICT: reject`; it names the full SHA of the commit it reviewed (the approval commit's parent; for a reject, the head it reviewed, since no approval commit exists), the commands it ran with their exit codes, the mutations it tried and their results, and each defect with its rule number. The Merger confirms by hand that the PR holds an approving verdict whose full SHA equals `git rev-parse <newest Reviewed-by-Agent commit>^`, as it already confirms that no merge follows the approval (`loop/rules.sh trailers` reads `--no-merges`). Nothing machine-checks the comment yet; a `--post <pr>` option on `review` would need its own scenario.

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

- Name prefix `ru-`, so a sweep can find them: `boxd machine list | grep '^ru-'`. `loop/boxd.sh build` refuses to start when `BOXD_MAX_VMS` (default 12) `ru-` VMs exist.
- `--auto-destroy-timeout 4200` (leak guard, longer than reboot, upload, an agent run of up to 1800 s and the 1800 s check together; `bake` uses 7200) and `--auto-suspend-timeout 0`, because CPU-only builds look idle and would be suspended mid-run. `BOXD_AGENT_TIMEOUT` (seconds, at most 1800) shortens the agent run. Both settings confirmed with `boxd machine get`. Auto-hibernate defaults to 14400 s of no network traffic; a job longer than 4 hours needs it set to 0.
- Create from the `ru-toolchain` snapshot (Rust from `rust-toolchain.toml`, clippy, rustfmt, nextest, cargo-machete, just, pnpm from `package.json`, Node 24). Boot reports 4–5 ms; `machine new` takes about 2.2 s wall. Desktop URLs and `agent-browser` work on snapshot-based machines. Rebake with `loop/boxd.sh bake` when `rust-toolchain.toml` or the pnpm pin changes. The snapshot is about 11.5 GB and is kept; `loop/boxd.sh bake` replaces it.
- A VM made from the snapshot is rebooted before use: `start_vm` does it (`boxd machine reboot`, then wait until `boxd machine exec <vm> -- true` answers); do the same for a VM made by hand (see "tsc on the VM"). Without the reboot `claude -p` printed its answer but `boxd machine exec` blocked until its timeout (exit 124); on 2026-10-01 a reboot fixed it on three fresh VMs, and `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` (see Secrets) is the other fix `loop/boxd.sh` uses. `machine resize` also reboots. There is no kernel or image option in `boxd machine new --help`.
- Upload the commit under test with `git archive`, never a recursive copy (that would ship `node_modules/`, `target/` and other agents' state).
- Bring results back as a patch (`git format-patch` then `boxd machine cp`) and push from the laptop. GitHub credentials never go to the VM.

## Secrets

- The Claude token is the boxd secret `CLAUDE_CODE_OAUTH_TOKEN`, host-scoped. Inside a VM the variable holds a placeholder (`bxds_...`); boxd swaps in the real token on requests to `*.anthropic.com`, `*.claude.com` and `claude.ai`, so the real token never reaches the VM. Set `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` for `claude`: without it the process hangs about 90 s after answering (some other host is blocked), with it `claude -p "say hi"` takes 2 s.
- The secret is account-wide by design for this private, personal repo (no outside contributors): every VM, including `--isolated` ones, gets the `bxds_` placeholder. It is substituted only for `*.anthropic.com`, `*.claude.com` and `claude.ai`, so exposure is quota use, not credential theft. If the repo ever runs code outside our own PRs, `check` and `bake` VMs must stop receiving it.
- Delivery (docs.boxd.sh/guides/env-secrets): injected at boot and into login shells; on `--isolated` VMs only into `exec` and SSH sessions. Observed: an `--isolated` VM's `boxd machine exec` carried the placeholder (37 characters) and `claude -p "say hi"` returned `is_error: false`. Passing `-e CLAUDE_CODE_OAUTH_TOKEN=...` would replace the placeholder with a real value inside the VM, so never do it.
- A placeholder only works against its bound hosts; another tool in the VM that needs the token elsewhere gets a useless `bxds_` string.
- An egress allowlist (`boxd machine egress <vm> host,host`) always admits the hosts of the secrets the VM holds. On a rebooted `--isolated` VM limited to `github.com,crates.io,index.crates.io,static.crates.io,registry.npmjs.org,static.rust-lang.org`, `pnpm install`, `claude -p` and the Rust build and tests ran (295 of 296 passed; the failure was the x8 scenario, not the network). `api.github.com`, `statsig.anthropic.com`, `sentry.io` and `example.com` were unreachable under it, so `api.anthropic.com` is the only host headless `claude -p` evidently needs. Interactive use (`--tty`, `boxd connect`) was not tested. Snapshots start unrestricted and forks inherit the source's list.

## Cost and quota

- Per run, notional list price: ping $0.09; small Sonnet task $0.08–0.14; Opus one-line review $0.17. On Max this is plan usage, not cash. Each fresh `claude -p` writes about 20k cache tokens, so prefer fewer, larger tasks.
- boxd credit stayed at about €30 across all spikes (29.997 on one reading, 30.00 after rounding; 29.91 on 2026-10-01 after the secret probes). Rate card (boxd.sh/pricing): €0.049 per vCPU-hour while running, €0.015 per GiB-hour of memory in use, €0.0001 per GiB-hour written to disk; €30 credit with a payment method, auto top-up €20.
- Sizes (`boxd manage billing`, docs.boxd.sh/guides/resources): 1 vCPU/4 GiB, 2/8 (default) and 4/16 (the quota maximum), 100 GB disk, 10 checkpoints per VM. `--vcpu` and `--memory` apply to fresh VMs only; a snapshot restore keeps the snapshot's size. A creation rate limit is not documented.
- The JSON output has no quota field. `loop/boxd.sh` pauses on `api_error_status` 429 or "usage limit" / "rate limit" in the result by writing `loop/out/PAUSED` and exiting 75. Delete the file to resume. This path is covered by a stub test (`loop/boxd.test.sh`) but has never met a real limit error.
- Cap: `BOXD_MAX_VMS` concurrent `ru-` VMs, default 12. Measured only up to 4; the account's own machine limit has not been measured. Watch for limit errors (`loop/out/PAUSED`) when raising it.

## Cleanup

Every VM that `loop/boxd.sh` creates is destroyed by its exit trap, which reports a leaked VM if removal fails. VMs made by hand (QA runs) carry only the auto-destroy timer, so remove them yourself. After any session, `boxd machine list` should show no `ru-` machines. The only standing resource is the `ru-toolchain` snapshot.
