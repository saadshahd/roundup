# Cost of rupd tests

Module: `crates/rupd`. Source: #2964. Acceptance below is policy for the implementation and its review; it requires no new test that checks test text or inventory text.

**T2964a remove redundant cost without losing behavioral proof.** Given the 24 tests in the inventory below, when their tests are audited under `AGENTS.md`, Rules, rule 10, then each original test has exactly one disposition in the implementation PR body (policy):

| Action | Required evidence in that test's row |
| --- | --- |
| Cut | Identify the compiler/type/lint check that catches the same break, the surviving test whose body catches it, or the explicitly retired scenario clause. A copied-value assertion alone is removed; retain any independently useful behavioral assertion. |
| Make cheap | Name the replacement test and the scenario clause it asserts. It fails for the same behavior break while starting no process, browser, Daemon, PTY or real `claude`. |
| Keep | Name the scenario clause and the specific real process or clock it needs, and explain why an observation starting none cannot catch that break. Put that requirement in the owning scenario if absent. A fixture using a process is not by itself evidence that the behavior needs one. |

Policy: inspect assertions and callers, not test names alone. Every retained scenario clause keeps a test whose body fails when that clause breaks, including after consolidation. Name replacements for renamed, split or merged tests in each original row. Do not replace a real behavioral assertion with an assertion only about a test double. Apply rule 10 to every edited test file, naming each additional cut and its surviving proof or retirement. No coverage target or prescribed number of kept tests applies.

Inventory: binary names identify the original CI test binaries; seconds are the baseline per-test elapsed times from green `check` run 39378452452. These are the fixed audit set, not a work queue.

| test | binary | seconds |
|---|---|---|
| `cleanup_completion::b6_resume_completion_waits_for_both_promotion_failure_cleanups` | `rupd::b6_cleanup_completion` | 82.3 |
| `cleanup_completion::b6_ack_timeout_waits_for_rollback_cleanup_and_accounts_for_refusal` | `rupd::b6_cleanup_completion` | 20.6 |
| `b21_start_releases_each_changed_child_under_b22s_caps` | `rupd::b21_stopped_door` | 16.9 |
| `b6_later_door_effects_cannot_overtake_a_released_resize` | `rupd::held_resizes` | 14.0 |
| `b21_a_reopened_door_waits_until_explicit_start` | `rupd::b21_stopped_door` | 11.5 |
| `b6_an_order_completion_waits_for_its_steer_effect` | `rupd::order_steering` | 10.8 |
| `b2704a_rename_updates_context_and_queues_one_note` | `rupd::b50_note_inbox` | 10.3 |
| `resources::tests::d8_a_disk_size_is_cached_for_30_s_and_the_walk_never_holds_the_call_past_1_s` | `rupd` | 10.2 |
| `b2704a_first_prompt_rename_notifies` | `rupd::b50_note_inbox` | 10.0 |
| `b6_wire_later_door_effects_wait_for_all_earlier_resizes` | `rupd::held_resizes` | 9.9 |
| `cleanup_completion::b6_unconfirmed_terminal_exit_preserves_the_resume_failure_and_blocks_a_blind_retry` | `rupd::b6_cleanup_completion` | 9.7 |
| `b2704a_workstream_rename_notifies_its_door` | `rupd::b50_note_inbox` | 9.6 |
| `b21_a_failed_start_keeps_the_entries_waiting` | `rupd::b21_stopped_door` | 9.3 |
| `b21_a_stopped_door_waits_silently_and_gets_the_newest_entry_on_start` | `rupd::b21_stopped_door` | 9.3 |
| `b21_a_refusal_other_than_a_stopped_door_is_logged_and_retried` | `rupd::b21_stopped_door` | 9.2 |
| `x14_daemon_observer::x14_daemon_observer_capacity_matches_its_receiver` | `rupd::n7_data` | 9.1 |
| `b2704a_unchanged_names_produce_no_note_for_a_no_word_first_prompt` | `rupd::b50_note_inbox` | 8.8 |
| `b6_checked_effects_exclude_prompt_replacement_under_parallel_load` | `rupd::prompt_identity` | 6.7 |
| `cleanup_completion::b6_cleanup_refusal_preserves_the_resume_failure_and_blocks_a_blind_retry` | `rupd::b6_cleanup_completion` | 5.5 |

