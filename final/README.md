# U86 final browser and test proof

Final source: `99f9b338e25d124aa3a796a105ccc0feaf14d670`.
Before source: `a1f4d5eae322470951bb4906b31412af564bed15`.
Linux Chromium, driven only by agent-browser. This does not claim macOS WKWebView verification.

`manifest.json` records 32 before/after captures: four U86 screens × two window sizes × light/dark system preference. Every image has a hash, immutable source SHA, viewport, fixture population and computed colors. Final head captures were refreshed after the wrap fix. The original baseline captures remain bound to unchanged main source.

`conflict` clicks the Group action and verifies `rail.createGroup` failing with -32003 and visible `name already taken`. Navigation alone is not conflict proof. Both theme preferences render the existing light palette; the16 filenames containing `dark-preference-BLOCKED-light-rendering` are diagnostics, not genuine dark-theme proof. No palette was injected.

## Measured Checks

- D2: head SVGs16×16; Rail rows28px at both sizes in both revisions.
- D4: measured head glyph colors match the existing design-system accent, grey, amber and red. U86 tests distinguish each shape without hue.
- D5: `contrast-summary.json` derives minimum4.54:1 against the existing selected band and minimum5.07:1 over white for measured head glyph tones; genuine dark contrast remains unavailable.
- D6: migrated SVG buttons>=24px; actual keyboard Tab-focus on Todo rows produces2px accent outline, offset1px. Existing text-only targets and inherited absent hover states are not claimed to satisfy every D6 requirement.

`checks.json` accompanies four Todo keyboard-focus captures. Baseline Todo hit15px/no outline becomes24px/2px outline. Final focus captures use the final source.

## Copilot finding and repair

The one Copilot review identified a real wrap regression: keeping the old2ch indent after a16px SVG made continuation text start8.171875px left of the `#`. One local derived CSS property now combines the existing icon width with its monospace separator; padding and inverse text indent share it.

`wrap-checks.json` and four `*-wrap-*.png` images compare the previous approved source `e624c79015d02bd64fcbfc3bf0c6bd3871e60466` with final source99f9b33. Real Range coordinates show first-# and first-continuation xdelta -8.171875px→0px at1280×800 and700×800. Long row heights remain91.5px and61.5px. The pinning assertion failed before the change; U15+U86 passed20/20 after it.

## Validation

Final macOS `just check` on99f9b33 exited0: Rust637, coordinator35, desktop969 passed plus1 todo; lint, type checks and fallow green. See `logs/u86-wrap-check-final.log`. `git diff --check` and size gate exited0.

Earlier failed checks remain in `logs/`. A U15 full-suite timeout was repaired by scoping the same role/name readiness query to its existing row helper, retaining120 Todos and reads<4800; no timeout or threshold changed. Its focused run completed in79ms and final full checks passed.

The final QA-only VM had no agent or authentication; it was destroyed immediately after verified local collection, and `boxd machine list --json` returned an empty list. Capture scripts and raw observations are retained. The harness clock was not replaced with an injected shared clock; status ages were stable in these observations.
