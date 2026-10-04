# AGENTS.md

Read `CONTEXT.md` and `PRINCIPLES.md` first; every identifier, RPC method and UI string uses a `CONTEXT.md` term, or adds one in the same PR. A skill that says `GLOSSARY.md` means `CONTEXT.md`.

## Rules

1. **Done**: `loop/rules.sh merge-ready <pr>` passes and every gate in `docs/development-loop.md` "Done" holds. A PR that changes what the user sees proves it with before and after images in `## Proof`.
2. **Slop** fails CI: `just check` finds most of it; also no public function without a test or caller, and no comment that restates its line.
3. **PR size**: aim for one module directory and about 2000 changed lines; `loop/rules.sh size` advises and never fails.
4. **Contract change** (an edit under `contracts/`, a new RPC method, an App seam command, U4's tokens) needs approval from a `docs/squads.md` architect other than the author. No v2s: change every caller in the same PR.
5. **Reviewer input** is the diff, the linked scenario, this file and earlier `VERDICT:` comments, with a checkout; never the author's rationale.
6. **Vocabulary**: `CONTEXT.md`'s _Avoid_ words stay out of public names, except under `crates/agents/claude_code/`.
7. **Perf**: cold start < 300 ms, keystroke-to-render < 16 ms p95, 10 idle agents < 150 MB extra RSS; `just perf` fails a regression above 10% (`docs/perf.md`).
8. **Visual change** under `apps/desktop/src` names the `docs/design-system.md` Checks it moves (`Moves: D3, D6`), uses only Tokens, and breaks no Check that passes on `main` (`loop/rules.sh delta`).

## Workflow

- No scenario, no work: a scenario in `scenarios/*.md`, written by its squad in its own ids (`docs/squads.md`).
- Build in your own worktree and branch: invoke `sound:prime` (Codex: `prime`), then `tdd`. Name tests after their scenario (`fn t3_…` proves T3).
- Review: invoke `sound:review` (Codex: `review`).
- Before pushing: `pnpm install`, then `just check`. Open the PR against `main`, naming its scenario ids.
- Red main: revert; never fix forward.
- `contracts/generated/` comes from `crates/contracts`; regenerate, never hand-edit.
- Only an architect pushes to `main`, and only `contracts/`, core crates and docs.

## boxd

Optional; read `docs/boxd.md` first.

- Start Builders only with `loop/boxd.sh build`.
- Keep every GitHub credential and the real `CLAUDE_CODE_OAUTH_TOKEN` off VMs: never copy, write or print one, and never widen the token's hosts.
- A VM holding boxd's own GitHub login runs no unreviewed code; it only publishes proof and reads or comments on PRs.
- VMs are named `ru-…`, at most `BOXD_MAX_VMS`; when `loop/out/PAUSED` exists, stop and tell the user. End every session with no `ru-` VM left in `boxd machine list`.
- macOS behaviour (perf, WKWebView) comes from the laptop and CI, never a VM.
