# Contracts from one declaration

Specification for #3801. This document defines acceptance; it does not deliver the migration.
Scope: `crates/contracts`, existing module dispatchers, `crates/rup/src/mcp.rs` and
its helpers, `contracts/generated/`, the `justfile` contract recipe and affected callers.
No App behavior, method permissions, wire names or Message delivery rules change.

**C4 a method is one declaration and one handler.** Given the migrated contracts,
when a developer adds an RPC method using existing parameter and result types,
then the only manually authored registration is one declaration naming its wire
name, Rust parameter/result types, owning module, handler and MCP exposure.
Its implementation is one handler; the declaration generates the method table,
module router entry, TypeScript bindings and, when exposed, MCP tool. No second
method-name list, stringly typed TS type expression, router arm or MCP registration
is edited. New domain types and behavior tests are additional work, not hidden
registrations. The following clauses are implementation acceptance, with proposed
test names; none of those proposed tests is claimed to exist on main.

| Clause | Then | Observer to implement |
| --- | --- | --- |
| Addition | In an isolated fixture, add a harmless method using existing types with MCP exposure for ordinary Agents and Doors. Edit only its declaration and handler; generate, enumerate it once, invoke it through the real module dispatcher and the MCP adapter, and typecheck a TS caller with its exact parameter/result types. The handler receives the supplied values and its result reaches both callers. Removing the declaration removes all generated registrations. | `c4_one_declaration_reaches_every_surface`; use the existing in-process module and MCP seams, no Claude or OS process |
| Exposure | The declaration explicitly selects no tool, every Agent, or Door only; absence never exposes a tool. Preserve tool-name exceptions, descriptions, own-id/reader injection, parameter reshaping, stream handling and Door exclusions. A non-offered call never reaches a handler. Existing tools keep their role sets and schemas. | `c4_declarations_preserve_mcp_policy`, plus Checks 3–4 |
| Freshness | On a clean checkout `just contracts-fresh` itself regenerates every derived output, then exits 0 only when committed outputs match. No preceding Rust test run is required. A stale, missing, extra or hand-edited generated file exits nonzero even if regeneration repairs it. A declaration change with old outputs also exits nonzero. A second generation is byte-identical. | `c4_contracts_fresh_detects_drift`, isolated checkout with the cases in the table below |
| Types | Export all existing contracts with plain `specta` and its Serde/TS exporter crates. Preserve paths, exported names, optional versus nullable fields, numeric representation, unions, tags and flattening. Each changed generated file is formatting-only or has an individually documented type-shape change and all callers updated in the same PR. | `c4_exported_shapes_match_existing_contracts`: TS compiler fixtures prove assignability in both directions and reject absent required fields, invalid nulls and wrong discriminants; review records each intentional exception (policy) |
| Atomic migration | Remove `ts-rs` dependencies, derives, attributes and exporter calls from every crate, workspace manifests and lockfile in the same PR. No transitional second generator and no `tauri-specta` dependency. `schemars` may remain for MCP JSON Schema; it is not a second Rust-to-TS generator. | Policy: dependency/source audit and `cargo tree --workspace`; compilation rejects leftover generator references |
| Wire compatibility | Keep Serde serialization/deserialization and Daemon request/reply/error semantics unchanged. Preserve the wire-test function bodies and expected values of Check 2, including the Pad round-trip. Compare serialized bytes against pre-migration fixtures as well: those existing tests compare JSON values, not byte order. | Check 2 and `c4_wire_bytes_match_before_migration`, using the same Event values and representative method request/reply/error envelopes |

Freshness observer: run each mutation separately from the same clean generated
fixture, restore it between rows, and assert both the exit code and named path.
The implementation owns the command and tests outside frozen loop machinery.

| Input to `just contracts-fresh` | Exit | Result |
| --- | --- | --- |
| Committed outputs equal generation | 0 | Every derived surface regenerated; no diff |
| Change one committed generated TS file by hand | 1 | Names changed path; fails even if repaired |
| Delete one committed generated file | 1 | Names missing path; fails even if recreated |
| Add an unexpected file under the generated output root | 1 | Names extra path |
| Change declaration but retain old generated outputs | 1 | Names stale output paths |
| Generator failure | 1 | Names failed generation step; never reports freshness |

The implementation may normalize a generator's nonzero code to 1; it must not
swallow it. Generated Rust/MCP outputs, if emitted as files, obey the same drift
checks as TS outputs. Compile-time generated router/tool code has no committed
file to compare; the addition and exposure observers exercise that code instead.

## Checks retained from main

1. `crates/contracts/src/methods.rs`, `writes_methods_ts`, `writes_contract_ts`,
   `a38_fingerprint_follows_the_methods_and_types` and
   `a38_a_change_to_one_method_rewrites_only_its_line`: method bindings and
   deterministic fingerprint behavior. Preserve order-independent fingerprints;
   a fingerprint changes only with its declared method/type surface.
