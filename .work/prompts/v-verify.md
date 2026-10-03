You are a Builder (`.agents/builder.md`). Build scenarios V1, V2 and V3 in `scenarios/ui-verify.md`, and nothing else. Read `AGENTS.md`, `CONTEXT.md`, `PRINCIPLES.md`, `scenarios/ui-verify.md`, `scenarios/ui-drawer.md` (U111), `scenarios/ui.md` (U26) and `.claude/sound/` first.

Edit only `apps/desktop/src/testing/**`, `apps/desktop/harness.html`, `apps/desktop/package.json` (the `axe-core` dependency, MPL-2.0, from the npm registry, pinned to the 4.13 line), `loop/rules.sh` and `loop/rules.test.sh`. Do not edit scenario files or `loop/rules.sh` outside the new `laws` subcommand. Use the library: do not write a contrast or ARIA checker of your own, and do not add a second implementation of D1 to D10 (U137's module stays the one).

Build, test first, in this order, one commit each: (1) V2's sampler as one function in `apps/desktop/src/testing/` and the `v2_` tests; (2) `window.__axe()` and the `v1_` tests, with a fixture page for each case the scenario lists; (3) `loop/rules.sh laws` and the `v3_` tests, with the three first laws added as `Law:` and `Check:` lines under the named clauses in `scenarios/ui-drawer.md` (U111), `docs/design-system.md` is not edited. Add a `Law:` line only where the scenario names it; do not invent laws.

Observer: `just check` green; the `v1_` to `v3_` tests; drive `just harness first-run` in `agent-browser` on a VM with auto-destroy (never a window on the user's machine) and report the `window.__axe()` result and the sampler's frame count for a Todo Drawer open and close at 1280 by 800. Say plainly what jsdom could not show.

Report: the test names, the axe-core version, the `window.__axe()` output for the three seeds, how many frames the sampler read, and the laws sheet's output.
