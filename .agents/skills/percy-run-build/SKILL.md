---
name: percy-run-build
description: "Kick a Percy build for the current change without waiting for CI. Optional — only needed when the project's CI doesn't already run Percy, or for in-loop development before pushing. Prefers percy-cli + the project's existing test command."
argument-hint: "[optional: test command to wrap, e.g. 'npx playwright test']"
---

# Percy Run Build

Kicks a Percy build for the current working copy. Most projects already run Percy in CI on PR — only use this skill when:
- You're iterating locally and want a Percy build *before* pushing.
- The repo has no CI integration for Percy.
- You're rerunning after a fix during a `/percy:gate` iteration.

## Prerequisites

- `.percy/config.yml` exists (run `/percy:setup` first).
- `PERCY_TOKEN` is available — pull it from the project record, or instruct the user to set it.
- A Percy CLI is installed (`npm install --save-dev @percy/cli`) OR the BrowserStack MCP server is available.

## Steps

### 1. Determine how to capture snapshots

Pick the right capture method:

| Project signal | Use |
|---|---|
| Has `playwright.config.*` | `percy exec -- npx playwright test` |
| Has `cypress.config.*` | `percy exec -- npx cypress run` |
| Has `.storybook/` | `npx @percy/storybook` |
| URLs in `.percy.yml` or `percy.config.js` snapshot list | `npx percy snapshot urls.yml` |
| None of the above | Ask user for a test command, or for URLs to snapshot |

### 2. Resolve the browser FIRST (proactive — do not wait for a failure)

percy-cli renders snapshots in a headless browser **it launches itself**. Its *bundled* Chromium is routinely blocked/stubbed by corporate package proxies, so a bare `percy exec` fails to launch the browser on many machines. Resolve a real browser up front and pass it **inline on the build command**.

**Critical — env scoping.** `PERCY_BROWSER_EXECUTABLE` must be in the environment of the *exact process* that runs percy. An export in an **interactive** shell profile (`~/.zshrc`, `~/.bashrc`) does **NOT** reach the non-interactive subprocess percy spawns — this is the #1 cause of "it worked in my terminal but the agent's build fails." Put it **inline on the command** (below) or in `~/.zshenv` (sourced by every zsh, interactive or not).

Resolve the path in this order:
1. `browser_executable` in `.percy/config.yml`, if present and still executable.
2. `PERCY_BROWSER_EXECUTABLE` already set to an executable in the current env.
3. The test framework's own browser (already proven on this machine): Playwright → `node -e "console.log(require('playwright').chromium.executablePath())"`; Puppeteer → `node -e "console.log(require('puppeteer').executablePath())"`. (Cypress bundles Electron — no reusable Chromium; fall through.)
4. System Chrome — first that exists: macOS `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`; Linux `google-chrome`/`chromium` on PATH; Windows `C:\Program Files\Google\Chrome\Application\chrome.exe` (Edge as secondary).

**Persist the resolved path to `.percy/config.yml` (`browser_executable: <path>`)** so every future build reads it — this is repo-local, needs no home-dir write, and reaches the subprocess because step 3 below always passes it inline.

### 2b. Run the build

**Preferred — via percy-cli, browser + token inline:**
```bash
PERCY_BROWSER_EXECUTABLE="<resolved path>" PERCY_TOKEN=<token> percy exec -- <test-command>
```
Passing the browser inline (not relying on shell profiles) is what makes this deterministic across the user's terminal, CI, and the agent's own shell.

**Alternative — via MCP:** if percy-cli isn't installed but MCP is available, call `percy_create_build` with the appropriate params:
- For URL list: `percy_create_build project_name=<slug> urls=<comma-separated-list> widths=<from config>`
- For test command: `percy_create_build project_name=<slug> test_command=<cmd>` (MCP shells out to percy-cli)

**If it STILL fails to launch** after a correctly-resolved inline browser, surface Percy's [browser-launch-failure doc](https://www.browserstack.com/docs/percy/common-issue/common-errors/browser-launch-failure) and ask. Do **not** conclude "the test-runner binary is a stub" or "local builds are impossible" — a browser-launch error is about percy's Chromium, never the test runner, and is **not** a reason to fall back to CI setup.

### 2c. Test-runner launch failures in agent/IDE shells

Agent shells inside Electron-based IDEs (VS Code, Claude Code desktop, Cursor) and machines with package-proxy shims produce launch failures that look like a broken test runner — **none of them mean the runner is broken, and none justify CI fallback**:

- **Electron-runner death: "Cannot find module …Cypress.app/Contents/MacOS/Contents/Resources/app/index.js"** (note the doubled `Contents`). Cause: the IDE injects `ELECTRON_RUN_AS_NODE=1` into subprocess envs, which forces any Electron binary (Cypress's included) to boot as plain Node. The binary is fine — `./node_modules/.bin/cypress verify` passes. Self-heal: **strip the variable for the test command and retry once**:
  ```bash
  env -u ELECTRON_RUN_AS_NODE <build command>
  ```
  Pinning `CYPRESS_RUN_BINARY` does NOT fix this — the env var, not binary resolution, is the poison. If `cypress verify` passes, the binary is NOT a stub — never report it as one.
- **Prefer direct binaries over `npx`.** Invoke `./node_modules/.bin/percy` and the project's own scripts; avoid `npx <tool>` (package-proxy shims intercept it). If a bare `npm`/`npx` call dies with a shim error, rerun via `./node_modules/.bin/<tool>` before diagnosing anything.
- **Cypress binary-resolution corrupted by a shim** (rare; only after ruling out `ELECTRON_RUN_AS_NODE`): pin `CYPRESS_RUN_BINARY="$HOME/Library/Caches/Cypress/<version>/Cypress.app/Contents/MacOS/Cypress"` (`<version>` = the installed `cypress` package version) and retry once.

### 3. Capture the build ID

Parse the build URL from CLI output (`https://percy.io/<org>/<project>/builds/<id>`). Save the build ID — `/percy:review` will need it.

### 4. Hand off

Print:
> Percy build <id> started: [build #<id> ↗](<url>). Run `/percy:review` to poll and classify when it finishes.

**Cold-start note:** if `.percy/config.yml` has `has_approved_baseline: false` (or this is the first build on the branch), add:
> 🟢 This is the baseline build — there's no prior build to diff against, so expect no visual diffs or AI verdict. Approve it in Percy to lock the baseline; the *next* build gets real classification.

That way the user isn't surprised when `/percy:review` reports "no AI" on a build that had nothing to compare against.

Do not poll inside this skill — that's `/percy:review`'s job. Returning quickly lets the user kick a build and continue working.

## Important

- Don't run this inside `/percy:gate`'s iteration loop blindly — let `/percy:gate` decide when to re-build. Iterating fast burns Percy snapshots quickly.
- If the project already has CI on PR push, *don't* run this skill — push the branch and let CI kick the build instead.
