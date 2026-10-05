---
name: percy-troubleshoot
description: "Diagnose why Percy isn't working. Runs local diagnostics first (percy doctor, token validity + role, config sanity, SDK + browser checks), matches the result against the ~10 most common stable failures, and for everything else fetches the relevant BrowserStack docs page live at runtime. Docs-referencing by design — the skill carries symptom-category → docs-URL pointers, never copied docs content."
argument-hint: "[optional: the symptom, e.g. '403 on review', 'build stuck', 'no AI verdicts', 'snapshots failed']"
---

# Percy Troubleshoot

Figures out why Percy isn't working — a build that won't start, an API call that 401s, AI verdicts that never appear. Three layers, in order:

1. **Local diagnostics** — run `percy doctor` + targeted probes and read what the machine says.
2. **Known-failure table** — the ~10 most common, stable issues, fixed inline.
3. **Live docs** — for anything else, fetch the matching `docs.browserstack.com/docs/percy/` page *at runtime* and reason from it.

**Design rule — no embedded knowledge base.** Percy's docs change; a skill that copies them goes stale. This skill carries **URLs only** and fetches the page content fresh when needed. The only fixes written inline here are the small curated set in Step 3, chosen because they're stable product behavior (auth model, token roles, baseline mechanics) — not docs prose.

If the diagnosis points at a *build-launch* problem (browser, test runner, agent-shell env), the fix lives in `/percy:run-build` Steps 2–2c — reference and follow those steps; do **not** duplicate their content here.

## Steps

### 1. Scope the symptom

If `$ARGUMENTS` describes a symptom, note it — it picks which probes to prioritize and which docs category to fetch in Step 4. If empty, ask nothing yet: run the full local diagnostics and let the results speak.

### 2. Local diagnostics — always run these first

Evidence before docs. Every probe below is cheap and local.

**2a. `percy doctor` (if the CLI has it).** Available in `@percy/cli` ≥ 1.31.10; probe with `./node_modules/.bin/percy doctor --help` (exit 0 = available; guard: prefer the repo-local binary over `npx` — see `/percy:run-build` Step 2c). Then run:

```bash
./node_modules/.bin/percy doctor --quick          # connectivity + SSL + token, ~5-10s
```

Quick mode checks: Percy/BrowserStack API reachability, the Chromium-download CDN (`storage.googleapis.com`), SSL handshake, and token auth (it reports the **token type** — `web`/`app` — and **role** — `master`/`write_only`/`read_only`). **Don't stop at "All checks passed" — read the reported role.** A `write_only` token *passes* doctor's auth check (doctor prints: "✔ Token authentication successful — role: write_only … can create builds but cannot read results via the API") while every review-loop read 403s; if the role isn't `master`, that IS the diagnosis for a broken `/percy:review`/`/percy:gate` (table row 2), green summary line notwithstanding. If quick mode passes but the problem persists, escalate to the full run, which adds config-file detection, env audit, proxy/PAC/security-agent scan, and a live Chrome network capture:

```bash
./node_modules/.bin/percy doctor --output-json /tmp/percy-doctor.json
```

Parse the report (`checks.{config,envAudit,connectivity,ssl,proxy,pac,token,browser}.findings[]`, each with `category`, `status: pass|warn|fail|info`, `message`, `suggestions[]`). Surface every `warn`/`fail` finding with its suggestions verbatim — doctor's own remediation lines are current and specific (e.g. a blocked Chromium CDN suggests `PERCY_BROWSER_EXECUTABLE` or `PERCY_CHROMIUM_BASE_URL`).

**Always quote each finding's `category` code** (`token_auth_fail`, `config_parse_error`, `ci_parallel_nonce_missing`, `env_tls_disabled`, …) alongside the diagnosis — those codes are the vocabulary BrowserStack support triages by, so a user who escalates can paste them straight into the ticket and skip a diagnostic round-trip.

Doctor launches a browser for the network-capture check — in an Electron-IDE agent shell, run it with the same env hygiene as a build: `env -u ELECTRON_RUN_AS_NODE …` (`/percy:run-build` Step 2c). Note: `percy doctor` is **not yet on the public docs site** — don't hunt for a docs page for it; its own output is the reference.

