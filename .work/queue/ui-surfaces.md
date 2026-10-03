# Work queue: ui-surfaces

The rows this squad holds (`docs/squads.md`, "Moving the queue"). The protocol, the status rules and the `Ids`/`Holder` table stay in `.work/queue.md`; the rule for a row is the same as there.

## Ready now

Scenario text is on main and nothing it needs is unmerged. Every row owns a directory no ready or in-flight row owns, so all start at once. Start the first row you have a free Builder for.

| Id | Item | Owns | Keeps green |
|---|---|---|---|
| U41 | Rail by keyboard, the rest (Moves: none) | `apps/desktop/src/keys/**`, one line in `src/App.tsx` | U31's and U32's tests |
| U112 | a Shelf row never moves on hover or focus (QA finding F7; Moves: none) | `apps/desktop/src/todos/**`, the Shelf rules of `apps/desktop/src/styles.css` | U35's tests |

## Waiting

Scenario text is on main. The row starts when what it waits on has merged. A UI row ends its Item with `(Moves: <D ids>)`, the design-system checks it moves (`docs/design-system.md`, baseline protocol); the Builder's PR body repeats the line and corrects it if the diff moves other checks.

| Id | Item | Owns | Keeps green | Waits on |
|---|---|---|---|---|
| U34 | the `↓ latest` control is visible: the `.pane-latest` rule outweighs `.word` (Moves: D1) | `apps/desktop/src/terminal/styles.css`; starts from branch `architect/terminal-defects-tdd`, does not edit `u34_latest_position.test.tsx` | `u34_latest_position.test.tsx` passes (1 of 2 red on main); `u34_` and `u13_` tests keep green | now |
| U11 | wide glyphs take two cells: Unicode 11 width table (Moves: none) | `apps/desktop/src/terminal/emulator.ts`, `apps/desktop/package.json`, `pnpm-lock.yaml`; starts from branch `architect/terminal-defects-tdd`, does not edit `u11_wide_glyphs.test.tsx` | `u11_wide_glyphs.test.tsx` passes (3 of 4 red on main); `u11_`, `u12_` and `u34_` tests keep green | now |
| U111 | **top priority**: a Drawer never moves the layout (the user's screenshot bug; Moves: none) | `apps/desktop/src/drawer/**`, `apps/desktop/src/styles.css` (the `.columns` rule only), `apps/desktop/src/app/**` for the harness check | `u5_`, `u24_`, `u40_` tests narrowed; new `u111_` | U40 on main; before U105–U110 |
| U105–U107, U109, U110 | the Thread's input and feed: dim, folded lines, jump, Attachment, Quote (Moves: D3) | `apps/desktop/src/thread/**` (new); U107 also `apps/desktop/src/rail/**`; U106 and U107 add chords to U54's help, which ui-surfaces owns | after the Door scenarios (not yet written); U107 after B6's `takeover.begin` Builder PR; narrows U12 for U107 |
| U101, U102 | Terminal copy and paste (Moves: D3); Terminal text size (Moves: D2) | `apps/desktop/src/terminal/**` | `u11_`–`u13_` tests | U101 after U57 merges; U102 needs U100 (merged); ids U101–U129 are reserved |
| U59 | the terminal blends with the app (Moves: D1, D5) | `apps/desktop/src/terminal/**` and its stylesheet | `u11_` to `u13_` tests; QA screenshots in `artifacts/ux/U59/` (not gating) | after U57 and the contrast tokens merge |
| U37 | reopen, webview half (Moves: D3, D5, D6, D7) | `apps/desktop/src/app/**` | `u25_daemon_gone.test.tsx` before a reopen | after S5 merges |
| contrast | Live lines and Ink text read at 4.5:1 (U4, U25) (Moves: D1, D5) | `src/styles.css`, `src/rail/styles.css`, the contrast tests, `u5_drawer.test.tsx` | U4, U25, U50 | go from the user; starts when this text is on main; observer: the computed-colour and stylesheet tests, `just check` |
| notch | U55: report which element draws the empty-Terminal notch | no app file (a report and screenshots under `artifacts/ux/U55/`) | U55 | go from the user; observer: the report |
| U130, U131, L41 | visual pass 1, Tokens: `tokens.css` holds every look value (starting values in `docs/design-system.md`), the four stylesheets read `var(--…)`, type and space steps; `loop/rules.sh tokens` with its tests, run by a `tokens` step in `.github/workflows/loop.yml` | `apps/desktop/src/tokens.css`, `apps/desktop/src/**/*.css`, `apps/desktop/src/main.tsx`, `apps/desktop/src/testing/harness.tsx`, `apps/desktop/src/testing/contrast.ts`, the inline styles of `apps/desktop/src/**/*.tsx`, `apps/desktop/src/ink/u4_contrast_tokens.test.ts`, slice 3 only `apps/desktop/src/ink/glyph.ts`, `apps/desktop/src/ink/glyph.test.ts` and `apps/desktop/src/ink/KindGlyph.test.tsx` (`working` and `idle` share the tone `light`; U132 then only tests the Kind tones), `loop/rules.sh`, `loop/rules.test.sh`, `.github/workflows/loop.yml` (the scenario says so) | every existing `apps/desktop` test; each `u4_`, `u5_` or `u20_` test that asserts a colour U130 replaces is changed and named in the PR | now, unless the `contrast` row is already in flight, then after it (D5 carries its ratios); three slices of U130 as `scenarios/ui-visual.md` orders them (Tokens with light values, one PR per stylesheet, then the dark block); L41's `tokens` step lands with the PR that leaves no literal outside `tokens.css`, so the check and the last migration land together; the `pads/` inline-style PR merges after U50, slice 3 after U50; the L41 PR opens only when no `loop-rules-*` PR is open (one writer of `loop/rules.sh` at a time), and adds only the `tokens` step if `loop-rules-4` already built `tokens` |
| U137 | one module computes D1 to D10; `window.__checks()` on the harness page | `apps/desktop/src/testing/**`, `apps/desktop/harness.html` | `u26_` tests | after U130 and U131 merge |
| U134, U135 | visual pass 3, surfaces and schemes: `--sunken` and `--ground`, the one hairline, the Drawer's shadow, dark, more contrast, reduced transparency | `apps/desktop/src/**/*.css`, `apps/desktop/src/drawer/**` | every existing test | after U132 and U133 merge |
| U103 | a reloaded Terminal pane shows its screen (webview half) | `apps/desktop/src/terminal/**` | U104's Builder PR |