2. `crates/contracts/tests/event_wire.rs`,
   `r16_large_event_payloads_keep_the_exact_wire_envelope`,
   `r16_event_data_keeps_the_generated_typescript_shape`,
   `a34_pad_deleted_round_trips_the_pad_name` and
   `a34_pad_deleted_is_in_generated_typescript`: wire values and EventData TS shape.
3. `crates/rup/tests/mcp.rs`, `m1_offers_one_tool_per_method_and_no_others` and
   `m1_input_schemas_are_the_contract_schemas`: tool inventory and JSON Schema.
4. `crates/rup/tests/mcp.rs`, `m5_every_app_action_has_a_tool_or_a_reason`, and
   `crates/rup/src/mcp.rs`, `door_tools`: explicit non-exposure and Door exclusions.
5. `justfile`, `contracts-fresh`: committed-output drift detection. C4 adds
   regeneration to that command; main currently regenerates in Rust tests first.
6. `AGENTS.md`, Rules 4, 7, 9 and 10: atomic callers, performance budgets,
   library evidence and tests that catch distinct behavior (policy).

Check 2's wire test bodies/fixtures stay unchanged. Its `use ts_rs::TS` and
`EventData::decl` assertion must be ported to Specta, preserving their proof of
the generated union; keeping the entire file byte-for-byte would contradict
removing `ts-rs`. Formatting of the generated union is not wire behavior.
If an edited test is redundant, its removal names the remaining test proving
its clause under rule 10; no wire assertion is weakened to fit the exporter.

## Baseline and migration decisions

Verified on main commit `a284e11af9d48e9fad4f83fba29bfc757019e3e3`:
`contracts/generated/` contains 276 tracked files. For a null-to-null method in
an existing module, a disposable `project.contractProbe` addition has three
hand-edited files: `crates/contracts/src/methods.rs` (`METHODS`),
`crates/agents/src/dispatch.rs` (`Module::call`, inline handler), and
`crates/rup/src/mcp.rs` (`offered_tools`). `writes_methods_ts` and
`writes_contract_ts` derive the two TS files; they are not hand edits.
A separate handler file or new parameter/result types add their implementation
files; neither is evidence for five hand-edited registrations. The probe is
removed before committing the specification.

The MCP list lives in `offered_tools`, with special tools in
`crates/rup/src/mcp/agent_tools.rs`; `commands`, `tools` and `door_tools` filter
and extend it. `offered` already uses installed `schemars` for input schemas.
Preserve that capability and the current RPC transport/module boundary; Specta
supplies type metadata, not an authorization layer or a replacement Daemon.

Inventory every `#[ts(...)]` in `crates/contracts`, not only the four files in
the work order's original evidence. The migration fixture covers these families:

| Existing annotation | Specta migration candidate and required shape |
| --- | --- |
| `export`, `export_to` | Explicit exporter collection/layout; retain current module paths and imports |
| `rename` | Serde rename or Specta metadata rename; retain current TS and wire spelling independently |
| Numeric and nullable numeric `type` overrides | TS `Number` metadata or equivalent type override, retaining nullable wrapper where present; no wire integer-to-string conversion |
| `optional` | Optional property; remove null from its metadata only where the existing TS excludes null |
| `optional = nullable` | Optional property retaining null, including launch Model/Effort fields |
| `as = "Option<bool>"` with `optional` | Metadata override; retain the existing optional boolean shape |
| `type = "Record<string, unknown>"` | String-keyed map of TS `Unknown` metadata; no `any` substitution |

Specta v2.0.0-rc.27 source supplies field `type`/`optional` overrides,
Serde renaming/tagging/flattening and TS `Number`/`Unknown`. A disposable Rust
fixture compiled with that tag and `specta-serde`/`specta-typescript` 0.0.14,
exporting required/nullable numbers, optional non-null boolean/string, optional
nullable string, `permissionMode`, a string-to-unknown map, a tagged Event,
flattening and a transparent wrapper with their expected TS shapes. This proves
those annotation mechanisms, not a full migration: the whole-contract type
fixture must pass before replacing the installed generator. Select and pin compatible Specta,
Serde and TS exporter versions together. No custom type parser or router
framework is authorized by this specification; rule 9 applies to any generator
glue and requires a specific missing library capability before custom machinery.

## Principles and boundaries

P4 gate: does generated dispatch/tool metadata introduce vendor knowledge outside
its Adapter? Observer: inspect changed declaration/dispatch paths; names and
transport remain vendor-independent. P9 gate: does generation change an Agent's
tool list during its session? Observer: the exposure test compares repeated
lists for each role and the existing fixed-list construction remains in use.

Policy: no Message, Route, Held or Takeover behavior changes; none of the M1–M10
Message laws in `scenarios/proofs.md` is amended. Keep `proofs/messages` and
`docs/messages.md` unchanged. A later behavioral change needs its own work order
and Contract review. Existing performance budgets apply; C4 adds no runtime
reflection/export or per-call regeneration. No native Mac observation is needed
for this declaration/export contract. Implementation remains in a dependent
Issue until these observers pass.
