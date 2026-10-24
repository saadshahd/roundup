# Rust boundary baseline

Specification evidence for #3798, measured on `origin/main` at `a284e11af9d48e9fad4f83fba29bfc757019e3e3` on Linux, with Rust 1.99.0 and Clippy 0.1.99 (`b940084d7`). These are baseline defects for implementation, not passes of RB3798a–RB3798c. Implementation is #3827, natively blocked by #3798.

## Crate roots

`wc -l` reproduces the reported sizes:

| File | Lines |
|---|---:|
| `crates/messages/src/lib.rs` | 2746 |
| `crates/todos/src/lib.rs` | 1943 |
| `crates/agents/src/lib.rs` | 797 |
| `crates/agents/src/worktree_wiring.rs` | 1724 |

## Clippy diagnostics

Both probes completed with exit 0. Warnings were explicitly enabled to enumerate every diagnostic without stopping on the first crate; the acceptance observer will deny them.

```sh
cargo clippy --workspace --all-targets --message-format=json -- \
  -W clippy::unwrap_used -W clippy::expect_used \
  -W clippy::panic -W clippy::indexing_slicing
cargo clippy --workspace --lib --bins --message-format=json -- \
  -W clippy::unwrap_used -W clippy::expect_used \
  -W clippy::panic -W clippy::indexing_slicing
```

Count `compiler-message` records whose diagnostic code is one of those four and whose primary span starts with `crates/`. Deduplicate by `(code, file_name, byte_start, byte_end)` so lib/test-target repetition counts a site once. All targets produce **16,522** unique diagnostics, including tests. The production (`--lib --bins`, without test targets) probe produces **432**:

| Crate | unwrap_used | expect_used | panic | indexing_slicing | Total |
|---|---:|---:|---:|---:|---:|
| agents | 1 | 87 | 4 | 98 | 190 |
| checkpoints | 0 | 7 | 0 | 0 | 7 |
| contracts | 0 | 0 | 0 | 0 | 0 |
| desktop | 0 | 16 | 0 | 1 | 17 |
| messages | 0 | 24 | 1 | 8 | 33 |
| pads | 0 | 2 | 0 | 7 | 9 |
| perf | 0 | 6 | 0 | 30 | 36 |
| provenance | 0 | 0 | 0 | 0 | 0 |
| rpc | 0 | 1 | 0 | 5 | 6 |
| rup | 0 | 3 | 0 | 19 | 22 |
| rupd | 20 | 38 | 3 | 24 | 85 |
| terminal | 0 | 18 | 0 | 6 | 24 |
| todos | 0 | 3 | 0 | 0 | 3 |
| **Total** | **21** | **205** | **8** | **198** | **432** |

This is a count of diagnostics at primary spans, not a grep count of panic expressions or proof of panic freedom. Mac-only code was not compiled by these Linux probes; implementation PRs require the existing `mac-clippy` CI observer too. No macOS zero count is claimed.

## String error results

The Issue's single-line pattern `Result<[^>]*, *String>` matches **49** lines at this revision, not 46. Three are test helpers:

- `crates/perf/tests/r16.rs`: `select_in`.
- `crates/rup/tests/attachment.rs`: `save`.
- `crates/rup/tests/e2e/daemon.rs`: `wait_until_idle`.

Two production matches are false positives: `crates/agents/src/measurement.rs` and `crates/agents/src/claude_code/measurement.rs` return a `BTreeMap<String, String>` inside a result whose error is `RpcError`. Thus this search finds **44 production String-error occurrences and three test occurrences** after those two exclusions. Occurrences include private helpers, a local binding, a callback bound and a future output; they are not 44 public signatures. Nested generics can also hide true String errors from this pattern (for example the `PlacementCall::call` trait result). Acceptance requires inspection of resolved public interfaces, not this regex.

`Cargo.toml` already declares `thiserror = "2"`; only `crates/rpc/Cargo.toml` consumes it. Its wire `RpcError` is a struct deriving `thiserror::Error`, not a local crate error enum. The workspace Clippy section currently denies `all` only, and members inherit it.

Clippy documents the test-only configuration switches in [Lint Configuration](https://doc.rust-lang.org/stable/clippy/lint_configuration.html). Test allowances must not suppress production diagnostics.
