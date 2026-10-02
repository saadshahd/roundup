# AGENTS.md

Read `CONTEXT.md` and `PRINCIPLES.md` first; every identifier, RPC method and UI string uses a term from it (or adds it there in the same PR).

## Rules

1. **Done** = CI `check` green; the scenario(s) named in the PR pass in e2e; one approval from an agent whose id differs from the author's; zero anti-slop findings; the PR body names the `PRINCIPLES.md` ids it serves and the Merger confirms they exist; the Reviewer answers each of those gates from the diff. That is the **block** lane, for every PR except a `post` one. A PR of class `post` (`loop/rules.sh class`, L44: `docs/` prose, `.work/queue.md` and scenarios that pass the content screen) merges on green CI, a base of `main` and `Author-Agent` trailers (`loop/rules.sh merge-ready`, L46), and an independent Reviewer must post a verdict within 60 minutes of the merge (L47). Until `loop/rules.sh class` and `merge-ready` exist on `main`, every PR is in the block lane.
2. **Slop** (each is a CI failure): an anti-slop rule violation; TypeScript duplication, unused exports, files or dependencies reported by `pnpm slop` (fallow); Rust dead code or unused dependencies reported by clippy and `cargo machete`; a public function with no test or caller; a comment that restates the line below it.
3. **PR size is a guide**: aim for exactly one module directory (or only `contracts/`) and about 2000 changed lines (excluding lockfiles and generated files). A PR that exceeds either is allowed; `loop/rules.sh size` prints an advisory and the Reviewer notes it. Neither ever fails a PR.
4. **Contract change** = any edit under `contracts/`. Needs approval from an architect of the committee in `docs/squads.md`, one whose id differs from the author's; so do a new RPC method, an App seam command and a change to U4's tokens. No v2s: change every caller in the same PR.
5. **Reviewer input** = diff + linked spec + this file + the PR's earlier `VERDICT:` comments, with a checkout so it can run `loop/rules.sh`. Never the author's rationale.
6. **Vocabulary**: see `CONTEXT.md`; `session`, `process`, `task`, `notification` and the other _Avoid_ words are banned in public names, except under `crates/agents/claude_code/`.
7. **Perf budget**: cold start < 300 ms; keystroke-to-render < 16 ms p95; 10 idle agents < 150 MB extra RSS. A regression above 10% fails: `just perf` enforces it for five metrics on Linux and five on macOS (not the same five: cold start is gated only on macOS), except that a time metric's regression test is skipped, loudly, when the machine is busier than its baseline's load (R13); every other metric is limit-only. Keystroke-to-render is limit-only too, measured on macOS by `just perf-keystroke`. A latency miss on a busy machine is rerun on a quiet one before it is believed; a memory miss is believed at any load (`docs/perf.md`).
8. **Visual change**: a PR that edits a stylesheet or component under `apps/desktop/src` names the Checks of `docs/design-system.md` it moves (`Moves: D3, D6`). Every look value is a Token (`loop/rules.sh tokens`, L41, once it lands), and no Check passes on `origin/main` and fails on the head (`loop/rules.sh delta`, L42, over QA's `checks.json` files). A Check that is a `vitest` test gates in `just check`; the Design critic's run on the real rendering is advisory.

## Workflow

- No scenario, no work: specs are `scenarios/*.md` (given/when/then in glossary words).
- Name tests after the scenario they prove: `fn t3_...` for scenario T3 (`scenarios/README.md`).
- Write the failing test first, then the code, in your own git worktree.
- Before writing or editing code, read the taste rules in `.claude/sound/` (`sound:prime`): 33 rules chosen for this repo, one file each, grouped by topic. Reviewers check every added comment against `comment-must-name-a-consequence`: delete a comment the code already says, correct one that is false. Rules the daemon deliberately breaks (shared locked state in one process, in-file unit tests of private functions) are not installed.
- Red main is stop-the-line. Fix-forward on main is forbidden; revert. A `post` PR with an unfixed `VERDICT: reject` 60 minutes after its merge is reverted (`loop/rules.sh revert-due`, L48), and so is work a Builder started on a scenario with no approval (L49). A PR rejected 4 times cannot merge until an Architect picks split, amend or retire (L45).
- Anti-slop is mandatory (installed in Phase 1 via `/install-anti-slop`).

Scenarios are written by squads inside their own files and id ranges (`docs/squads.md`); a squad needs no architect to write a scenario or start a Builder.

## Branches and reviews

- Builders work on a branch in their own worktree and open a PR against `main`. Nobody pushes to `main` except an architect for `contracts/` and core crates, and docs.
- Before pushing: `pnpm install` then `just check`; the PR must also pass `loop/rules.sh size origin/main` (advisory only: it exits 0 and prints when a guide is exceeded).
- A module may ship as several PRs; each names its scenario ids.
- The generated TypeScript in `contracts/generated/` comes from `crates/contracts`; never edit it by hand.

## Using boxd

boxd is optional. Read `docs/boxd.md` before touching it; it says what a VM can and cannot do and what was measured.

- Start boxd Builders only with `loop/boxd.sh build <name> <prompt-file>`. It creates an `--isolated` VM from the `ru-toolchain` snapshot with an auto-destroy timer, uploads `git archive HEAD`, runs `just check` as the observer, returns a patch, and destroys the VM.
- Name every VM `ru-<something>`. Create QA VMs with `--auto-destroy-timeout` and `--auto-suspend-timeout 0`, and use `--isolated` for any VM that runs code you did not write.
- Claude authenticates through the boxd secret `CLAUDE_CODE_OAUTH_TOKEN` (sealed, scoped to `*.anthropic.com`, `*.claude.com`, `claude.ai`): a VM sees only a placeholder and boxd substitutes the real token on those hosts. Never print or write the real token, and do not widen the secret's hosts.
- Never push GitHub credentials to a VM. Results come back as a patch and are pushed from the laptop.
- At most `BOXD_MAX_VMS` `ru-` VMs at once, default 12 (`loop/boxd.sh` enforces it). If `loop/out/PAUSED` exists, a limit was hit: stop and tell the user.
- A VM cannot show macOS behaviour: perf numbers, WKWebView rendering and the macOS gate come from the laptop and CI.
- After any session, `boxd machine list` must show no `ru-` machines.
