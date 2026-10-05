---
name: percy-integrate
description: "Integrate Percy into a project that already has a functional test suite but no visual testing yet. Detects the language + test framework, installs the matching BrowserStack Percy SDK + @percy/cli, wires config, inserts the first percySnapshot() calls (with your approval), and suggests a first build. Run this once per project to go from zero to visual coverage."
argument-hint: "[optional: project name, e.g. 'checkout-web']"
---

# Percy Integrate

The "day-0" onboarding skill (the Visual Test Integration Agent's initial-setup workflow). Takes a project that has a working functional test suite but **no Percy** and wires in visual testing end-to-end. After this, the review-loop skills (`/percy:visual-intent`, `/percy:review`, `/percy:gate`) have snapshots to work with.

For a project that **already** has Percy and just needs more coverage on a specific file, use `/percy:expand-coverage` instead.

## Eligibility (check first, stop if not met)

| Condition | Required |
|---|---|
| Project is **Percy Web** (browser snapshots) | ✅ — not App Automate / mobile, not Percy-on-Automate (PoA) |
| An **existing, functional test suite** runs (Playwright/Cypress/WDIO/Selenium/Puppeteer/Storybook, etc.) | ✅ |
| Percy **not already integrated** | ✅ — no `@percy/*` in deps and no `percySnapshot(` calls in the suite |

If Percy is already wired in → don't re-integrate. Point the user to `/percy:expand-coverage <test file>` to grow coverage. If there's no functional test suite at all, say so — this skill adds visual checkpoints to *existing* tests; it does not author a test suite from scratch.

## Steps

### 1. Detect the stack

Scan the repo to identify the language + test framework and pick the matching SDK:

| Project signal | Percy SDK to install (with `@percy/cli`) | `percySnapshot()` call to insert |
|---|---|---|
| `playwright.config.*` | `@percy/playwright` | `await percySnapshot(page, '<name>')` |
| `cypress.config.*` / `cypress.json` | `@percy/cypress` (add `import '@percy/cypress'` to the support file) | `cy.percySnapshot('<name>')` |
| `wdio.conf.*` | `@percy/webdriverio` | `await browser.percySnapshot('<name>')` |
| `selenium-webdriver` in deps | `@percy/selenium-webdriver` | `await percySnapshot(driver, '<name>')` |
| `puppeteer` in deps | `@percy/puppeteer` | `await percySnapshot(page, '<name>')` |
| `nightwatch.conf.*` | `@percy/nightwatch` | `browser.percySnapshot('<name>')` |
| `.storybook/` | `@percy/storybook` | (automatic — one snapshot per story; no inline calls) |
| Static site / list of URLs | `@percy/cli` only | `npx percy snapshot urls.yml` |

**Non-Node stacks** (Python/pytest, Ruby/Capybara, Java): `@percy/cli` (Node) is still the runner, but the snapshot binding differs — e.g. `percy-selenium` for Python/Java Selenium, `percy-playwright-python` for Python Playwright, `percy-capybara` for Ruby. Confirm the exact package + call signature against the Percy docs for that SDK before inserting; don't guess the binding.

Detect the package manager from the lockfile (`package-lock.json` → npm, `yarn.lock` → yarn, `pnpm-lock.yaml` → pnpm) and use it consistently.

### 2. Install the SDK

Propose the install command and run it on approval. Example (Playwright, npm):
```bash
npm install --save-dev @percy/cli @percy/playwright
```
Swap the binding package + package-manager per Step 1. Don't install if the dep is already present.

### 3. Ensure the Percy token

`percySnapshot()` calls are no-ops at runtime unless `percy exec` wraps the run with a valid `PERCY_TOKEN` — follow `/percy:setup` Step 1's credential flow for this (it covers account creation, project creation, and safe persistence to a gitignored secrets file); never suggest a shell-profile `export`, and never ask the user to paste a token into the chat.

### 4. Write config

- If there's no `.percy.yml` / `percy.config.js`, create a minimal one (snapshot widths, optional discovery settings). Keep defaults unless the project signals otherwise.
- Run `/percy:setup` (or replicate it) to write `.percy/config.yml` so the review-loop skills can read project state.

### 4b. Make the Percy commands available in the user's IDE (GitHub Copilot)

Claude Code / Cursor / Codex / Antigravity discover the skills automatically. **GitHub Copilot does not** — it reads a root `AGENTS.md` (which this plugin provides) plus `.github/prompts/*.prompt.md` for slash commands. If the user is on Copilot (or asks for it), scaffold those into their repo so `/percy-review`, `/percy-gate`, etc. work in Copilot Chat:

- Copy the plugin's bundled `copilot/prompts/*.prompt.md` → the user repo's `.github/prompts/`.
- Copy `copilot/copilot-instructions.md` → `.github/copilot-instructions.md`. If the user already has one, **append** the Percy section rather than overwriting it.
- Ensure an `AGENTS.md` exists at the repo root (Copilot reads it as primary instructions). If the project doesn't have one, add a short one that points at the Percy workflow; don't clobber an existing `AGENTS.md` — append a Percy section.

These artifacts are generated from the canonical skills by `scripts/gen-copilot.mjs` — never hand-edit them. Skip this step entirely for non-Copilot IDEs; it changes nothing about how the other IDEs use the plugin.

### 5. Insert the first `percySnapshot()` calls

Cover each **distinct page or UI state** the suite renders — dedupe near-identical states, but do **not** limit the selection to the primary user flow. A visual regression can surface on any page with a distinct layout — content/editorial pages, list/index pages, empty states, settings screens — so a funnel-only selection (e.g. home → cart → checkout) leaves the content pages unprotected, and a bug there ships silently. For each chosen test, find the logical visual checkpoint: **after a navigation or state-changing action, before/around the test's assertion** — the moment the UI is in the state the test cares about.

Sizing: for a small suite (≲8 tests) that usually means one snapshot per distinct page. For a large suite, one per distinct layout/state, collapsing repeats — aim for representative-**and-complete**, not minimal. If you deliberately skip a distinct page (e.g. it's visually identical to one already covered), say which and why, so the user can catch a gap.

- Present a proposed list before editing, grouped so gaps are obvious:
  > "I'll add `percySnapshot()` to: `home.cy.js` (landing page), `catalog.cy.js` (shop grid, product detail, cart), `pages.cy.js` (About story, Journal index, Contact form). That's every distinct page the suite renders. Proceed, or adjust?"
- On approval, insert using the Step-1 syntax for this framework, with a **descriptive, unique snapshot name** (incorporate the test/case name). Match the surrounding code style and import conventions.
- Add the framework's required import/registration once (e.g. the Cypress support-file import) if not already present.

### 6. Suggest the first build

Once snapshots are inserted, recommend kicking a build to validate end-to-end:
```bash
PERCY_TOKEN=<token> percy exec -- <the project's test command>
```
**Run the first build from the repo's default branch** (or confirm the Percy project's default base branch matches the branch you're on) — otherwise the first PR build has no baseline to diff against, and the whole first review is a confusing no-op.

Then `/percy:review` to confirm the snapshots came through. Hand off — don't poll here.

### 6b. Wire Percy into CI (the hands-off step)

The first build was local; day-to-day Percy should run in CI on every PR. Offer to wire it now — this is where the workflow becomes fully hands-off:

1. **Detect the CI provider**: `.github/workflows/` (Actions), `.circleci/config.yml`, `Jenkinsfile`, `.gitlab-ci.yml`.
2. **GitHub Actions (full hands-off path):**
   - Find the job that runs the test command and propose the minimal edit: wrap it with `npx percy exec -- <test command>` and add `env: PERCY_TOKEN: ${{ secrets.PERCY_TOKEN }}` to that step. Don't create a parallel workflow unasked.
   - **Set the secret without ever seeing it**: `gh secret set PERCY_TOKEN` reads the value from the user's local environment (already configured in Step 3) — shell → GitHub directly, nothing through the chat. Requires repo admin; if `gh` lacks permission, print the exact Settings → Secrets path instead.
   - Commit the workflow change on the current branch; on the next push, **verify the Percy check appears** on the PR (`gh pr checks`).
3. **Other CIs**: propose the equivalent config snippet and point at the provider's secret store (no programmatic set) — link Percy's CI/CD docs (browserstack.com/docs/percy/ci-cd/overview).

With this in place: push → CI runs Percy → `/percy:watch` sees the check → gate fires. No `/percy:run-build` needed again.

### 7. Finish with `/percy:setup`

Integration is not complete until setup finishes: credentials (including Step 1c's BrowserStack user creds for approve/reject), AI-tier detection, baseline state, and the workflow hook all live there. **Re-invoke `/percy:setup` now** — it picks up the freshly integrated project and closes the loop.

## Output

Summarize what changed:
> Integrated Percy (Playwright) — installed `@percy/cli @percy/playwright`, wrote `.percy.yml`, added 3 `percySnapshot()` calls across 3 tests. Run `percy exec -- npx playwright test` to capture the first build, then finish with `/percy:setup`.

If a step couldn't complete (e.g. token missing, ambiguous framework), surface the actual blocker — don't report success.

## Important

- **Web only.** If the project is App Automate / mobile / Percy-on-Automate, stop and explain that this agent supports Percy Web; point to standard setup docs.
- **Don't author tests.** This skill adds visual checkpoints to *existing* functional tests. If coverage is thin, that's `/percy:expand-coverage`'s job per file.
- **Confirm before editing code.** Always propose the snapshot insertions and get approval before writing to test files.
- **Cross-IDE.** Prefer `percy-cli` + the project's test command; use BrowserStack MCP only when it's available (token fetch, install helpers) behind an "if MCP available" branch.
- **Don't invent SDK signatures.** When unsure of a binding's exact call, check the Percy docs for that SDK rather than guessing.
