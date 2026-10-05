---
name: percy-migrate
description: "Migrate a project's existing visual testing to Percy. Handles two families: (A) Applitools Eyes — maps eyes.open/check/close, coded regions, and applitools.config.js to percySnapshot(), Percy regions, and .percy.yml; (B) in-house screenshot assertions — Playwright toHaveScreenshot(), cypress-image-snapshot, jest-image-snapshot, WDIO visual service — replacing local golden-file baselines with Percy's server-side baselines. Proposes a full mapping before touching code, and is honest about what has no Percy equivalent."
argument-hint: "[optional: what you're migrating from, e.g. 'applitools' or 'toHaveScreenshot']"
---

# Percy Migrate

Takes a project that **already does visual testing** — via Applitools Eyes or in-house screenshot assertions — and migrates it to Percy. The counterpart to `/percy:integrate` (which assumes *no* visual testing yet): here the job is translation, not introduction. Existing checkpoints tell us exactly where snapshots belong; the work is mapping each call, config, and baseline workflow to its Percy equivalent — and saying clearly when there isn't one.

For a project with no visual testing at all, use `/percy:integrate`. To grow coverage after migrating, use `/percy:expand-coverage`.

## Eligibility (check first, stop if not met)

| Condition | Required |
|---|---|
| Project is **Percy Web** (browser snapshots) | ✅ — not native mobile. Applitools native-mobile suites map to **App Percy**, a different product; say so and stop for those tests |
| An existing visual-testing setup is present (Family A or B below) | ✅ — otherwise this is `/percy:integrate`'s job |
| Percy **not already integrated** | ✅ — if `@percy/*` + `percySnapshot(` already exist alongside the old tool, offer to finish/clean up the migration rather than redo it |

## Steps

### 1. Detect what they're migrating from (dep scan)

Scan `package.json` / lockfiles / requirements / build files and grep the suite:

| Signal | Family |
|---|---|
| `@applitools/eyes-playwright` / `eyes-cypress` / `eyes-webdriverio` / `eyes-selenium` / `eyes-storybook`; `eyes-selenium` (PyPI), `com.applitools:eyes-*` (Maven); `applitools.config.js`; `APPLITOOLS_API_KEY`; `cy.eyesOpen(` / `eyes.check(` in code | **A — Applitools Eyes** |
| `toHaveScreenshot(` / image `toMatchSnapshot(` (Playwright); `cypress-image-snapshot` or `@simonsmith/cypress-image-snapshot` (`cy.matchImageSnapshot(`); `jest-image-snapshot` (`toMatchImageSnapshot(`); `@wdio/visual-service` / `wdio-image-comparison-service` (`checkScreen`/`checkElement`/`checkFullPageScreen`); baseline dirs like `*-snapshots/`, `__image_snapshots__/`, `cypress/snapshots/` | **B — in-house screenshot assertions** |

If both are present, ask which to migrate (or both, sequentially). Also detect the test framework + package manager exactly as `/percy:integrate` Step 1 does — the Percy SDK to install is the same table.

### 2. Inventory existing visual checks

Enumerate every check call (file, test name, check name, options used — regions, match levels, `fully()`, masks, thresholds), plus config files, env vars, CI steps (e.g. `--update-snapshots` jobs, Applitools batch env), and baseline directories. This inventory drives the proposal and is the migration's checklist — nothing gets silently dropped.

### 3. Propose the mapping (before any edit)

Show the user a table for **their** framework with before/after code, plus the concept map. Get approval before transforming.

**Family A — Applitools → Percy concept map:**

