---
name: percy-expand-coverage
description: "Expand Percy visual coverage in a specific test file. Analyzes the file, finds logical touchpoints that lack a visual checkpoint (after an action, before an assertion, at distinct UI states), proposes percySnapshot() insertions, confirms with you, inserts them, and suggests a build. The 'day-2' companion to /percy:integrate for growing coverage as the app evolves."
argument-hint: "<path to a test file, e.g. tests/auth/login.spec.ts>"
---

# Percy Expand Coverage

Solves the "day-2" problem: a project already has Percy wired in, but coverage stagnates because adding `percySnapshot()` calls by hand is tedious. Point this skill at one test file and it proposes the visual checkpoints that file is missing.

For a project with **no** Percy yet, run `/percy:integrate` first.

## Steps

### 1. Resolve the target file

Use the path in `$ARGUMENTS`. If none was given, ask which test file to expand (don't guess across the whole suite). Confirm the file exists and is a functional test, not a config/helper.

### 2. Check eligibility

This workflow is for **Percy Web projects using the BrowserStack SDK snapshot path** — not Percy-on-Automate (PoA), and not projects that only use screenshots or the CLI `percy snapshot` URL path.

Detect pragmatically:
- `.percy/config.yml` exists (project is set up) and a `@percy/*` SDK is in the project deps.
- The suite already contains `percySnapshot(` calls via an SDK (confirms the SDK path, not screenshots).
- Optionally confirm against Percy: the project has **≥ 1 build** (`GET /builds?page[limit]=1`).

If the project isn't eligible (no builds yet, or it uses screenshots / CLI-URL snapshots rather than the SDK), **stop and warn** — don't insert calls that won't run:
> "Expanding coverage with AI is currently supported only for projects using `percySnapshot()` via the BrowserStack SDK (not screenshots). Run `/percy:integrate` first, or switch the project to the SDK snapshot path."

### 3. Identify the framework

From the file's imports + the project config, determine the framework so you insert the correct call signature (same table as `/percy:integrate`):

| Framework | `percySnapshot()` call |
|---|---|
| Playwright | `await percySnapshot(page, '<name>')` |
| Cypress | `cy.percySnapshot('<name>')` |
| WebdriverIO | `await browser.percySnapshot('<name>')` |
| Selenium | `await percySnapshot(driver, '<name>')` |
| Puppeteer | `await percySnapshot(page, '<name>')` |
| Nightwatch | `browser.percySnapshot('<name>')` |

Match the file's existing async/await and import style. If the file uses a non-Node binding (Python/Ruby/Java), use that binding's signature and confirm it against the docs.

### 4. Analyze the file for missing checkpoints

Read the file and map its test cases. For each, find the **logical visual touchpoints that lack a checkpoint**:
- **After a state-changing action** (navigation, form submit, modal open, tab switch) — the UI just reached a state worth capturing.
- **Before/around an assertion** — the test already cares about this state; capture it visually too.
- **At each distinct view/state** a test walks through (e.g. empty → filled → success).

Skip touchpoints that are **already covered** by an existing `percySnapshot()` nearby — don't duplicate. Skip pure setup/teardown and non-visual API steps.

### 5. Propose the insertions

Present a concrete list before touching the file:
> "I analyzed `login.spec.ts`. I can add `percySnapshot()` to: **'Login — empty state'** (after the page loads, test `displays login form`), **'Login — validation error'** (after submitting empty, test `shows error`). Proceed, or adjust (e.g. skip one)?"

Let the user approve directly or correct (exclude cases, rename, add one you missed).

### 6. Insert on confirmation

- Insert each approved call with the Step-3 syntax and a **descriptive, unique snapshot name** — incorporate the test/case name so names stay stable and meaningful (e.g. `percySnapshot('Login — validation error')`).
- Place it at the touchpoint (after the action / before the assertion), matching indentation and style.
- Add any required import/registration once if missing.
- Do **not** duplicate an existing snapshot or change unrelated code.

### 7. Summarize + offer next

Report what was added:
> "Added **3** `percySnapshot()` commands to `login.spec.ts`. Want me to expand another file, or run a build to verify?"

If the user is done adding files, suggest running a build (`percy exec -- <test command>`) and then `/percy:review`.

## Important

- **One file at a time.** This skill is scoped to the file the user names — don't fan out across the suite unasked.
- **Confirm before editing.** Always propose insertions and get approval first.
- **No duplicates.** Respect existing `percySnapshot()` calls; only fill genuine gaps.
- **Eligibility is a hard gate.** If the project isn't on the SDK snapshot path, warn and stop rather than inserting calls that silently won't capture.
- **Names matter.** Snapshot names are the baseline key in Percy — make them descriptive and stable, derived from the test/case.
