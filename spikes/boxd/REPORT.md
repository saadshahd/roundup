# boxd spike (2026-10-01)

Superseded in part by `docs/boxd.md`, which holds the current use cases and recipes. This file keeps the raw measurements and corrects what the first version got wrong.

## Corrections to the earlier version

- **Wrong:** an earlier note in this repo dropped boxd on the grounds that e2e "needs the Tauri window, which a Linux VM can't give" and that boxd has no desktop. boxd has a live **Desktop** (`boxd machine desktop <vm>`, a browser-viewable graphical display) and a preinstalled `agent-browser`. A VM can run the web UI and take screenshots. It still cannot run the macOS WKWebView or produce macOS perf numbers.
- **Wrong:** the quota section recommended a cap of "3 to 4" from a single sequential run. The cap is now 4 because four parallel Builders were run and succeeded (see below). Larger tasks are untested.
- **Misleading:** the first quota attempt failed because I reassembled the token from a wrapped terminal capture. It was not a boxd or token-policy failure. It is removed from this report.
- **Incomplete:** the first report said the Max token "works unattended" from ping and small tasks only. It had not checked whether the token lands on the VM's disk, whether `--isolated` VMs work, or what happens at parallelism. Those are measured below.

## VM mechanics

| Measure | Result |
|---|---|
| Default VM | 2 vCPU / 8G, x86_64, Ubuntu 24.04; node 24, claude 2.1.283, `agent-browser` 0.34.0 preinstalled; no Rust |
| Fresh VM | boot reported 8 ms, about 1.0 s wall; first exec about 0.9 s |
| Builder + `just check` on a snapshot VM | 29–37 s wall with Rust 1.89; 56 s with Rust 1.99 and the larger workspace (`loop/boxd.sh build`) |
| From snapshot | boot reported 4–5 ms; about 2.2 s wall; toolchain, Desktop URL and `agent-browser` all present |
| Fork of a running VM | reported 222 ms, about 2.4 s wall |
| Exit codes | propagate through `boxd machine exec` |
| Rust via rustup (minimal profile) | about 12 s; `exec` runs `sh -c`, so source `~/.cargo/env` |
| `--isolated` | boots about 1.5 s; egress is `unrestricted`; `claude -p` and `agent-browser` work; inbound isolation untested |
| Snapshot `ru-toolchain` | about 11.5 GB; 2.5 min to bake with Rust, nextest, cargo-machete, just, pnpm |
| Credit | about EUR 30 before and after everything (29.997 on one reading, 30.00 after rounding) |

## Builder runs (Max token passed per call with `exec -e`)

| Run | Turns | Wall | Notional cost |
|---|---|---|---|
| Ping, laptop / VM | 1 | 1.7 s / 1.5 s | $0.105 / $0.092 |
| Sonnet: add a unit test and run the check | 3–8 | 13–19 s | $0.08–0.14 |
| Opus: one-line review | 2 | 5.2 s | $0.170 |
| 4 Sonnet Builders in parallel, distinct tasks | 3–6 each | 21–27 s wall each (model time 19–25 s) | $0.08–0.11 each |

No 401, 429 or terms error appeared in any run. The JSON has no quota field. `sonnet` and `opus` resolve to claude-sonnet-5-5 and claude-opus-5-5.

## Secrets

After a Builder run, a search of the VM's home, `/tmp` and `/etc` found no copy of the token, and `~/.claude/.credentials.json` did not exist. `boxd env list` was empty. The first, failed attempt did use `boxd env set --secret` (account-wide) and removed it afterwards; every later run passed the token per call with `exec -e`.

## QA pipeline proof

On an `--isolated` VM, `agent-browser` rendered a static page and saved a PNG that copied out with `boxd machine cp`. Notes: the default viewport is 2545x2739, so set one (`agent-browser set viewport W H`); a page without `<meta charset=utf-8>` shows mojibake; the glyphs `● ✕ ⏸ ○ · ✓ ◈ ◇ ▾ ›` all render, but `⏸` and `✕` use fallback glyphs, so Linux screenshots cannot be diffed against macOS baselines.

## Still unmeasured

- Parallelism above 4, or with large tasks, and what a real limit error looks like (the pause path in `loop/boxd.sh` is untested against one).
- What the Desktop view shows (URLs were issued, nobody opened them).
- Whether the token is visible to other processes in the VM during a call.
- Inbound isolation and escape resistance of `--isolated`.
