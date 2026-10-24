# Lint-suite specification evidence

Source: #3797. Specification: `scenarios/lint-suite.md`, T3797a. Implementation remains in #3819; this record demonstrates the specification's starting observations, not implementation completion.

Observed revision: `a284e11af9d48e9fad4f83fba29bfc757019e3e3` (`origin/main`). Re-inventory the implementation base after intervening merges. No dependency or lint behavior changes in this specification PR.

## Cycle detector selection

Installed fallow 3.34.0, through `pnpm exec fallow architecture`, reports no issues (exit 0). Madge 8.0.0, through the command below, reports exactly the five chains T3797a requires removed (exit 1). Each contains a type-only edge; the longer chains also contain runtime imports. Thus installed fallow is cheaper but cannot enforce the requested boundary. Use madge unless a verified fallow configuration detects the same type edges; retain fallow's existing dead-code and duplication checks either way.

```sh
pnpm dlx madge@8.0.0 --circular --json --extensions ts,tsx --ts-config apps/desktop/tsconfig.json apps/desktop/src
```

Three sequential samples per command on the same Linux host, with cached pnpm runners, including pnpm startup and concurrent Rust lint compilation:

| Tool | Samples, seconds | Median, seconds | Cycle findings |
| --- | --- | --- | --- |
| fallow architecture | 1.007, 1.076, 1.023 | 1.023 | 0 |
| madge | 12.593, 13.179, 13.292 | 13.179 | 5 |

These are host observations, not a new CI or product performance budget. Installation/download time is excluded; implementation should measure its pinned invocation and fresh-runner setup separately.

