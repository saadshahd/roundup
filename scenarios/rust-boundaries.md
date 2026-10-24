# Rust crate interfaces and errors

Source: #3798. Ids: RB3798a–RB3798c. This specification covers hand-written Rust in the workspace members under `crates/`; vendored dependencies and generated files are excluded. It implements the Rust portion of the owner-approved C2 and C8 direction and C1's file cap. The specification changes no runtime behaviour.

The existing crate interface is the test seam. Splitting implementation creates private responsibility modules, not new caller-facing abstractions. Rust error types may change only in the error slice; the RPC contract, persisted data and observable success/refusal behaviour remain the same.

**RB3798a Crate roots expose their interface.**

Given a workspace crate, when its interface slice lands, then:

- **Root (policy).** `src/lib.rs`, where present, contains module declarations, imports/re-exports, public types, trait declarations and interface implementations only. Storage queries, orchestration, parsing, process management and test bodies live in responsibility modules. A root function or method body occupies at most 20 physical lines between its opening and closing braces, counting comments and blank lines; a same-line body counts as one. Review checks each body, including nested functions. Short forwarding methods may stay, but a 20-line limit does not authorize implementation logic in the root.
- **Size (policy).** Every hand-written `.rs` file in the sliced crate, including integration tests and shared test setup, has fewer than 1000 lines (`wc -l` per file). Split by the concept that changes, extending existing modules first; no numbered parts, overflow helpers, `include!` indirection, raised cap or skipped check. This includes `agents`' `worktree_wiring` and all other oversized files in a touched crate, not only the four reported hubs.
- **Moves (policy).** A layout-only slice preserves public Rust paths, visibility, types, registered RPC methods, persistence and behaviour. Inspect `git diff --color-moved` and compare `cargo doc -p <crate> --no-deps` public items with its base. Existing tests move with their concept and keep their names and assertions; compare `cargo nextest list -p <crate>` before/after, ignoring test-binary/module relocation only. Do not add tests that reread source layout.
- **Regression (policy).** Run the crate's existing tests with `cargo nextest run -p <crate>` and compile every caller. Tests keep asserting the same successes, error codes, identities and saved state. Required checks include `just clippy` on Linux and the existing macOS clippy job on the same PR head. No code slice lands on a red platform gate.

**RB3798b Production code rejects panicking operations.**

Given the workspace's existing inherited Clippy configuration, when panic-removal slices and the final enforcement slice land, then:

- **Enforcement (policy).** `[workspace.lints.clippy]` explicitly sets `unwrap_used`, `expect_used`, `panic` and `indexing_slicing` to `deny`, alongside the existing `all` deny. Each workspace crate inherits these lints. `just clippy` continues checking all targets with warnings denied, on Linux and macOS.
- **Test allowance (policy).** Test-only code may allow those four lints using `#![cfg_attr(test, allow(...))]`, a `#[cfg(test)]` module allowance, or Clippy's four `allow-*-in-tests` settings. An integration-test crate may use its test-only allowance; a production helper shared with tests may not. No production `allow`/`expect` attribute or command-line cap suppresses these lints. A crate-root `cfg_attr(test, ...)` must still leave its normal library build checked.
- **Repair (policy).** Remove production unwraps, expects and explicit panic sites rather than relabelling them as `unreachable!`, assertions, aborts or unchecked access. Replace indexing/slicing flagged by Clippy with checked access, iteration or a representation carrying the required invariant. Enumerable failures return the crate's error type; recovery must preserve existing refusals, saved data and operation ordering. Do not introduce empty defaults, success after failure, poisoned-lock recovery that discards state, or catch-and-continue for an unenumerated failure. Existing tests assert behaviour; new tests are justified only for a newly exposed behavioural branch and use the `rb3798b_` prefix.
- **Staging (policy).** Remove sites one crate at a time and run the four lints explicitly for that crate with production targets enabled. Enable the workspace denies only after every member is clean, without temporary production opt-outs. Record diagnostics by lint and crate and command exit status; a partial build or an uncompiled macOS path is not a zero count. These four lints enforce the named operations, not a proof that dependencies, allocation or every Rust operation can never panic.

**RB3798c Crate errors remain typed until the RPC edge.**

Given a fallible crate interface, when its error slice lands, then:

- **Typed interface (policy).** Each crate with fallible operations exposes one error enum deriving `thiserror::Error` as its crate error. Public fallible operations use it rather than `String`, a boxed error or `RpcError` as their local failure type. Implementations of externally defined traits retain the error type required by that trait; their local fallible operations still use the crate error. Review return types, type aliases, trait methods, associated future outputs and callback bounds, resolving aliases and multiline/nested generics. No externally reachable `pub` signature carries `Result<_, String>` as a return or nested error result. Private and `pub(crate)` helpers are not public signatures; tests implementing a public trait change with that trait. Crates without fallible operations need no unused error enum.
- **Edge (policy).** Convert a local error to the existing `rpc::RpcError` once, where a Daemon RPC handler/registration closure returns a wire reply. Existing registration closures in library crates are part of that edge; no new RPC route or parallel dispatcher is needed. Inter-crate calls retain typed errors and their sources until that point. The RPC transport's existing wire `RpcError` representation remains its wire contract, not a second local error enum; transport/IPC handler interfaces that already transmit wire replies retain it. No string matching recovers variants after conversion.
- **Compatibility (policy).** Preserve the existing RPC code and user-visible failure meaning for every migrated failure; retain source errors where present. Do not collapse not-found, forbidden, conflict, busy or unknown-outcome errors into internal errors, and do not turn unknown outcome into permission to retry a mutation. Each error slice inventories the existing failure paths and names the existing test asserting each mapping. Add an `rb3798c_` behaviour test through the existing public crate/Daemon seam only for a mapping no existing test asserts, checking both the wire code and unchanged effects. A source/configuration-text assertion is not this observer.
- **Atomicity (policy).** Every changed error type updates all direct and transitive callers, trait implementations and test fixtures in the same PR. Compile the workspace's targets, run the owning crate's tests and the tests of affected callers. Update glossary names in the same contract slice under rule 4; regenerate affected contracts from `crates/contracts`, never edit generated files by hand. No intermediate commit offered for merge leaves a caller broken.

## Slice and observation policy

Implementation work lives in GitHub Issues and native blocked-by dependencies, never a Work table here. Start with interface moves in `messages`, `todos` and `agents`, then remove panics and migrate errors per crate; the final workspace-lint Issue depends on all crate repairs. Each PR owns one crate directory, about 2000 changed lines. Larger crate work can land through several green PRs; an Issue closes only once every clause it owns holds. Atomic caller/manifest/glossary edits belong to the owning contract slice, not an unrelated second refactor. If that exceeds scope, file a scoped prerequisite rather than accepting a broken caller or broadening ownership silently.

For each implementation PR, record commands and exit statuses, one row per failed command with its diagnostic; never summarize a failed probe as a pass. Linux checks run locally/CI; `.github/workflows/check.yml`'s `mac-clippy` job supplies the macOS observation, with its run URL and head SHA. This work requires no user-only Mac observation. Existing performance budgets in `docs/perf.md` and AGENTS rule 7 still apply; no speed improvement is claimed from line counts alone.

Applicable principle gates are P1 (Message writes), P2 (worktree isolation), P4 (Adapter ownership), P5 (landing), P6 (item Home), P7 (Message retention), P9–P10 (launch and delivery turns), and P11–P13 (draft, deliverable and reaction continuity) when a slice reaches those paths. Their observers are the existing scenario tests for those paths; refactoring never weakens their acceptance. A Message implementation slice names the active M1, M4–M7, M9 and M10 obligations in `scenarios/proofs.md` V3, preserves the retired status of M2, M3 and M8, runs `just proofs`, and keeps `docs/messages.md`, the model and Rust agreeing.

## Checks on main

These locate existing seams and observers, not completed RB3798 acceptance.

1. `crates/messages/src/lib.rs`: `Messages::open`, `Messages::open_with` and `b1_a_rejected_call_changes_nothing` hold the Message interface and refusal-effect observer.
2. `crates/todos/src/lib.rs`: `Todos::open` and `t17_stale_expected_change_conflicts` hold the Todo interface and conflict observer.
3. `crates/agents/src/worktree_wiring.rs`: `mod tests` holds the worktree wiring's current unit observers; `crates/agents/src/lib.rs` declares the `worktree_wiring` module.
4. `crates/rpc/src/error.rs`: `RpcError`, `impl RpcError` and `code` hold wire error identity and code constructors.
5. `Cargo.toml`: `[workspace.lints.clippy]` and `[workspace.dependencies]` hold inherited lints and the pinned-major `thiserror` dependency.
6. `justfile`: `clippy` runs all-target linting; `.github/workflows/check.yml`: `mac-clippy` runs it on macOS.
7. `scenarios/proofs.md`: V3 and V7 hold the Message law and review obligations; `justfile`: `proofs` is their existing observer.
8. `.claude/sound/placement/file-stays-under-a-size-cap.md` holds the file-cap rule; `scenarios/rupd-modules.md`: RM1 holds the existing Daemon move/behaviour requirements.
