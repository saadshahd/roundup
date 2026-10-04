# U86 browser proof

Captured code: 69108b822a5e21d8f94906585ab491f668ef0778
Final code: 5668d3299009fa2040faba5f0c814be8d3fc2874

The final commit changes only `apps/desktop/src/todos/u15_todo_list.test.tsx`: a scoped query using the existing row helper, with unchanged button role/name, count and work threshold. The production tree is identical to the captured code.
Base: a1f4d5eae322470951bb4906b31412af564bed15
Platform: Linux Chromium through agent-browser. These are not WKWebView captures.

`manifest.json` records all 32 base/head × four screens × two sizes × system-preference captures, immutable code SHAs, PNG hashes, viewport, fixture population, computed colors and the observed conflict RPC. The `conflict` step clicks the Group action and observes `rail.createGroup` failing with -32003 and `name already taken`; navigation alone is not conflict proof.

Dark preference is recorded honestly: both revisions render the light palette. The 16 filenames containing `dark-preference-BLOCKED-light-rendering` are diagnostics, not proof of an implemented dark theme. No theme styles were injected.

`checks.json` and four additional `todo-focus` PNGs record actual keyboard Tab focus at both sizes. Base Todo hit height15px and no outline become24px and a2px accent outline, offset1px. Head SVGs measure16×16; head buttons containing migrated SVGs measure at least24px. Existing text-only controls are not claimed to satisfy all of D6. No complete-app polish claim is made.

Validation at unchanged code69108b8:
- Initial macOS `just check` exited1 at inherited L66 timeouts. Quiet focused base/head L66 runs each passed12/12.
- Remaining desktop suite passed967 tests; U67 Editor readiness and U15 constant-work timed out. Their quiet focused rerun passed24/24.
- One quiet full `just check` retry exited1. Rust637, L6635, lint, type checks, fallow passed; desktop968 passed,1 todo,1 failed. The sole failure is `u15_listing_blocked_todos_does_a_constant_amount_of_work_per_todo` at5000ms.
- The independent reviewer confirmed that U15’s global accessible-name search was incidental to the constant-work assertion. The final test-only commit scopes that same role/name lookup to Todo120, preserving readiness and reads<4800. Focused U15 passed18/18; the constant-work test completed in79ms. Required full final-head check is recorded separately below.

The full logs and recovered red/mutation/green regression evidence are in `logs/`. `capture.py` and `measure.py` preserve the actual browser procedure. Capture uses the harness clock; it does not inject a shared fixed clock. The final check and review status below decide delivery, not the earlier focused reruns.

## Measured Checks

- D2: every captured head SVG16×16; Rail rows28px in both revisions and both sizes.
- D4: glyph colors match the design-system accent, grey, amber and red; stable shapes checked by U86.
- D5: `contrast-summary.json` derives minimum4.54:1 over the existing selected band and minimum5.07:1 over white for measured head glyph tones. This does not claim genuine dark contrast.
- D6: migrated vector buttons>=24px; actual Todo Tab-focus ring2px, offset1px. Existing text-only controls and inherited missing hover surfaces are not claimed to pass all D6 states.

## Final local check

`just check` on macOS at5668d3299009fa2040faba5f0c814be8d3fc2874 exited0: Rust637, L6635, desktop969 passed plus1 todo, lint/typecheck/fallow green. `git diff --check` and size gate exit0. CI and independent review follow on the pushed head.