Maintenance caveat: [madge](https://github.com/pahen/madge)'s latest published version, 8.0.0, and latest upstream commit (`456057b85c2d063dad9c10147d7686ded4fc5ce5`) are dated 2024-08-05. Do not claim recent maintenance. [Fallow architecture](https://fallow.tools/docs/cli/architecture/) documents cycles, but the installed version's actual result is the acceptance-relevant limitation. This record selects the detector by observed type-edge coverage, without adding a dependency or custom graph parser. The implementation still owes rule 9's reuse evidence for any machinery it adds.

## Rust duplicate versions

Both commands succeed (exit 0):

```sh
cargo tree -d --locked
cargo tree -d --locked --target all --prefix none --depth 0
```

The cross-target result contains 48 names with multiple distinct versions. Repeated identical name/version pairs from separate build contexts are collapsed here; they are not extra versions. These are inventory facts, not approved exemptions: implementation must either remove a duplicate or supply exact version entries with dependency-chain reasons under [cargo-deny's bans policy](https://embarkstudios.github.io/cargo-deny/checks/bans/cfg.html). No subtree or all-version exemption follows from this table.

| Crate | Distinct versions |
| --- | --- |
| base64 | 0.21.7, 0.22.1, 0.23.1 |
| bitflags | 1.3.2, 2.13.2 |
| foldhash | 0.1.5, 0.2.0 |
| getrandom | 0.3.4, 0.4.3 |
| hashbrown | 0.12.3, 0.15.5, 0.16.1, 0.17.1 |
| heck | 0.4.1, 0.5.0 |
| indexmap | 1.9.3, 2.14.2 |
| io-lifetimes | 2.0.4, 3.0.1 |
| jni | 0.21.1, 0.22.4 |
| jni-sys | 0.3.1, 0.4.1 |
| miniz_oxide | 0.8.9, 0.9.1 |
| nom | 7.1.3, 8.0.0 |
| png | 0.17.16, 0.18.1 |
| proc-macro-crate | 1.3.1, 2.0.2, 3.5.0 |
| quick-xml | 0.41.0, 0.42.0 |
| r-efi | 5.3.0, 6.0.0 |
| schemars | 0.8.22, 1.2.2 |
| schemars_derive | 0.8.22, 1.2.2 |
| serde_derive_internals | 0.29.1, 0.30.0 |
| serde_spanned | 0.6.9, 1.1.1 |
| shlex | 1.3.0, 2.0.1 |
| syn | 1.0.109, 2.0.119, 3.0.6 |
| thiserror | 1.0.69, 2.0.21 |
| thiserror-impl | 1.0.69, 2.0.21 |
| toml | 0.8.2, 1.1.6+spec-1.1.0 |
| toml_datetime | 0.6.3, 1.1.1+spec-1.1.0 |
| toml_edit | 0.19.15, 0.20.2, 0.25.15+spec-1.1.0 |
| windows | 0.61.3, 0.62.2 |
| windows-collections | 0.2.0, 0.3.2 |
| windows-core | 0.61.2, 0.62.2 |
| windows-future | 0.2.1, 0.3.2 |
| windows-link | 0.1.3, 0.2.1 |
| windows-numerics | 0.2.0, 0.3.1 |
| windows-result | 0.3.4, 0.4.1 |
| windows-strings | 0.4.2, 0.5.1 |
| windows-sys | 0.45.0, 0.59.0, 0.60.2, 0.61.2 |
| windows-targets | 0.42.2, 0.52.6, 0.53.5 |
| windows-threading | 0.1.0, 0.2.1 |
| windows_aarch64_gnullvm | 0.42.2, 0.52.6, 0.53.1 |
| windows_aarch64_msvc | 0.42.2, 0.52.6, 0.53.1 |
| windows_i686_gnu | 0.42.2, 0.52.6, 0.53.1 |
| windows_i686_gnullvm | 0.52.6, 0.53.1 |
| windows_i686_msvc | 0.42.2, 0.52.6, 0.53.1 |
| windows_x86_64_gnu | 0.42.2, 0.52.6, 0.53.1 |
| windows_x86_64_gnullvm | 0.42.2, 0.52.6, 0.53.1 |
| windows_x86_64_msvc | 0.42.2, 0.52.6, 0.53.1 |
| winnow | 0.5.40, 1.0.4 |
| winreg | 0.10.1, 0.55.0 |

## Python and spelling

[Ruff](https://docs.astral.sh/ruff/configuration/) 0.17.2 (released 2026-10-22) with an isolated explicit rule set reports 8 diagnostics in 6 files (exit 1):

```sh
ruff check --isolated --select E4,E7,E9,F --output-format json loop/
```

| File | Location | Rule |
| --- | --- | --- |
| loop/affected.py | 127:5 | E731 |
| loop/builder.py | 1035:5 | E731 |
| loop/builder.py | 1135:9 | F841 |
| loop/builder.test.py | 344:9 | E731 |
| loop/orders.test.py | 291:31 | E701 |
| loop/stall.py | 51:58 | E741 |
| loop/stall.py | 54:60 | E741 |
| loop/stall.test.py | 48:56 | E741 |

These locations identify observed diagnostics, not source citations for acceptance. Frozen loop files need no edits: implementation can baseline the exact occurrences and reject new ones.

[typos](https://github.com/crate-ci/typos) 1.51.1 (released 2026-10-06) reports 161 typo diagnostics across 74 files (exit 2). Its 21 binary-file notices are not typo diagnostics:

```sh
typos --isolated --format json
```

The implementation must record exact current occurrences rather than globally permit the words. Do not confuse typos' native exit 2 for findings with T3797a's normalized wrapper exit 2 for failed scanning. Preserve the actual distinction in the wrapper's tool-output handling.

## File-size inventory

A scan of tracked regular text files, counting newline bytes and applying the size gate's exclusions, finds 31 files above 1000 lines. Restricting to `.rs`, `.ts`, `.tsx`, `.css`, `.py` gives the audit's 27; CI must also cover other hand-written tracked text. Header/path/attribute exclusions come from `.claude/sound/placement/file-stays-under-a-size-cap.gate`, function `generated`. Do not add exemptions based only on extension or an apparently generated filename. These counts are starting evidence, not permanently approved ceilings.

| File | Newlines |
| --- | --- |
| `crates/agents/src/claude_code/discovery.rs` | 1682 |
| `crates/agents/src/rail.rs` | 1670 |
| `crates/agents/src/worktree.rs` | 1285 |
| `crates/agents/src/worktree_wiring.rs` | 1724 |
| `crates/agents/tests/asks.rs` | 1302 |
| `crates/agents/tests/launch/lean.rs` | 1090 |
| `crates/agents/tests/scenarios/a35.rs` | 1204 |
| `crates/agents/tests/scenarios/resume.rs` | 1641 |
| `crates/agents/tests/scenarios/worktrees.rs` | 2028 |
| `crates/desktop/tests/app.rs` | 2197 |
| `crates/messages/src/digest.rs` | 1228 |
| `crates/messages/src/lib.rs` | 2746 |
| `crates/messages/src/store.rs` | 1036 |
| `crates/pads/tests/pads.rs` | 1581 |
| `crates/rup/src/cli.rs` | 1167 |
| `crates/rup/tests/a34.rs` | 1182 |
| `crates/rup/tests/a35.rs` | 1306 |
| `crates/rup/tests/cli.rs` | 1319 |
| `crates/rup/tests/e2e.rs` | 4681 |
| `crates/rup/tests/e2e/fixtures/h15_hundred_tools.json` | 1521 |
| `crates/rup/tests/e2e/reaction.rs` | 1256 |
| `crates/rup/tests/mcp.rs` | 1567 |
| `crates/rupd/src/queue.rs` | 1030 |
| `crates/rupd/tests/b50_note_inbox.rs` | 1636 |
| `crates/terminal/src/lib.rs` | 1218 |
| `crates/todos/src/lib.rs` | 1943 |
| `loop/builder.py` | 1492 |
| `loop/builder.test.py` | 1706 |
| `proofs/messages/PROOF.bend` | 1630 |
| `spikes/tauri-latency/src-tauri/gen/schemas/desktop-schema.json` | 2291 |
| `spikes/tauri-latency/src-tauri/gen/schemas/macOS-schema.json` | 2291 |

## Preserved boundaries and validation

- `apps/desktop/reporting/results.test.mjs`, function `run`, invokes `reporting/vitest.config.ts`; it is used.
- `apps/desktop/src/perf/nativeDragProbe.tsx` imports both pragmatic-drag-and-drop packages; they are used and stay with their owning decision.
- No knip, madge or jscpd is installed in the base manifests. None is being replaced.
- `just lint-web` and `just lint-rust` pass on the observed base. `loop/rules.sh vocab` and `loop/rules.sh scenario-ids` pass with the specification present.
- CI currently invokes lint recipes through `.github/workflows/pre.yml`, jobs `lint` and `lint-web`. That workflow and the setup action are frozen; install tools through allowed recipe boundaries. Do not assume the lint jobs live in `check.yml`.
- The quality-target staging document is on `origin/audit/quality-target`, not main. The canonical work order authorizes C1, C3 and the C7 guard here; no staging-branch edit is required.
- [ast-grep rules](https://ast-grep.github.io/guide/rule-config.html) provide the requested structural matching; upstream 0.51.0 was released 2026-10-15. cargo-deny 0.20.2 was released 2026-07-09. Final implementation pins and inventories remain its responsibility, including actual advisory/licence and ast-grep matches.