**T2964b measure the implementation head.** Given T2964a's dispositions and a green `check` on the implementation PR's head, when its CI test timings are read, then the PR reports that run URL and head, each surviving original test's elapsed time and their sum against the recorded 313 s baseline (policy). Include the timings and a separate sum for replacement tests so moving work to another name cannot hide its cost. A cut contributes zero; a split or merge maps its measured tests back to all affected original rows and counts each executed test once in the total. Use per-test elapsed times from that run, not job duration, local timings or the baseline substituted for missing data. Missing timing evidence leaves acceptance unmet. No workflow or loop change is required or authorized by this scenario (policy).

Checks (existing acceptance on main; all remain in force):

1. `AGENTS.md`, Rules, rule 10: a test must detect its scenario's break without duplicating a cheaper proof.
2. `scenarios/messages.md`, B6's retired Takeover (its surviving cleanup amendments), B21 pushed on change, B22 caps, and B2704a: preserve action ordering, cleanup, stopped-Door delivery and rename-note behavior when changing their tests.
3. `scenarios/resources.md`, D8 the Daemon samples an Agent's Resource use when asked: preserve disk-cache and bounded-walk behavior.
4. `crates/rupd/tests/n7_data.rs`, `x14_daemon_observer::x14_daemon_observer_capacity_matches_its_receiver`: preserve actual receiver capacity, ordered delivery at capacity and lag at capacity plus one unless a named surviving test proves the same boundary.

Policy: this scope changes tests and the production seams needed to test the same behavior cheaply; it changes no product outcome, contract, performance budget or merge gate. Any scenario amendment must state the needed observer or an explicit retirement, never silently drop an existing behavioral requirement. Engineering progress and questions belong on #2964.

# Slow-test cost audit

Module: tests in `crates/agents`, `crates/terminal`, `crates/messages`, `crates/perf` and `crates/desktop`. Ids: R2965.

**R2965 preserve failure detection at lower test cost.** Given the 27 tests below, when their test cost is audited under AGENTS.md rule 10, then every original test has one disposition in the implementation PR's table: cut, make cheap, or keep. All acceptance below is policy, verified by independent review of the diff, surviving test bodies and the implementation head's CI evidence; add no test that rereads this specification or asserts the audit table.

1. Cut a test whose break already fails compilation, type checking or linting, whose assertions only repeat values copied from its implementation, or whose break another test already detects. Each cut names the surviving test and the then-clause it still proves, or explicitly retires the scenario's then-clause. For a compiler/type/lint rejection, name the failing check and the rejected construct. Do not retire required product behavior merely to delete its only observer.
2. Otherwise, make it cheap when a test starting no process, browser, Daemon, PTY or `claude` detects the same break. Replace the expensive setup with that observer, preserving the scenario's behavioral assertion and naming the replacement test. A shorter sleep alone does not establish equivalent failure detection.
3. Keep an expensive observer only when its scenario explicitly names the real process or clock it needs and the behavior that a process-free or controlled-time observer cannot detect. Add that need to the owning scenario if absent. A timer's existence alone does not require wall-clock waiting. Preserve the real-process assertion rather than merely exercising startup or checking a test double.
4. Every affected scenario's retained then-clause has a test whose body fails when it breaks. Further consolidation is allowed only with that mapping; changing test names or merging setup cannot hide a missing assertion. Apply rule 10 to the whole of each edited test file, naming the surviving proof or retirement for any additional cut.
5. The implementation PR has one row per original test below, with disposition, surviving/replacement test or retirement, and the reason. Renamed or consolidated tests remain traceable from every original row. Scope is these tests, their shared setup and the production seams necessary to remove their expensive setup, plus their owning acceptance scenarios; no product behavior change or loop machinery change is authorized.
6. From a successful `check` run on the implementation PR's own head, report the run URL, head SHA, each surviving observer's elapsed test time and their sum against the original 205 s group total. Include replacement tests, not only unchanged names; count a consolidated observer once and a cut with no replacement as zero. Use the same Linux test observations as the baseline, separating macOS observations and retries rather than adding them together. Missing timing evidence is incomplete acceptance. No fixed reduction target or weakening of existing product/performance bounds is authorized.

