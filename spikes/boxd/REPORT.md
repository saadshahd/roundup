# boxd spike: VM mechanics (2026-10-01)

Three machines created and destroyed; `boxd machine list` showed 0 afterwards. No Claude token was used.

| Measure | Result |
|---|---|
| Default VM | 2 vCPU / 8G, x86_64, Ubuntu 24.04; node v24.21.0 and claude 2.1.283 preinstalled |
| Boot | reported 8 ms, about 1.0 s wall |
| First exec | about 0.9 s wall; each exec carries similar CLI overhead |
| Fork of a running VM | reported 222 ms, about 2.4 s wall; inherits state and size |
| `--isolated` | boots in about 1.5 s; still has outbound internet; no in-VM `boxd` CLI; inbound from other machines not tested |
| Exit codes | propagate (42 -> 42, fork 3 -> 3, missing binary 127) |
| File copy in and out | round-trips |
| Rust | not preinstalled; rustup minimal profile takes about 12 s (rustc 1.99.0); `exec` runs `sh -c`, so source `~/.cargo/env` or use absolute paths |
| Credit | 30.00 EUR before and after; per-hour cost unmeasured (billing may be lazy) |

Implication: for Linux CI-parity runs, install Rust once, snapshot, then fork per run.

## Unmeasured

- Claude quota burn. Needs `claude setup-token`, which only the user can generate.
- cargo build and test time on 2 vCPU.
- Fork from a snapshot with Rust preinstalled.
- Behaviour above 3 concurrent VMs.

## Quota-burn attempt 1 (blocked: corrupted token)

The token I extracted from a PTY capture of `claude setup-token` was rejected: `Failed to authenticate. API Error: 401 OAuth access token is invalid.` It failed the same way on the laptop, so boxd is not the cause. The extracted string was 128 characters; the terminal had re-rendered and wrapped the output, so it was most likely corrupted when I reassembled it. Nothing past authentication ran, so burn, rate limits and automation-terms behaviour are still unmeasured.

Findings that do hold:

- `boxd env set NAME value --secret` injects intact and lists as `(sealed)`.
- Those secrets are account-wide, so every machine would receive the token. For concurrent Builders, pass it per machine (`exec -e` or `env push`).
- The VM and secret were removed afterwards. Credit stayed at about 29.997 EUR.

To redo: generate the token in a real terminal, copy it, and save it with `pbpaste > ~/.roundup-spike-token; chmod 600 ~/.roundup-spike-token`.

## Quota-burn attempt 2 (measured, 2026-10-01)

A fresh `claude setup-token` token, passed per machine with `exec -e` (never stored on the VM, no account-wide secret). The VM was 2 vCPU / 8G with claude 2.1.283. No 401s, rate limits or automation-terms errors appeared.

| Run | Turns | Duration | In / cache-create / cache-read / out tokens | `total_cost_usd` (list-price notional) |
|---|---|---|---|---|
| Ping, laptop | 1 | 1.7 s | 2 / 26,245 / 0 / 4 | 0.105 |
| Ping, VM | 1 | 1.5 s | 2 / 22,038 / 18,639 / 4 | 0.092 |
| Sonnet: Rust fn + test + `cargo test` (6 pass) | 3 | 12.9 s | 6 / 19,617 / 68,138 / 896 | 0.101 |
| Opus: one-line review of the diff | 2 | 5.2 s | 4 / 19,558 / 38,998 / 266 | 0.170 |

- `sonnet` and `opus` resolve to claude-sonnet-5-5 and claude-opus-5-5. Rustup minimal plus cargo took about 9 s.
- Each fresh `claude -p` writes about 20k cache-creation tokens, so even a trivial call carries that overhead.
- On Max the cost field is plan usage, not cash. The JSON exposes no plan-limit or remaining-quota field, so quota must be tracked outside claude.
- All VMs share one token and one Max usage window; that, not boxd, limits concurrency. Opus is about 1.7x Sonnet per run here.
- VM destroyed; `boxd machine list` and `boxd env list` empty. Credit 29.997 EUR before and after.

Not tested: concurrent runs. Cap: 3 to 4 Builder VMs, Sonnet by default, Opus for review only, with a circuit breaker that pauses every run on the first 429 or usage-limit error. Run a 4-way parallel test before raising the cap.
