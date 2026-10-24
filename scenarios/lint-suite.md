# Missing lint boundaries

Scope: the lint recipes, their tool configuration and behavioral fixtures. Source: work Issue #3797. This specification delivers acceptance only; implementation belongs to a dependent Work Issue. Existing fallow, oxlint, stylelint, type checks and cargo-machete remain enabled. No product behavior, contract or frozen loop machinery changes.

**T3797a reject new lint violations without inheriting new debt.** Given the existing lint commands and a recorded baseline from the implementation's base on `origin/main`, when a contributor runs `just lint-web` or `just lint-rust`, then the following boundaries hold. Names in the Observer column are proposed implementation tests, not claims that these tests exist on main. Each runs the real selected tool against a small temporary repository or dependency graph and asserts diagnostics and status, rather than rereading recipe text. Independent review and CI establish actual recipe wiring (policy).

| Boundary | Required result | Observer |
| --- | --- | --- |
| Import cycles | `lint-web` rejects cycles within `apps/desktop/src`, including `import type`, type re-exports, self-imports and mixed value/type edges, using the desktop tsconfig and both `.ts` and `.tsx`. An acyclic graph passes. Break all five baseline cycles before implementation completion; its final cycle allowlist is empty. | `t3797a_cycles_include_types` |
| Rust dependencies | `lint-rust` runs `cargo deny check` alongside machete. Unapproved SPDX licences, RustSec advisories and additional crate versions fail. Configure `multiple-versions = "deny"`; each permitted duplicate is an exact crate/version entry with its dependency-chain reason. No crate-wide version wildcard or subtree exemption. Check the workspace graph across supported platforms, including macOS; Linux's active graph alone is insufficient. | `t3797a_dependency_policy` |
| File size | A new hand-written tracked file with 1001 newline characters fails; 1000 passes. Scan every tracked file, irrespective of extension, without requiring staged changes or installed hooks. Use exactly the generated/binary/vendored/lockfile exemption predicate of Check 3, applied to the checked revision. An existing oversized file may stay at its recorded ceiling or shrink, but cannot grow. | `t3797a_size_cap` |
| Sleep polling | ast-grep rejects a Rust `loop`, `while` or `for` containing a call to `sleep` or `sleep_until`, including qualified calls and `.await`. A sleep outside a loop passes. Scan tracked `.rs` files outside generated/vendor paths; tests are included. This is a syntactic telltale, not an interprocedural proof. | `t3797a_sleep_polling` |
| DOM cloning | ast-grep rejects member calls named `cloneNode` in tracked `.ts`/`.tsx` outside generated/vendor paths, with or without arguments. Comments mentioning it and calls to other methods pass. | `t3797a_dom_cloning` |
| Style overrides | ast-grep rejects a CSS declaration containing `!important` inside a TypeScript/TSX string or template literal, including multiline templates and JSX style values. Ordinary prose and comments pass. Existing standalone CSS stylelint enforcement stays. | `t3797a_style_overrides` |
| Spelling | `typos` checks repository text with its normal ignore/binary handling. A new misspelling such as `teh` fails, including a new occurrence of a word already present in the baseline. Correct text passes. Do not globally whitelist existing misspelled words or exclude a whole source directory to absorb its violations. | `t3797a_spelling` |
| Loop Python | `ruff check` covers `loop/` with explicit `E4,E7,E9,F` selection, including test files. A new unused import fails even in a file already containing a baselined diagnostic; clean Python passes. No frozen loop file edit is needed to establish its baseline. | `t3797a_python` |

Each ast-grep rule has positive and negative fixtures; style fixtures distinguish a CSS declaration from prose, and sleep fixtures include qualified asynchronous and synchronous calls. Fixtures demonstrate the rule on a newly introduced occurrence beside an existing allowed one (policy).

The cycle observer also checks these existing chains, relative to `apps/desktop/src`, each closing back to its first file. Each has a type-only edge; the longer chains also have value imports. Remove dependency edges without changing Rail persistence, connected-Project composition or their existing callers' behavior (policy).

| Chain required to become acyclic |
| --- |
| `rail/persist/storage.ts` → `state/connectedProject.ts` |
| `state/rail.ts` → `rail/persist/storage.ts` → `state/connectedProject.ts` |
| `state/rail.ts` → `rail/persist/storage.ts` → `state/connectedProject.ts` → `state/queue.ts` |
| `state/rail.ts` → `rail/persist/storage.ts` → `state/connectedProject.ts` → `state/worktrees.ts` |
| `state/rail.ts` → `rail/persist/storage.ts` → `state/connectedProject.ts` → `timeline/log.ts` |

