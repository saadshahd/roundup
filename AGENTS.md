# AGENTS.md

Read `CONTEXT.md` first; every identifier, RPC method and UI string uses a term from it (or adds it there in the same PR).

## Rules

1. **Done** = CI `check` green; the scenario(s) named in the PR pass in e2e; one approval from an agent whose id differs from the author's; zero anti-slop findings.
2. **Slop** (each is a CI failure): an anti-slop rule violation; TypeScript duplication, unused exports, files or dependencies reported by `pnpm slop` (fallow); Rust dead code or unused dependencies reported by clippy and `cargo machete`; a public function with no test or caller; a comment that restates the line below it.
3. **Small PR**: at most 400 changed lines (excluding lockfiles and generated files) and exactly one module directory, or only `contracts/`.
4. **Contract change** = any edit under `contracts/`. Needs Architect approval. No v2s: change every caller in the same PR.
5. **Reviewer input** = diff + linked spec + this file. Never the author's rationale.
6. **Vocabulary**: see `CONTEXT.md`; `session`, `process`, `task`, `notification` and the other _Avoid_ words are banned in public names, except under `crates/agents/claude_code/`.
7. **Perf budget**: cold start < 300 ms; keystroke-to-render < 16 ms p95; 10 idle agents < 150 MB extra RSS. A regression above 10% fails.

## Workflow

- No scenario, no work: specs are `scenarios/*.md` (given/when/then in glossary words).
- Name tests after the scenario they prove: `fn t3_...` for scenario T3 (`scenarios/README.md`).
- Write the failing test first, then the code, in your own git worktree.
- Red main is stop-the-line. Fix-forward on main is forbidden; revert.
- Anti-slop is mandatory (installed in Phase 1 via `/install-anti-slop`).

## Branches and reviews

- Builders work on a branch in their own worktree and open a PR against `main`. Nobody pushes to `main` except the Architect for `contracts/` and core crates, and docs.
- Before pushing: `pnpm install` then `just check`; the PR must also pass `loop/rules.sh size origin/main`.
- A module ships as several small PRs, each naming its scenario ids.
- The generated TypeScript in `contracts/generated/` comes from `crates/contracts`; never edit it by hand.

## Using boxd

boxd is optional. Read `docs/boxd.md` before touching it; it says what a VM can and cannot do and what was measured.

- Start boxd Builders only with `loop/boxd.sh build <name> <prompt-file>`. It creates an `--isolated` VM from the `ru-toolchain` snapshot with an auto-destroy timer, uploads `git archive HEAD`, runs `just check` as the observer, returns a patch, and destroys the VM.
- Name every VM `ru-<something>`. Create QA VMs with `--auto-destroy-timeout` and `--auto-suspend-timeout 0`, and use `--isolated` for any VM that runs code you did not write.
- The Claude token goes in per call as `CLAUDE_CODE_OAUTH_TOKEN` via `boxd machine exec -e`. Never use `boxd env set` (it is account-wide), never write the token to a file in the repo or on a VM, never print it. `loop/boxd.sh` deletes any artifact that contains it.
- Never push GitHub credentials to a VM. Results come back as a patch and are pushed from the laptop.
- At most 4 `ru-` VMs at once (`loop/boxd.sh` enforces it). If `loop/out/PAUSED` exists, a limit was hit: stop and tell the user.
- A VM cannot show macOS behaviour: perf numbers, WKWebView rendering and the macOS gate come from the laptop and CI.
- After any session, `boxd machine list` must show no `ru-` machines.