| Applitools | Percy equivalent |
|---|---|
| `APPLITOOLS_API_KEY` | `PERCY_TOKEN` (via `/percy:setup` / BrowserStack MCP path) |
| `eyes.open(...)` / `eyes.close()` / `eyesOpen`/`eyesClose` per test | **Nothing** — `percy exec` wraps the whole run; delete the session calls |
| `eyes.check('name', Target.window().fully())` / `eyes.checkWindow('name')` / `cy.eyesCheckWindow('name')` | `percySnapshot(<handle>, 'name')` — full page is Percy's default |
| `Target.region(selector)` / region target | `percySnapshot(..., { scope: '<selector>' })` |
| `.ignore(selector)` / ignore regions | `regions: [createRegion({ elementCSS: '<sel>', algorithm: 'ignore' })]` (or `percyCSS` to hide) |
| `.layout(selector)` / layout **regions** | `createRegion({ ..., algorithm: 'layout' })` |
| Dynamic-content noise (ads, carousels) | `algorithm: 'intelliignore'` region, or project-level Intelli Ignore |
| **Global** match level `Layout`/`Content`/`Exact` | ⚠️ **No global equivalent.** Closest: per-region `layout`/`intelliignore`, plus Percy AI diff grouping at review time. Surface every test that sets one |
| Ultrafast Grid `browser: [{width,…}, {deviceName}]` in `applitools.config.js` | `widths:` in `.percy.yml` + browsers (Chrome/Firefox/Safari/Edge) in Percy **project settings**; UFG `deviceName` emulation → nearest width, note the approximation |
| `BatchInfo` / `APPLITOOLS_BATCH_ID` | Percy **build** — automatic per `percy exec` run; branch via `PERCY_BRANCH` / `PERCY_TARGET_BRANCH` |
| Branch baselines, save/merge | Percy picks baselines from **approved builds on the base branch** automatically |
| `applitools.config.js` / `eyesConfig` in `playwright.config.ts` | `.percy.yml` |

**No Percy equivalent — list these to the user honestly, per occurrence found:** floating regions, per-check match levels beyond the four region algorithms, accessibility/contrast validation, root-cause DOM diffs, visual locators, PDF/image-file testing, UFG real-device rendering. If the suite leans on one of these, say what's lost and let the user decide before proceeding.

Example before/after (show only the detected framework):

```js
// Before (Applitools, Playwright)          // After (Percy)
await eyes.open(page, 'App', 'checkout');   // (deleted)
await eyes.check('Cart', Target.window().fully().ignore('.promo'));
await eyes.close();                          // (deleted)
// →
await percySnapshot(page, 'Cart', {
  regions: [createRegion({ elementCSS: '.promo', algorithm: 'ignore' })],
});
```

Cypress: `cy.eyesCheckWindow('Cart')` → `cy.percySnapshot('Cart')` (drop `eyesOpen`/`eyesClose`, swap the `eyes-setup` support import for `import '@percy/cypress'`). Selenium: `eyes.check(...)` → `percySnapshot(driver, 'Cart')`. Storybook: `eyes-storybook` → `@percy/storybook` (both auto-snapshot per story).

**Family B — screenshot assertions → Percy:**

| Before | After |
|---|---|
| `await expect(page).toHaveScreenshot('cart.png', { mask: [loc] })` (Playwright) | `await percySnapshot(page, 'cart')` + ignore region/`percyCSS` for the mask |
| `await expect(locator).toHaveScreenshot(...)` | `percySnapshot(page, '<name>', { scope: '<selector>' })` |
| `cy.matchImageSnapshot('cart')` (cypress-image-snapshot) | `cy.percySnapshot('cart')` |
| `expect(await page.screenshot()).toMatchImageSnapshot()` (jest-image-snapshot + Puppeteer) | `await percySnapshot(page, '<name>')` via `@percy/puppeteer` |
| `browser.checkScreen('cart')` / `checkElement` / `checkFullPageScreen` (WDIO visual service) | `browser.percySnapshot('cart')` (element → `scope`) |
| `maxDiffPixels` / `threshold` / `failureThreshold` tuning | ⚠️ No per-snapshot pixel threshold — Percy's noise handling is Intelli Ignore + human/AI review, not a numeric knob. Say so |

**Behavioral change to state up front:** these assertions **fail the test locally** on a pixel diff; `percySnapshot()` never fails the run. Review moves to the Percy dashboard / CI status check (`/percy:review`, `/percy:gate`). Local golden files are replaced by Percy's **server-side baselines** — no more `--update-snapshots` commits or cross-OS rendering flake.

### 4. Transform on confirmation