**CI tip:** suggest `PERCY_AUTO_DOCTOR=true` on the CI test step — on a build failure percy-cli then auto-runs the quick diagnostic (~4s, connectivity + token) and prints the findings inline in the CI log, answering "was that a network/token issue?" before anyone files a ticket.

**2b. Token present + valid + right role.** If doctor is unavailable (older CLI), probe by hand:

```bash
curl -sS -o /dev/null -w "%{http_code}" -H "Authorization: Token token=\"$PERCY_TOKEN\"" \
  -H "User-Agent: curl/8.7.1" "https://percy.io/api/v1/builds?page%5Blimit%5D=1"
```

- No `PERCY_TOKEN` in env → check the project's secrets files (same search order as `/percy:setup` Step 1) before declaring it missing.
- `401` → token invalid/revoked.
- `200` for `percy exec` but `403` on **reads** (builds/snapshots/comparisons) → the token is **`write_only`** — uploads work, the review loop breaks. Fix: use the project's `master`-role token (`/percy:setup` Step 1 selects it by `attributes.role == "master"`).
- Keep the `User-Agent: curl/8.7.1` header on every REST call. Percy's edge may block unrecognized User-Agents (a 403 indistinguishable from an auth failure), and the block can be **intermittent** — it varies by network/IP. Diagnostic order on a 403: retry with the header; if the 403 persists, it's the token role, not the UA.

