# AGENTS.md

roundup is built by agents in a loop. Every identifier, RPC method and UI string uses a `GLOSSARY.md` term (read it when naming one), or adds one in the same PR.

## The loop

Builder, Reviewer, QA and Retro runs are one-shot GitHub Actions jobs, and GitHub does the waiting. A role's prompt is `.agents/<role>.md`, its skills are in `.agents/skills/`, and data a step reads on demand is in `.agents/data/`.

| Step | Who, where | Skills |
|---|---|---|
| 1 Specify | The user, or an `architect` run on request; one spec PR per batch | to-spec, grill-with-docs, anchor |
| 2 Build | `build.yml` (L23): each hour, one Builder run per ready row, four at most | prime, tdd; UI: emil-design-eng, percy-visual-intent; motion clause: animate |
| 3 Review | `review.yml` (L24): once `check` passes on a Code PR's head; one fix run after a first reject | review, judge; UI: percy-review |
| 4 Merge | GitHub auto-merge on `check`, `rules` and `merge-ready` (L46) | — |
| 5 Observe | `qa.yml` (L66) sweeps `main` every six hours; a `design-critic` run on request | review-animations, break-ui |
| 6 Retro | `retro.yml` (L27): one Retro PR per 20 merged PRs, read from the Ledger (L29) | retro, reduce |

## Rules

1. **Done**: `loop/rules.sh merge-ready <pr>` passes and `.agents/data/pr.md` and `.agents/data/gates.md` hold.
2. **Slop** fails CI: `just check`, plus no public function without a test or caller.
3. **PR size**: one module directory, about 2000 changed lines.
4. **Contract change** (`contracts/`, a new RPC method, an App seam command, U4's tokens) needs an approve from an Architect run or the user, other than the author. Change every caller in the same PR.
5. **Reviewer input**: the diff, the scenario, this file, earlier `VERDICT:` comments and a checkout; never the author's rationale.
6. **Vocabulary**: no `GLOSSARY.md` _Avoid_ word in a public name outside `crates/agents/src/claude_code/`.
7. **Perf**: cold start < 300 ms, keystroke-to-render < 16 ms p95, 10 idle agents < 150 MB extra RSS; `just perf` fails a regression above 10% (`docs/perf.md`).
8. **Visual change** under `apps/desktop/src` names the Checks it moves, uses only Tokens, and breaks no Check passing on `main`.

## Conventions

- No scenario, no work. Tests are named after their scenario (`fn t3_…` proves T3).
- Work in your own worktree and branch; `pnpm install` and `just check` before pushing; PRs go against `main`.
- A PR touching Loop machinery waits for the user's merge, whoever opened it.
- Red `main`: revert, never fix forward.
- Regenerate `contracts/generated/` from `crates/contracts`, never hand-edit it.
- Only an Architect pushes to `main`, and only `contracts/`, core crates and docs.
- What the user reads (the Status issue, a needs-you comment, a PR body, a report): `reduce`, then the `show-me` layout. A verdict is for agents.
