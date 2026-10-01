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
