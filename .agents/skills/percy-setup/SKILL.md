---
name: percy-setup
description: "One-time Percy setup for this project. Verifies BrowserStack credentials, registers the project, detects paid-tier access for AI classification, and writes a local config the other Percy skills read from. Run this first."
---

# Percy Setup

One-time configuration so the other Percy skills (`/percy:visual-intent`, `/percy:review`, `/percy:gate`, `/percy:run-build`, `/percy:status`) can work without re-asking the user every time.

## What this skill does

1. Verifies `PERCY_TOKEN` is available (project-scoped token).
2. Detects whether the BrowserStack MCP server is configured for this IDE (informational — v0.1 uses REST directly anyway).
3. Probes the user's tier — does this account have Percy AI classification?
4. Persists detected state to `.percy/config.yml` (gitignored).

## Steps

### 0. Detect where this project is — and route

`/percy:setup` is the single entry point; figure out the project's state before anything else, so the user never has to know which skill they needed:

| Signal | State | Route |
|---|---|---|
| No `@percy/*` dep, no `percySnapshot(` calls anywhere | **Not integrated** — has tests but no visual coverage | Say so, and offer to run `/percy:integrate` right now (it installs the SDK, wires config, inserts the first snapshots — then returns here to finish setup). |
| Percy integrated, but no builds / no approved baseline | **Cold start** | Continue setup; Step 4b explains the baseline flow and offers to kick the first build. |
| Integrated + builds + approved baseline | **Ready** | Continue setup normally — verify creds, detect AI tier, write config, hook the workflow (Step 6). |

One command in, the right journey out — "new to Percy" and "existing Percy user" both just run `/percy:setup`.

**Also determine the repo's role + regression branch — but only ask when a signal says to.** Downstream skills behave differently per role, so stamp it in config. Default silently to `repo_role: app`, `regression_branch: null` unless a signal is detected; **NEVER ask these questions on a cold-start or not-integrated project** (a user who hasn't run their first build has no regression flow to describe — the questions just read as noise):
- **Role signal:** a suite-only repo is mostly test files with no app source; an app repo has the app + tests. Only ask "app repo, regression suite, or both?" when the tree genuinely looks suite-only (or mixed) — otherwise stamp `repo_role: app` without asking.
- **Regression-branch signal:** branch names matching `pre.?prod|qa|staging|regression|release`; a non-default branch with recurring **non-PR** Percy builds (`GET /builds?filter[branch]=`); the project's `auto_approve_branch_filter` / default-base-branch settings. **Check CI files too** — the regression branch may only exist as a `PERCY_BRANCH` env var in the CI config, not as a local git branch. If a signal hits, confirm with the user once and persist as `regression_branch`; if nothing hits, stamp `null` silently.
- **App repos (regression-suite only).** When `repo_role: regression-suite` (or `both` with a separate app), setup MUST ask: "which repo(s) does the app code live in?" — regression reviews harvest merged-PR intent from those repos, so the plugin can't guess them. Stamp the answer as `app_repos: [owner/name, ...]` in config.

### 1. Check credentials — and guide a new user all the way

**Fast path — BrowserStack credentials present? Skip the browser entirely.** If `PERCY_USERNAME` + `PERCY_ACCESS_KEY` are already in the env/secrets (common on teams, CI machines, and demos), the whole token journey collapses into one question — the Percy API can list projects, create one, and fetch its token under those creds:

1. **List** the account's organizations and Percy projects via the API (Basic auth with the BrowserStack creds). Exact endpoints — don't rediscover these:
   ```
   GET  /api/v1/user/organizations                      # the account's orgs
   GET  /api/v1/organizations/<org_id>/projects         # projects in an org
   ```
