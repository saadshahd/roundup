# AGENTS.md

Read `CONTEXT.md` first; every identifier, RPC method and UI string uses a term from it (or adds it there in the same PR).

## Rules

1. **Done** = CI `check` green; the scenario(s) named in the PR pass in e2e; one approval from an agent whose id differs from the author's; zero anti-slop findings.
2. **Slop** (each is a CI failure): an anti-slop rule violation; TypeScript duplication, unused exports, files or dependencies reported by `pnpm slop` (fallow); Rust dead code or unused dependencies reported by clippy and `cargo machete`; a public function with no test or caller; a comment that restates the line below it.
3. **PR size is a guide**: aim for exactly one module directory (or only `contracts/`) and about 2000 changed lines (excluding lockfiles and generated files). A PR that exceeds either is allowed; `loop/rules.sh size` prints an advisory and the Reviewer notes it. Neither ever fails a PR.
4. **Contract change** = any edit under `contracts/`. Needs Architect approval. No v2s: change every caller in the same PR.
5. **Reviewer input** = diff + linked spec + this file, with a checkout so it can run `loop/rules.sh`. Never the author's rationale.
6. **Vocabulary**: see `CONTEXT.md`; `session`, `process`, `task`, `notification` and the other _Avoid_ words are banned in public names, except under `crates/agents/claude_code/`.
7. **Perf budget**: cold start < 300 ms; keystroke-to-render < 16 ms p95; 10 idle agents < 150 MB extra RSS. A regression above 10% fails.

## Workflow

- No scenario, no work: specs are `scenarios/*.md` (given/when/then in glossary words).
- Name tests after the scenario they prove: `fn t3_...` for scenario T3 (`scenarios/README.md`).
- Write the failing test first, then the code, in your own git worktree.
- Before writing or editing code, read the taste rules in `.claude/sound/` (`sound:prime`): 33 rules chosen for this repo, one file each, grouped by topic. Reviewers check every added comment against `comment-must-name-a-consequence`: delete a comment the code already says, correct one that is false. Rules the daemon deliberately breaks (shared locked state in one process, in-file unit tests of private functions) are not installed.
- Red main is stop-the-line. Fix-forward on main is forbidden; revert.
- Anti-slop is mandatory (installed in Phase 1 via `/install-anti-slop`).

## Branches and reviews

- Builders work on a branch in their own worktree and open a PR against `main`. Nobody pushes to `main` except the Architect for `contracts/` and core crates, and docs.
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