- Install `@percy/cli` + the framework's Percy SDK (same table and package-manager detection as `/percy:integrate`); remove the old dependency only in Step 6.
- Apply the approved mapping with the **smallest possible diffs**: replace each check call in place, keep test logic, names, and structure untouched. Reuse the old check/snapshot names as Percy snapshot names (they're the baseline key).
- Migrate options that have an equivalent (regions, scope, widths); where one doesn't, leave a `// TODO(percy-migrate): <what was here and why it has no equivalent>` and include it in the final report.
- Port `applitools.config.js` browsers/widths into `.percy.yml`; run `/percy:setup` (or replicate it) to write `.percy/config.yml` and wire `PERCY_TOKEN` (never ask for the token in chat).

### 5. Clean up old baselines and infra — with confirmation, never silently

Present the exact list before deleting anything: old dependency + config file, support/plugin registrations (`eyes-setup` artifacts, `cypress-image-snapshot` plugin hooks), baseline dirs (`*-snapshots/`, `__image_snapshots__/`, `cypress/snapshots/`), and CI steps for updating/uploading pixel diffs (`--update-snapshots` jobs, diff-artifact uploads, `APPLITOOLS_API_KEY` secrets). On approval, remove them and `git rm` the baseline dirs. If the user wants a safety net, suggest doing cleanup in a follow-up commit so the migration commit is pure translation.

### 6. Run the first build and hand off

Run the build via `/percy:run-build` (its Steps 2–2c resolve the browser inline and self-heal agent-shell launch traps) — not a bare `percy exec`:

```bash
PERCY_BROWSER_EXECUTABLE="<resolved path>" PERCY_TOKEN=<token> percy exec -- <the project's test command>
```
The first build has **no baseline** — every snapshot appears new. Hand off to the `/percy:setup` baseline flow: approve the first build on the base branch so subsequent builds diff against it.

**Acceptance check — count snapshots via the API, not CLI stdout.** The CLI's final `Uploading N snapshots...` line is the *remaining upload queue at exit*, not the build total (e.g. a 6-snapshot build can print "Uploading 2 snapshots..." because 4 had already uploaded mid-run). Count `[percy] Snapshot taken:` lines during the run, then confirm against the build itself:
```
GET /api/v1/snapshots?build_id=<id>&page[limit]=100
```
The build's snapshot count must match the Step-2 inventory. (Snapshots appear a few seconds after finalization — if the count reads low/zero immediately, wait and re-query before concluding snapshots were dropped.)

## Output

> Migrated 14 Applitools checks across 6 spec files to `percySnapshot()` (Playwright). Mapped 5 ignore regions and 1 layout region; 2 checks used `matchLevel: Layout` globally — no Percy equivalent, flagged with TODOs. Removed `@applitools/eyes-playwright`, `applitools.config.js`, and the UFG CI step. First build: 14 snapshots, matching inventory. Approve it on `main` to set the baseline.

If anything couldn't be mapped or the build count doesn't match the inventory, surface it — don't report a clean migration.

## Important

- **Web only.** Applitools native-mobile (or Appium-based) suites are App Percy territory — a different product; stop and say so rather than mis-migrating.
- **Don't oversell.** Match levels, floating regions, accessibility checks, and pixel-threshold knobs have no Percy equivalent. Every unmappable feature found in Step 2 must appear in the proposal and the final report.
- **Propose before editing.** The mapping table (Step 3) and the deletion list (Step 5) each require explicit approval. Never silently delete baselines or CI config.
- **Smallest diffs.** Translate calls in place; don't refactor tests, rename cases, or "improve" logic while migrating.
- **Names are baselines.** Carry old check names over as Percy snapshot names, unique and stable. For Family B golden files, use the *logical* name — strip the `.png` extension and any platform suffix Playwright appends to stored files (`home-full.png` / `home-full-darwin.png` → `home-full`).
- **Cross-IDE.** Prefer `percy-cli` + the project's test command; use BrowserStack MCP only behind an "if available" branch (token fetch). If the user is on GitHub Copilot, apply `/percy:integrate` Step 4b scaffolding.
- **Don't invent SDK signatures.** For non-Node bindings (Python/Ruby/Java) confirm the exact Percy call against the docs before inserting — same rule as `/percy:integrate`.