2. **Recommend deterministically — never loop:**
   - Recommend **reuse** ONLY on a strong match: a project whose **name** (`attributes.name`) matches this repo, or one already referenced by `.percy.yml` / `.percy/config.yml`. A **slug**-only match is NOT strong — slugs are immutable, so a renamed/archived project keeps its old repo-like slug forever (e.g. `zz-archived-…` projects); matching on it resurrects dead projects and their tokens. Recent activity alone is also NOT grounds to recommend reuse (an active project may be someone else's — wiring builds into it is the worst failure mode). Show recently-active projects as *options* with last-build timestamps.
   - Otherwise recommend **create new**, named after the repo — safe default.
3. **Ask exactly once:** "Recommended: create `<repo-name>` (new). Or reuse: `<match>` (matches repo name, last build <when>). Which?" One answer, no follow-ups.
4. **Create/select the project via the API, fetch its `PERCY_TOKEN`**, and write it directly to the gitignored secrets file — never print the token in chat. Exact contract (the create route is **org-scoped by SLUG**, and `type` is a required *attribute* meaning the project kind — NOT the JSON:API top-level type):
   ```
   POST /api/v1/organizations/<org_slug>/projects
   Content-Type: application/vnd.api+json
   {"data": {"type": "projects", "attributes": {"name": "<repo-name>", "type": "web"}}}

   GET  /api/v1/projects/<project_id>/tokens            # read the new project's PERCY_TOKEN
   ```
   Use the org **slug** (`attributes.slug` from `/user/organizations`) in the create path — the numeric org id returns `403 forbidden` here, so don't retry with the id or treat the 403 as a permissions problem.
   **After creating, set the project's default base branch to the repo's default branch** (Percy defaults to `master`; most repos are `main` — mismatch breaks baseline comparison). For `repo_role: regression-suite`, use the configured **`regression_branch`** instead — the nightly builds live there and must baseline against each other, not against a `main` that never builds:
   ```
   PATCH /api/v1/projects/<project_id>
   {"data": {"type": "projects", "id": "<project_id>", "attributes": {"default-base-branch": "<repo default branch>"}}}
   ```
   **Pick the `master` (full-access) token** from that list — token roles are `master`, `write_only`, `read_only`. The plugin's `PERCY_TOKEN` must be the master token: `write_only` works for `percy exec` uploads but **403s on the review loop's reads** (builds/snapshots/comparisons), which breaks `/percy:review`/`/percy:gate`. Don't probe-and-swap — select `attributes.role == "master"` directly.
   Valid `attributes.type` values: `web` (this plugin's case), `app`, `automate`, `generic`. A 400 "param is missing: type" means the type attribute is absent/misplaced — it belongs inside `attributes`. POSTing to bare `/api/v1/projects` is not the create route.

Only when BrowserStack creds are absent does the browser walkthrough below apply.

**Check for an existing token first** — in this order, stop at the first hit:
1. The shell environment (`PERCY_TOKEN` already exported).
2. The project's local secrets files — follow the suite's own convention: `.env`, `.env.local`, `.env.test`, `.envrc` (direnv), `.percy/.env`, `cypress.env.json` (Cypress suites) — grep for `PERCY_TOKEN`. If found but not loaded, wire the loading rather than asking for the token again.

**If none found, walk the user through getting one** — sized to where they are:

**Brand new to Percy (no project yet):**
1. Sign in at [percy.io](https://percy.io) with your BrowserStack account (create one if needed — free tier exists).
2. **Create a Web project** (New Project → name it after this repo).
3. Open **Project Settings** → copy the **`PERCY_TOKEN`** shown there (it's a write-only, per-project token).

**Has a Percy project:** dashboard → the project → Project Settings → copy `PERCY_TOKEN`.

**Persist it locally — the agent must never see the token.** Preference order:
1. **Existing project secrets file first.** If the project already has one (`.env`, `.env.local`, `.envrc`, `.percy/.env`), add a `PERCY_TOKEN=` placeholder line **there** — it's already part of the project's conventions and loading. Verify it's gitignored before touching it; if it isn't, warn and don't put a secret in it.
2. **No secrets file? Create a gitignored `.env`** with the placeholder, ensure `.gitignore` covers it, and wire loading if the project has a convention (direnv, dotenv, `source .env` in the test script).
3. **Shell profile — last resort only** (pollutes the user's global shell with a per-project secret): the user adds `export PERCY_TOKEN=web_xxx` themselves.

In every path the user pastes the actual value **in their editor**, never in chat; the agent writes only the placeholder, then verifies via the probe call.

**Never put the token in `.percy.yml` or any committed file.** `.percy.yml` is *snapshot configuration* (widths, ignore-regions — created via `percy config:create`) and is committed to the repo; the token is a secret and lives only in the environment (locally) or a CI secret (in CI).

**CI (optional, for hands-off day-to-day runs):** the same token goes in the CI provider's secret store — e.g. GitHub → repo **Settings → Secrets and variables → Actions → New secret → `PERCY_TOKEN`** — and the workflow exposes it on the test step:
```yaml
- run: npx percy exec -- <test command>
  env:
    PERCY_TOKEN: ${{ secrets.PERCY_TOKEN }}
```
Guardrails on this — it is a convenience, **never a prerequisite**:
- **Not a fallback for local-build trouble.** If a local build fails to launch the browser, that is a percy-Chromium issue (`/percy:run-build` Step 2 resolves it), **not** a reason to set up CI. Never reason "local won't work → wire CI." For local or demo runs, `/percy:run-build` is the intended path.
- **Only wire CI if the repo already has `.github/workflows/`** — add the env line to the existing test job. Do **not** create a new/parallel workflow unless the user explicitly asks.
- **Never write the token as a secret non-interactively** (`gh secret set …` with a real token). Direct the user to paste it in the provider UI, or scaffold the workflow and let them add the secret themselves — a master token must not be piped into a secret store by the agent.

Test it with a probe call:
```
curl -sS -H "Authorization: Token token=\"$PERCY_TOKEN\"" -H "User-Agent: curl/8.7.1" \
  "https://percy.io/api/v1/builds?page%5Blimit%5D=1"
```
A 200 response confirms the token works. 401/403 → bad token. The `User-Agent: curl/8.7.1` header is required — Percy's Cloudflare layer blocks default UAs.

### 1b. Check GitHub access (gh CLI)

Three workflows lean on GitHub: `/percy:visual-intent` (writes the intent section into the PR), `/percy:watch` (Path B watches the PR's Percy check), and regression reviews (harvesting merged-PR intent). Check `gh auth status` now, once, so the user isn't surprised mid-flow:

- **Authenticated** → note it in config and move on.
- **Missing/unauthenticated** → say what still works and what degrades — the plugin stays functional: intent is saved to `.percy/intent-<branch>.md` instead of the PR, watch uses direct Percy-API polling (Path A), and regression harvest falls back to commit messages. Offer `gh auth login` to unlock the full flow; don't block setup on it.

### 1c. BrowserStack user credentials (approve/reject)

Approving/rejecting builds and smart-debug reads use these — one-time. `/percy:gate`'s approve/reject (`POST /reviews`) and `/percy:review`'s network-logs probe authenticate with **`PERCY_USERNAME` + `PERCY_ACCESS_KEY`** (HTTP Basic), not the project `PERCY_TOKEN` — a project token returns 403 on those endpoints.

**Check for existing creds first** — same order as Step 1: the shell environment (`PERCY_USERNAME` + `PERCY_ACCESS_KEY` already exported), then the same project secrets files (`.env`, `.env.local`, `.env.test`, `.envrc`, `.percy/.env`, `cypress.env.json`) — grep for both names. If found but not loaded, wire the loading rather than asking again.

**If missing, guide the user:** BrowserStack Dashboard → **Account** → **"Access Key"** (the username is shown alongside it). Persist `PERCY_USERNAME=` / `PERCY_ACCESS_KEY=` placeholder lines to the **same gitignored secrets file** Step 1 used — the user pastes the actual values **in their editor**, never in chat; the agent writes only the placeholders.

**Verify with an authed probe:**
```
curl -sS -o /dev/null -w "%{http_code}" -u "$PERCY_USERNAME:$PERCY_ACCESS_KEY" -H "User-Agent: curl/8.7.1" \
  "https://percy.io/api/v1/network-logs?comparison_id=<any recent comparison id>"
```
Any **non-401** response confirms the creds are live (a 404/422 on a stale comparison id still proves auth). If the project has no comparison yet (cold start), skip the probe and say so — the creds get exercised on the first gate action.

Stamp the result as `write_creds_verified: true | false` in `.percy/config.yml` (Step 5). Don't block setup on it — reads work with `PERCY_TOKEN` alone; `/percy:gate` will just point back here when it needs to approve/reject.

### 2. Note MCP availability (informational)

This plugin's v0.1 calls REST directly. If the BrowserStack MCP server is configured for this IDE, note it in the config (we'll switch the plugin to MCP-primary in v0.2). MCP-availability does not change v0.1 behavior.

If the user wants to install MCP anyway:
> ```json
> {
>   "mcpServers": {
>     "browserstack": {
>       "command": "npx",
>       "args": ["-y", "@browserstack/mcp-server@1.2.15-beta.2"],
>       "env": {
>         "BROWSERSTACK_USERNAME": "<username>",
>         "BROWSERSTACK_ACCESS_KEY": "<access_key>"
>       }
>     }
>   }
> }
> ```

### 3. Identify the project

From the probe response in step 1, the first build's relationships tell you the project. Or extract `attributes.web-url` (looks like `https://percy.io/<org>/<project-slug>/builds/<id>`) and parse the slug.

Store the project slug. Skills downstream use it for branch-scoped queries.

### 4. Detect AI tier

Find one recent finished build:
```
GET /api/v1/builds?filter[branch]=<any active branch>&page[limit]=5
```

Pick the most recent with `attributes.state === "finished"`. Inspect `attributes.ai-details`:

| Signal | Verdict |
|---|---|
| `total-comparisons-with-ai > 0` OR `summary-status === "ok"` | **AI enabled** ✅ |
| `total-comparisons-with-ai === 0` AND `summary-status` missing | likely AI off / free tier |
| No recent builds | ask user once |

**Do not gate on `ai-details.ai-enabled`** — that field is the AI-auto-approve toggle, not whether AI is available. Builds with `ai-enabled: false` still produce AI classification.

### 4b. Detect baseline state (cold start)

Percy's AI verdicts only mean something once there's an **approved baseline** to diff against. Detect whether this project is cold (no baseline yet) so the review loop can explain it instead of returning a confusing NO-AI result later.

From the recent-builds probe (Step 4):
- **No builds at all** → cold start. The first build will *be* the baseline (no diffs, no AI by design).
- **Builds exist but none is approved** — check the most recent finished build: `relationships.base-build.data` is null, or `attributes.review-state` is `unreviewed` / `attributes.approved-at` is null across recent builds → no approved baseline yet.
- **A recent build is `review-state: approved` with a non-null `base-build`** → baseline established; AI diffs will work normally.

Persist the result and, if cold, tell the user up front:
> ⚠️ No approved baseline yet. Your first Percy build establishes the baseline — approve it in Percy, then subsequent builds get real visual diffs + AI classification. `/percy:review` will guide you through this.

### 5. Write `.percy/config.yml`

```yaml
project_slug: <slug>
project_web_url: <web-url base>          # e.g. https://percy.io/<org>/web/<project>
ai_classification_enabled: true | false  # has Percy AI tier been used on this project
has_approved_baseline: true | false      # false = cold start; first build will be the baseline
write_creds_verified: true | false       # BrowserStack user creds (approve/reject + smart-debug) probed OK — Step 1c
repo_role: app | regression-suite | both # how the PR loop vs regression triage applies here
regression_branch: <name> | null         # branch scheduled/regression runs build on (e.g. pre_prod)
app_repos: [owner/name, ...]             # regression-suite only: repo(s) the app code lives in (intent harvest)
mcp_available: true | false              # informational; plugin uses REST regardless
detected_at: <ISO timestamp>
```

If `ai_classification_enabled: false`, warn the user that `/percy:review` and `/percy:gate` will degrade to NO-AI mode (surfacing diff counts without classification).

> Note: the plugin is Percy-AI-native — verdicts run on Percy's AI text outputs (`change_reason`, `change_description`) reconciled against the developer's code context. No additional host-agent capability is required.

### 6. Hook the loop into the repo's agent workflow (the actual "install")

Writing config isn't enough — nothing yet makes the visual loop *happen* on the next UI change. The hook is the **repo's agent-instructions file** (`CLAUDE.md` / `AGENTS.md`), which every host agent reads on every session. The hook is **role-aware** — pick the section matching `repo_role` and offer to append it (append, never clobber other content).

**For `repo_role: app`** (the PR loop):

```markdown
## Visual review (Percy)
<!-- percy-hook v2 -->

This repo uses the Percy Visual Testing plugin. When you change UI code:
1. Before/while opening the PR, run `/percy:visual-intent` (it injects the intent section and auto-starts `/percy:watch`).
2. The watcher runs `/percy:gate` automatically when the Percy build finishes — don't merge UI changes before the gate passes.
3. If you skipped intent and a build already exists, run `/percy:gate` directly.
4. If this repo's CI doesn't run Percy, run `/percy:run-build` after pushing — the watcher can't watch a build that never starts.
```

**For `repo_role: regression-suite`** (substitute the configured `regression_branch`):

```markdown
## Visual review (Percy)
<!-- percy-hook v2 -->

This repo is a Percy regression/QA suite. Morning triage: run `/percy:gate` — it resolves the latest build on `<regression_branch>`, classifies every diff (merged app-PR intent / dynamic noise / anomaly), and one confirm approves an all-explained run.
```

(`repo_role: both` → include both blocks under the one `<!-- percy-hook v2 -->` heading.)

**Upgrade, don't skip.** The `<!-- percy-hook v2 -->` comment is the version marker. If the file already has a "Visual review (Percy)" section that is **older or markerless** (e.g. written by `install.sh` or a previous setup run), **replace that section in place with the v2 block above** — don't skip it as "already present," and don't append a duplicate. Only skip when the existing section already carries the current marker.

With this in place the workflow is hands-off: the developer (or their agent) just changes UI code and opens a PR — intent, watching, review, and gating chain automatically. Without it, every step is a manual invocation.

## Output

Print a one-line summary:
> Percy setup complete. Project: `<slug>`. AI classification: enabled. Workflow hook: written to CLAUDE.md. MCP: available.

If anything failed, surface the actual blocker — do not pretend setup succeeded.