## Baseline inventory

Source: green `check` run [39378452452](https://github.com/saadshahd/roundup/actions/runs/39378452452). Times are elapsed test seconds, not workflow wall time; 205 s is the work order's rounded group baseline.

| test | binary | seconds |
|---|---|---|
| `x14_flood_leaves_rail_events_unlagged_and_cleans_up` | `perf::flood` | 14.8 |
| `d9_the_app_worktree_provision_wait_stays_within_the_call_bound` | `desktop::app` | 13.8 |
| `a35::a35_agent_wrong_acknowledgements_time_out_and_retry_the_same_copy` | `agents::scenarios` | 10.5 |
| `a35::a35_door_wrong_acknowledgements_time_out_and_retry_the_same_copy` | `agents::scenarios` | 10.5 |
| `orders::o8_the_prompt_survives_a_reopen_and_a_resume` | `agents::scenarios` | 10.4 |
| `resume::a2693a_a_stop_or_removal_claim_refuses_an_explicit_resume` | `agents::scenarios` | 10.4 |
| `resume::a21_a_failed_save_while_held_fails_the_whole_spawn` | `agents::scenarios` | 10.3 |
| `worktrunk::tests::u2556b_timeout_ends_a_stalled_collector` | `agents` | 10.1 |
| `a24::a24_a_start_never_acknowledged_fails_within_the_bound` | `agents::scenarios` | 8.3 |
| `events::tests::a14_a_signal_released_once_the_agent_is_registered_never_outruns_the_held_one` | `agents` | 8.2 |
| `n7_launch_sites_clean_on_success_and_failure` | `perf::n7` | 8.1 |
| `prepared_observer::g2542e_warm_launch_observer` | `agents::scenarios` | 8.0 |
| `x14_library_screen_edit_observation_survives_checkpoint_replay` | `terminal::library_edits` | 6.5 |
| `x14_library_screen_restore_state_is_bounded` | `terminal::library_attach` | 6.2 |
| `steer::a2703a_prompt_errors_use_rail_names` | `agents::scenarios` | 5.8 |
| `worktree_wiring::tests::u2875c_unpublished_resources_are_reconciled_at_every_durable_step` | `agents` | 5.5 |
| `orders::o3_a_workstream_gives_its_door_the_order_and_stopping_the_door_keeps_it` | `agents::scenarios` | 5.5 |
| `steer::h11_an_agent_that_ends_while_a_steer_waits_is_not_found` | `agents::scenarios` | 5.4 |
| `tests::b1_concurrent_sends_at_the_bound_never_both_succeed` | `messages` | 5.4 |
| `a41_removing_a_workstream_fails_its_queued_part` | `agents::asks` | 5.4 |
| `remove::a16_a_delete_failure_after_a_successful_stop_leaves_the_node_with_its_exited_agent` | `agents::scenarios` | 5.2 |
| `resume::a2681a_stopping_attempt_hides_parts` | `agents::scenarios` | 5.2 |
| `remove::a16_a_workstreams_failed_delete_leaves_its_children_under_it` | `agents::scenarios` | 5.2 |
| `resume::a2693a_stop_failure_launches_nothing` | `agents::scenarios` | 5.2 |
| `resume::a21_a_failed_save_after_spawn_returned_errors_settles_error_and_stops_the_terminal` | `agents::scenarios` | 5.2 |
| `events::tests::a14_a_failed_rail_write_for_a_held_signal_does_not_poison_the_locks` | `agents` | 5.1 |
| `events::tests::a21_a_failed_held_save_reinserts_so_the_caller_can_clean_up` | `agents` | 5.1 |

## Existing checks

1. `AGENTS.md`, Rules, rule 10: assertions must detect a scenario break; redundant or needlessly expensive tests are removed.
2. `justfile`, `lint-rust` and `lint-web`: compiler, lint and type-check rejection can replace tests of already-rejected constructs.
3. `crates/messages/src/lib.rs`, `tests::b1_concurrent_sends_at_the_bound_never_both_succeed`: the current B1 race observer is one of the audit subjects, not evidence that all observers require processes.
4. `crates/agents/src/worktrunk/tests.rs`, `u2556b_timeout_ends_a_stalled_collector`: the current collector timeout observer is one of the audit subjects; retention must identify the collector behavior that needs a real process or clock.


# Web test cost

Module: web tests in `apps/desktop/src` and their lint configuration under `tools/oxlint/` and `.oxlintrc.json`. Source: #3779. These scenarios specify T1's web timeout limit, T3's assertion limit and T4's web cuts. Implementation follows this specification in a dependent Issue; this specification delivers no lint or cut.

**T3779a bound web test timeouts.** Given a web test and the timeout it inherits or explicitly sets, when the repository lint runs, then a timeout above 60,000 ms fails unless an exact-test allowlist entry names its scenario, the real process or clock it needs, why a test starting none cannot prove that clause, and its finite maximum timeout. A value at or below that maximum passes only for that named test. Zero (unlimited) fails; an unresolved timeout expression fails with its location rather than silently escaping the limit. An absent override inherits the configured timeout; the existing 15,000 ms default passes. Configuration must not grant an unnamed test an over-limit timeout. A permitted exception is not permission to add retries or pad a wait.

Observer: planned lint fixture test `t3779a_timeout_boundary` runs the actual lint entry point on fixtures for the cases below. The fixtures use Vitest's numeric timeout argument, options-object `timeout`, imported `it`/`test` aliases, chained forms such as `test.each` and `test.skip`, local numeric constants/arithmetic and the inherited `testTimeout`. Each supported form must yield the same boundary result. The fixture test starts no browser, Daemon, PTY or vendor process.

| Input | Required lint exit |
| --- | --- |
| No override with the current default; explicit 60,000 ms | 0 |
| 60,001 ms without an entry | 1 |
| Exact exception within its declared finite maximum, with the required reason | 0 |
| Exception timeout on another test | 1 |
| Exception missing its process/clock reason | 1 |
| Timeout above the exception maximum | 1 |
| Zero (unlimited) timeout | 1 |
| Unresolved timeout expression | 1 |
| Inherited over-limit timeout on an unnamed test | 1 |

Policy: the implementation PR identifies the lint command that normal `pnpm lint` reaches and records one row per failing fixture with its input, diagnostic and exit code. No fixture may pass solely because its source was excluded from linting. Hooks, assertion polling and process-call deadlines are not per-test timeout exceptions; their existing bounds remain in force.

**T3779b prevent new appearance and copy assertions.** Given the web tests on the implementation's base and a proposed head, when the assertion lint runs, then it rejects each new assertion on computed style, measured position/size, or exact App-authored text. A baseline allowlist identifies each existing assertion by file, containing test and assertion identity, not a file-wide suppression, line number or count allowance. The head's entries must be a subset of the base's entries; deleted assertions lose their entries, and stale entries fail. Editing, moving or renaming an assertion cannot grant permission to add a different one. Baseline creation inventories the actual offenders once; subsequent cuts only remove entries (policy, checked by the independent review against the base and head).

Appearance assertions include `toHaveStyle`, values obtained from `getComputedStyle`, bounding rectangles, offset/client/scroll dimensions and equality/range assertions on their derived values, including values returned by browser evaluation. Exact-copy assertions compare App-authored labels, paragraphs, notifications or snapshots containing them to literal expected wording, through text matchers, DOM text reads or exact-text queries. A regex anchored to the complete copy is an exact-copy assertion too. Reads used only to set up an action are not assertions. Role/accessibility queries used to locate an action, nonempty accessible names, user-authored content round trips, call counts, RPC contracts and action/focus outcomes remain valid behavior observations. Such a query must not double as an assertion that App copy equals a literal. Appearance/capture observations remain the design critic's work; do not disable those observers to satisfy this lint (policy).

Observer: planned lint fixture test `t3779b_assertion_boundary` exercises the actual lint entry point, including TS/TSX and embedded browser expressions; Python browser assertions are included in the audit and may not become a way to move a forbidden assertion outside enforcement. The implementation must either lint a retained Python form or replace/remove its forbidden assertions before claiming acceptance.

| Input | Required lint exit |
| --- | --- |
| New computed-style assertion | 1 |
| New geometry assertion | 1 |
| New exact App-copy assertion | 1 |
| Forbidden assertion through a local alias | 1 |
| Forbidden assertion on a browser-evaluation result | 1 |
| Unchanged baseline assertion with its exact entry | 0 |
| Additional assertion in the same allowed file/test | 1 |
| Deleted assertion with a stale entry | 1 |
| Added entry after baseline creation | 1 |
| Broadened entry after baseline creation | 1 |
| Role query locating a click target, name-presence check, user-content round trip, call/contract assertion | 0 |

Policy: record each negative fixture's diagnostic and exit code separately. Test the lint's accept/reject behavior, not a copy of this specification or an inventory count. Before writing custom machinery, inspect the installed Oxlint plugin seam and Vitest timeout API, and evaluate maintained `eslint-plugin-testing-library` and Oxlint rules under AGENTS rule 9. Role-query rules alone do not establish the appearance/copy or shrinking-baseline boundary. The implementation's Reuse section states the selected capability and specific uncovered cases; this specification does not authorize a custom parser or a new dependency in advance.

**T3779c remove measured web test cost while preserving behavior.** Given the audit groups below, after both lints enforce T3779a and T3779b, when the web tests are cut or moved, then every removed original test has a row in its implementation PR body naming its scenario clause and the surviving test whose body still fails for that clause, or an explicit retirement of that scenario clause (policy). A rename, split, consolidation or move identifies its replacement in every affected original row. A fake that merely asserts its own implementation is no replacement. Apply AGENTS rule 10 to every edited test file, including additional cuts in that file.

| Audit group | Required disposition (policy) |
| --- | --- |
| Real-Daemon web tests: enumerate test consumers, including indirect fixture imports, from the measured base | Move to jsdom with a fake App/Daemon, or cut with the clause mapping above; retain exactly one real-Daemon web smoke, named with its behavior and justified real-process need. Helpers and metadata are not tests. |
| Chromium appearance tests: `header/u440_boxed_tabs.test.ts`, `app/u510_ink_feedback.test.ts`, `header/u2232_flat_tabs.test.ts`, `keys/u2230b_browser.test.ts`, `shelf/u371_reserved_modes.test.ts` | Remove appearance assertions/tests; retain independent action, focus and contract assertions cheaply. Explicitly replace any owning scenario's automated-looks observer with the design-critic observer rather than silently deleting acceptance. |
| Library re-proofs: `ui/u474_browser.test.ts`, `ui/u484_browser.test.ts`, `ui/u491_browser.test.ts`; drag stacks in `ui/u484_browser.test.ts`, `ui/u535_browser.test.ts`, `rail/u522_browser.test.ts`, `testing/u362_todo_drag.test.ts`, `todos/u155_reorder.test.tsx` | Keep one smoke per distinct library whose re-proofs are cut. Map each original to the named smoke and retain separate roundup behavior proof for unique actions, cancellation, calls and contracts. One smoke per file, widget or consumer is not one per library. Inspect current imports before identifying the library; historical filenames do not identify it. |
| Gutter: `terminal/u514_browser.test.ts` | Split setup/observations or reduce to the required then-clauses, preserving actual emulator-dependent behavior; do not delete its only observer or merely increase its timeout. |
| Copy strings: `u471_copy_preservation.test.tsx` in `bubbles/`, `header/`, `ink/`, `orders/`, `todos/`; `app/u472_copy_journey.test.ts` | Cut exact-copy proof. Preserve independent content preservation, recipient identity and delivery assertions in surviving behavior tests; retire only the copy-specific observer where needed. |
| Rendered/capture: `rail/u378_rendered.test.ts`, `keys/u2240_browser.test.ts`, `rail/u466_edges.test.ts`, `shelf/u460_rendered.test.ts`, `testing/u375_captures.test.ts`, `testing/u367_captures.test.ts`; control-name sweeps `ink/u501_browser.test.ts`, `header/u494_keyboard.test.ts` | Cut or move behavior to jsdom; design-critic observations own looks. Map redundant control-name checks to the assertions actually covered by `testing/y1_axe.test.tsx`, retaining any distinct behavioral assertion it does not prove. |
| Python/browser pairs: `todos/u3741`, `todos/u3760`, `todos/u3774`, `todos/u3776`, `settings/u3799` (each `_browser.test.py` plus `_rendered.test.ts`) | Cut or move behavior to jsdom, accounting for both the launcher and the Python assertions in each disposition. |

Paths in the table are relative to `apps/desktop/src`. They identify the fixed audit groups, not an instruction to delete whole files regardless of assertions. Each group's dispositions cover the whole group, in PRs bounded by one module directory; a directory PR may not silently leave part of its assigned class unaudited. Order directory work by measured cost after the lint baseline lands (policy).

Policy: preserve untouched the timing tests under `crates/terminal`, the failure tests `testing/u26_chromePage*.test.ts`, the real-Claude scheme test `terminal/x17_scheme.test.ts` and `crates/rup/tests/e2e/`. No change to those files, their helpers, acceptance, timeout allowances or execution selection may weaken those observers. The one real-Daemon smoke and one smoke per affected library have their existing scenario-prefixed test names recorded in the PR body; independent review runs them and the surviving tests for each retained clause. No new inventory-text test is required for T3779c.

## Measurement and regression policy

Before each cut, obtain a successful base CI run covering every affected original test, run `loop/web-durations.py` on its Vitest logs, and record the run URL, head SHA, executed files and elapsed seconds on the implementation Issue. Account separately for Python work included in a launcher's time, hooks, replacements and retries; count an executed test file once. A partial CI run or absent duration is missing evidence, never zero. Inventory the actual style/size/copy assertions and real-Daemon consumers with a reproducible command and source revision before creating the allowlists. The work order's “275 lines in 87 files” and “about 400 s” are audit claims to verify, not acceptance counts or measured savings (policy).

The committed `apps/desktop/src/testing/shardDurations.json` sums to 4,424 s and records 456 s for the gutter file at the specification base `a284e11af`; these are shard-planning values, not a fresh complete CI observation. `rg -l 'real_daemon|realDaemon|startDaemon' apps/desktop/src` finds 39 files there, including helpers, metadata and documentation; that count is not 39 executed tests. Reconcile the consumer inventory rather than cutting by a filename match (policy).

After each directory change, report the successful implementation-head CI run URL/SHA, affected original and replacement file seconds, and their total against the same base inventory. A removed test without replacement contributes zero only after its disposition is established; a merged/split observer maps back to every original and counts once in the total. Show which assertions, setup or waits were removed; no fixed reduction target, retry increase or product/performance regression is authorized. Missing baseline or head timing leaves implementation acceptance unmet. Progress and engineering questions belong on the work Issue, never in scenario Work tables (policy).

## Existing checks

1. `AGENTS.md`, Rules, rules 3, 9 and 10: bounded module PRs, reuse before machinery and surviving behavioral proof for every cut.
2. `apps/desktop/vite.config.ts`, default `test.testTimeout`: the inherited timeout is 15,000 ms; the new lint must reach overrides without raising this default.
3. `justfile`, `lint-web`, `lint-rust` and `test-web-since`: keep lint/type/compile rejection and affected behavior tests as merge evidence. This specification authorizes no frozen loop or workflow change.
4. `scenarios/ui-verify.md`, Y1 axe-core runs on the harness page: accessibility checks remain; do not assume a name-sweep's distinct action assertions are covered by axe.
5. `scenarios/terminal.md`, X17 a program's colour query is answered from the App's scheme: retain its real-Claude observation unchanged.
6. `scenarios/ui.md`, U26 concurrent cold Chromium readiness: retain failure detection, bounded replies and cleanup unchanged.
7. `scenarios/ui-terminal-turns.md`, U514 each command in a shell Terminal is a Turn the user selects from its gutter: preserve the real-emulator then-clauses when reducing the gutter test.

Policy: no product outcome, App/RPC contract, runtime performance budget or merge gate changes. Every Principle gate for a changed product path remains in force; this specification changes no such path. Its observers are lint rejection fixtures and independent review of retained behavioral assertions and CI measurements. Implementation scenario amendments may change the ordered test observers or explicitly retire redundant copy/looks proof; they may not retire required product behavior to fit a cut.
