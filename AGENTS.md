# AGENTS.md

Read `CONTEXT.md` and `PRINCIPLES.md` first; every identifier, RPC method and UI string uses a term from it (or adds it there in the same PR).

## Rules

1. **Done** (block lane, every PR `loop/rules.sh class` does not call `post`, L44) = CI `check` green; the scenario(s) named in the PR pass in e2e; zero anti-slop findings; one approval with a `Reviewed-by-Agent: <id>` line whose id differs from every `Author-Agent` on the PR; the body names the `PRINCIPLES.md` ids it serves, the Driver confirms they exist and the Reviewer answers each from the diff; the body has `## Shape` and `## Proof` (`loop/rules.sh proof`, L53). Proof is test output, plus before and after screenshots or a short video at the scenario's window size when the PR changes what the user sees; without it an approval does not count. A specification-only PR (L53's `Proof scope: specification`) is done on baseline CI, an independent consistency review and approval; its observers stay pending and are never reported as run.
   **Post lane** (`class` says `post`: `docs/` prose, queue files, screened scenarios) merges on `loop/rules.sh merge-ready` (L46); an independent Reviewer posts a verdict within 60 minutes of the merge (L47). Until `revert-due` (L48) exists, the Driver assigns that Reviewer and reverts a post PR whose newest verdict is a reject 60 minutes after the merge. Unsure → block.
2. **Slop** (each is a CI failure): an anti-slop rule violation; TypeScript duplication, unused exports, files or dependencies reported by `pnpm slop` (fallow); Rust dead code or unused dependencies reported by clippy and `cargo machete`; a public function with no test or caller; a comment that restates the line below it.
3. **PR size is a guide**: about one module directory (or only `contracts/`) and 2000 changed lines, excluding lockfiles and generated files. `loop/rules.sh size` prints an advisory; the Reviewer notes it; it never fails a PR.
4. **Contract change** = any edit under `contracts/`, a new RPC method, an App seam command or a change to U4's tokens. Needs approval from an architect of the committee in `docs/squads.md` other than the author. No v2s: change every caller in the same PR.
5. **Reviewer input** = diff + linked spec + this file + the PR's earlier `VERDICT:` comments, with a checkout to run `loop/rules.sh`. Never the author's rationale.
6. **Vocabulary**: the _Avoid_ words of `CONTEXT.md` (`session`, `process`, `task`, `notification`, …) are banned in public names, except under `crates/agents/claude_code/`.
7. **Perf budget**: cold start < 300 ms; keystroke-to-render < 16 ms p95; 10 idle agents < 150 MB extra RSS. `just perf` fails a regression above 10%; which metrics it gates per OS, and when a latency miss is believed, is in `docs/perf.md`.
8. **Visual change**: a PR that edits a stylesheet or component under `apps/desktop/src` names the `docs/design-system.md` Checks it moves (`Moves: D3, D6`); every look value is a Token; no Check passes on `origin/main` and fails on the head (`loop/rules.sh delta`, L42). `## Proof` links the images.

## Workflow

- No scenario, no work: specs are `scenarios/*.md`, given/when/then in glossary words. Squads write them in their own files and id ranges (`docs/squads.md`) and start Builders without an architect.
- Name tests after their scenario: `fn t3_...` proves T3.
- Skills live in `.agents/skills` (Codex) and `.claude/skills` (Claude); `sound` is a plugin for Claude (`sound:prime`) and plain `prime` for Codex. A skill that says `GLOSSARY.md` means `CONTEXT.md`.
- Build: invoke `sound:prime`, then `tdd` (the failing test first, then the code), in your own git worktree.
- Review: invoke `sound:review` on the diff; every added comment must name a consequence, or is deleted (says what the code says) or corrected (false).
- Red main is stop-the-line: revert, never fix forward. Also reverted: work a Builder started on an unapproved scenario (L49). A PR rejected 3 times waits for an Architect to pick split, amend or retire (L45).

## Branches

- Builders open a PR against `main` from their own branch and worktree. Only an architect pushes to `main`, and only `contracts/`, core crates and docs.
- Before pushing: `pnpm install`, then `just check`.
- A module may ship as several PRs; each names its scenario ids.
- `contracts/generated/` is generated from `crates/contracts`; never edit it by hand.

## boxd

Optional; read `docs/boxd.md` before touching it. Always:

- Start Builders only with `loop/boxd.sh build`; it creates, guards, checks and destroys the VM and returns a patch.
- Never copy, write or print a GitHub credential or the real `CLAUDE_CODE_OAUTH_TOKEN` into or out of a VM, and never widen that secret's hosts. A VM with boxd's own GitHub login runs no unreviewed code and only publishes proof and reads or comments on PRs.
- Name VMs `ru-…`; at most `BOXD_MAX_VMS` (default 12). If `loop/out/PAUSED` exists, stop and tell the user.
- A VM cannot show macOS behaviour (perf, WKWebView, the macOS gate).
- After any session, `boxd machine list` shows no `ru-` machines.