## Baselines and failure reporting

Policy: introduce every new lint with an explicit inventory of the implementation base's current violations, including an empty inventory where none exist. Each exception names a reason. ast-grep exceptions live in an allowlist file, keyed by rule, repository-relative file, matched source and occurrence identity; exempting a whole file, rule, directory or word is forbidden. Use equally narrow diagnostic identities for Ruff and typos; crate exceptions name exact versions and advisory exceptions name advisory plus affected crate/version. Approved SPDX licence identifiers are the licence policy, not a blanket exception to licence checking.

The initial inventory is demonstrated against the unmodified base. Later inventories must be subsets of the merge base's inventory; occurrence counts and size ceilings may only decrease. A reduced count or oversized file records its new actual count as the ceiling in the same PR. Updating a tool or moving a file cannot add exceptions. When a violation disappears, remove its stale entry in the same PR. Keep any residual exceptions visible in lint output. The zero-cycle result is required even though the initial inventory records five cycles. Main remains green throughout rollout; this work does not require splitting all oversized files or repairing unrelated historical spellings and Python diagnostics.

`t3797a_baseline_ratchet` asserts baseline-only success, rejection of an extra occurrence, rejection of a widened/new entry, rejection of a raised size ceiling, successful removal, and rejection of stale entries. It compares actual inventories and tool output, not configuration strings.

Policy: each added lint command uses this failure contract. Native tool statuses may be normalized by the caller; no pipeline, warning-only setting, missing executable or failed inventory read can turn a failure into success. Behavioral observer: `t3797a_lint_failures`.

| Result, one row per failure | Exit | Required output |
| --- | --- | --- |
| Clean or only current exact exceptions | 0 | Tool identity and any remaining baseline debt |
| New cycle | 1 | Cycle's file chain |
| Disallowed licence | 1 | Crate/version and SPDX expression |
| Advisory outside baseline | 1 | Advisory id and crate/version |
| Additional duplicate version | 1 | Crate name and versions |
| File above its permitted size | 1 | File, measured line count and ceiling |
| Sleep-poll match outside baseline | 1 | Rule and file/location |
| `cloneNode` match outside baseline | 1 | Rule and file/location |
| Style override outside baseline | 1 | Rule and file/location |
| Spelling outside baseline | 1 | File/location and misspelling |
| Ruff diagnostic outside baseline | 1 | Rule and file/location |
| Added/widened exception or ceiling | 1 | Exception identity and baseline difference |
| Stale exception | 1 | Exception identity |
| Missing tool, malformed configuration, failed scan/graph resolution or unreadable baseline | 2 | Tool/input and cause; partial results are not clean |

Policy: install pinned tools reproducibly through existing setup/recipe boundaries; CI's `pre` lint jobs must exercise them on a fresh runner without lefthook. A new helper lives outside frozen `loop/`; only `check.yml` may change under workflows. Extend existing tools rather than adding another dead-code or duplication checker. Apply rule 9 before writing any machinery. Keep tests bounded and process-free except where invoking the actual lint tool or Cargo graph is the behavior being observed; no App, browser, Daemon, PTY or real `claude` is needed.

## Existing checks on main

1. `justfile`, recipes `lint-web` and `lint-rust`: existing lint entry points; `check-web` and `check-rust` call them.
2. `package.json`, script `slop`, and `.fallowrc.json`, `rules`: fallow dead-code and duplication checks remain; no knip, madge or jscpd is being replaced.
3. `.claude/sound/placement/file-stays-under-a-size-cap.gate`, function `generated`: sole definition of file exemptions. Preserve the pre-commit gate; CI must not depend on its staged-file input or fail-open trap.
4. `.github/workflows/pre.yml`, jobs `lint`, `lint-web` and `pre`, and `.github/workflows/check.yml`, job `approved`: existing lint/approval boundaries remain; the PR cannot approve its own gate.
5. `apps/desktop/reporting/results.test.mjs`, function `run`: invokes `reporting/vitest.config.ts`; preserve this used configuration.
6. `apps/desktop/src/perf/nativeDragProbe.tsx`, imports: pragmatic-drag-and-drop and its hitbox package are used; leave them and their owning decision unchanged.
7. `AGENTS.md`, Rules 7, 9 and 10: preserve performance budgets, research reuse before adding machinery and retain only tests that detect a distinct required break.

Principles: this changes engineering checks only. No product Gate gains a new path; no product Observer or Mac observation is needed. Observe the new boundaries through the real lint commands and isolated fixtures above, plus independent review of CI wiring and initial inventories.