**2c. Config sanity.** `./node_modules/.bin/percy config:validate` (validates `.percy.yml` against the schema — surfaces unknown keys, wrong types, deprecated options; `percy config:migrate` upgrades old formats). Also confirm `.percy/config.yml` exists (that's the *plugin's* state file, distinct from `.percy.yml` — if missing, `/percy:setup` hasn't run).

**2d. SDK installed + actually called.** The framework SDK must be present alongside `@percy/cli` (`@percy/playwright`, `@percy/cypress`, `@percy/storybook`, `@percy/puppeteer`, `@percy/selenium-webdriver`, …). A suite that runs green but produces **zero snapshots** usually means the SDK import/registration is missing, not broken. Percy makes this diagnosable: when a `percy exec` run takes no snapshots, the build **fails with a specific reason** — `Snapshot command was not called` (surfaced in the CLI output and on the build's `failure-reason`), with a docs link Percy emits inline. So the diagnosis for "Percy set up but no snapshots appear" is almost always: the SDK isn't wired into the specs — confirm the suite imports it (e.g. `import '@percy/cypress'` in the Cypress support file, `import percySnapshot from '@percy/playwright'` in Playwright specs) **and** actually calls `percySnapshot(...)`. Installing the package is not enough on its own.

**2e. Browser + agent-shell poisons.** Symptoms: percy can't launch its Chromium; Cypress dies with `Cannot find module …Contents/MacOS/Contents/Resources/app/index.js`; bare `npx` dies with a package-proxy shim error. All three are diagnosed and self-healed in **`/percy:run-build` Steps 2, 2b, 2c** — follow those (browser resolution order, inline `PERCY_BROWSER_EXECUTABLE`, `env -u ELECTRON_RUN_AS_NODE`, direct binaries over `npx`). Never conclude "the test runner is broken" or "local builds are impossible" from these signatures.

### 3. Known-failure table — match before fetching docs

The stable, high-frequency issues. If a probe result or the user's symptom matches a row, apply the fix directly — no docs fetch needed.

| # | Symptom | Cause | Fix |
|---|---|---|---|
| 1 | Every API call `401` | `PERCY_TOKEN` missing/invalid/revoked | Step 2b; re-copy from Percy project settings via `/percy:setup` |
| 2 | Uploads work, review-loop reads `403` | Token role is `write_only` | Switch to the `master`-role token (`/percy:setup` Step 1) |
| 3 | `403` with a *valid* token, esp. from scripts | Cloudflare blocking the client's User-Agent — **intermittent** (varies by network/IP reputation) | Retry the identical call with `User-Agent: curl/8.7.1`: 403 persists → it's the token role (row 2), not the UA. Keep the header on all calls regardless — free insurance |
| 4 | `401 No user found` on approve/reject or network-logs | Those endpoints need BrowserStack user creds, not the project token | `PERCY_USERNAME` + `PERCY_ACCESS_KEY` via `/percy:setup` Step 1c |
| 5 | Every build shows every snapshot as new / diffs never appear | No approved baseline, or `default-base-branch` ≠ repo default (Percy defaults to `master`) | Approve the baseline build; `PATCH` the project's `default-base-branch` (`/percy:setup` Step 1) |
| 6 | Diffs exist but no AI verdicts on a **large** build | AI summary skipped: `ai-details.summary-status: "skipped"`, `summary-reason: "too_many_comparisons"` | Expected behavior — `/percy:review` falls back to per-comparison scan; split the suite if summaries matter |
| 7 | No AI verdicts on a normal-size build | Cold start (baseline unapproved), free tier, or non-AI browser pool | `/percy:review` cold-start Cases A/B first; then its NO-AI cause table |
| 8 | `Some snapshots failed` / snapshots missing from build | Asset discovery: network-idle timeout, slow/authenticated assets, >20MB resource | Fetch docs category **capture-failures** (Step 4) — remedies are config-specific (`allowed-hostnames`, idle timeout, asset auth) |
| 9 | CI never creates a build; logs show "Skipping visual tests" / missing token | `PERCY_TOKEN` not set as a CI secret, or not exposed on the test step | GitHub → Settings → Secrets → Actions → `PERCY_TOKEN`; `env:` on the test step (README "In CI") |
| 10 | Same region diffs every build (ads, carousels, timestamps, feeds) | Dynamic content, not a regression | Ignore/Intelli-Ignore regions — fetch docs category **dynamic-content** for current region options; `/percy:review` Step 6b explains the history-based detection |
| 11 | Browser/test-runner won't launch in the agent shell | Corporate-proxy-blocked Chromium CDN, `ELECTRON_RUN_AS_NODE`, npx shims | `/percy:run-build` Steps 2–2c (self-heals inline there) |
| 12 | SSL/certificate errors reaching percy.io (doctor: SSL FAIL) | Corporate proxy intercepting HTTPS | Point `NODE_EXTRA_CA_CERTS` at the proxy's CA cert (get it from the network admin). Never recommend `NODE_TLS_REJECT_UNAUTHORIZED=0` — it disables TLS verification entirely (MITM exposure); if it's already set, doctor flags it as `env_tls_disabled` — tell the user to remove it and use the CA cert instead |

### 4. Everything else — fetch the docs live

Map the symptom to a category, fetch 1–2 of its URLs, and reason from the **live page content** plus the local evidence from Step 2. Quote the docs' remediation, cite the URL, and say which diagnostic finding it explains.

**How to fetch, per IDE:** in Claude Code, use WebFetch. In other IDEs, use the environment's web-fetch/browse capability if it has one. If the IDE can't fetch the web, print the mapped URL(s) as Markdown links and summarize what the user should look for on the page — never pretend to have read a page you couldn't fetch.

**Symptom-category → docs-URL map** (URLs only — content lives on the docs site; checked 2026-07):

| Category | When | URLs |
|---|---|---|
| **troubleshoot-hub** | Fallback when nothing narrower matches | https://www.browserstack.com/docs/percy/troubleshoot/percy-common-faqs (30 common issues + 15 error messages, per-SDK sections) · https://www.browserstack.com/docs/percy/troubleshoot/self-debug |
| **auth-tokens** | Token errors beyond table rows 1–4 | https://www.browserstack.com/docs/percy/common-issue/common-errors/missing-percy-token · https://www.browserstack.com/docs/percy/common-issue/common-errors/incorrect-project-type (token/project-type mismatch) · https://www.browserstack.com/docs/percy/references/set-env-var |
| **baseline-branches** | Wrong/missing baseline, branch strategy, `PERCY_TARGET_BRANCH` | https://www.browserstack.com/docs/percy/visual-testing-workflows/baseline-management/overview · https://www.browserstack.com/docs/percy/visual-testing-workflows/baseline-management/git |
| **browser-launch** | Percy's Chromium won't start (esp. CI/Docker) | https://www.browserstack.com/docs/percy/common-issue/common-errors/browser-launch-failure · https://www.browserstack.com/docs/percy/common-issue/common-errors/browser-spawn-failure · https://www.browserstack.com/docs/percy/common-issue/chromium-failed-to-launch-browser |
| **capture-failures** | Upload/asset failures, timeouts, blank or broken snapshots | https://www.browserstack.com/docs/percy/common-issue/common-errors/upload-snapshot-failure · https://www.browserstack.com/docs/percy/common-issue/common-errors/network-idle-timeout · https://www.browserstack.com/docs/percy/common-issue/common-errors/page-load-failed · https://www.browserstack.com/docs/percy/common-issue/common-errors/large-payload · https://www.browserstack.com/docs/percy/troubleshoot/network-logs |
| **dynamic-content** | Recurring noise, animations, lazy-loaded content | https://www.browserstack.com/docs/percy/set-regions/define-regions · https://www.browserstack.com/docs/percy/stabilize-screenshots/animations · https://www.browserstack.com/docs/percy/stabilize-screenshots/lazy-loading · https://www.browserstack.com/docs/percy/troubleshoot/lazy-loading-issues |
| **ci-setup** | CI wiring, PR status checks, parallel suites / "build not finalized" | https://www.browserstack.com/docs/percy/integrations/ci-cd/github-actions · https://www.browserstack.com/docs/percy/integrations/source-control/github · https://www.browserstack.com/docs/percy/troubleshoot/parallel-test-suites |
| **ai-review** | AI toggles, noise reduction, review-agent behavior | https://www.browserstack.com/docs/percy/ai-agents/visual-review-agent/overview |
| **sdk-install** | SDK/framework integration problems; "Snapshot command was not called" / zero snapshots (the per-framework get-started page shows the exact `percySnapshot()` call to wire in; also follow the link Percy's own failure message emits) | https://www.browserstack.com/docs/percy/playwright/get-started · https://www.browserstack.com/docs/percy/cypress/get-started · https://www.browserstack.com/docs/percy/storybook/get-started · https://www.browserstack.com/docs/percy/selenium/get-started · https://www.browserstack.com/docs/percy/integrate-bstack-sdk/webdriverio (WDIO goes via the BrowserStack SDK) |
| **cli-reference** | What a CLI command/flag does | https://www.browserstack.com/docs/percy/references/commands |
| **networking-local** | localhost/IPv6 binding issues | https://www.browserstack.com/docs/percy/common-issue/common-errors/networking-binding-issue |

**Staleness guard:** the Percy docs are periodically restructured (old paths 301 to new ones — e.g. `baseline-management/*` → `visual-testing-workflows/baseline-management/*`). If a URL 404s, don't give up: search the docs (`site:browserstack.com/docs/percy <symptom>` via web search, or the docs site's own search) and use the current page. If a moved URL is found, note it so the map can be updated in the plugin repo.

### 5. Report

Structure the output as: **diagnosis → evidence → fix → source.**

> **Diagnosis:** review-loop reads are failing because the configured token is `write_only`.
> **Evidence:** `percy doctor --quick` → token auth OK, role `write_only`; `GET /builds` probe → 403.
> **Fix:** switch to the project's `master` token — `/percy:setup` Step 1 selects it via `attributes.role == "master"`.
> **Source:** local probes (no docs fetch needed).

When docs were fetched, cite them: `**Source:** [Error - Network idle timeout](https://www.browserstack.com/docs/percy/common-issue/common-errors/network-idle-timeout) (fetched live).` If several findings exist, report each separately, worst first. If nothing conclusive was found, say so, share the doctor JSON highlights + the most relevant docs link, and suggest escalating to BrowserStack support **with the pre-support bundle their triage expects**: the finding's `category` code + status, the exact doctor message, what remediation was attempted, and `report.json` from `--output-json` (plus `-v` logs if asked). That's the handoff shape support triages fastest.

## Important

- **Local evidence first, docs second, guessing never.** Every diagnosis must cite a probe result or a fetched docs page.
- **URLs only in this skill.** Never paste docs content into this file when updating it — fetch at runtime. Curated-table rows are limited to stable product behavior.
- **Don't duplicate sibling skills.** Browser/test-runner launch problems → `/percy:run-build` Steps 2–2c. Credential setup → `/percy:setup` Steps 1/1c. Cold-start/NO-AI interpretation → `/percy:review`. This skill routes to them.
- **Never print tokens** — report presence/validity/role only.
- **`percy doctor` may be absent** on older CLIs — feature-detect with `--help`, fall back to the manual probes in Step 2b–2e, and suggest upgrading `@percy/cli`.
