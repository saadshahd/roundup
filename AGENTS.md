# AGENTS.md

roundup is built by agents in a loop. Every identifier, RPC method and UI string uses a `GLOSSARY.md` term (read it when naming one), or adds one in the same PR.

## The loop

Work orders are GitHub Issues (`.agents/data/work.md`); scenario files hold acceptance only.

Every unattended run chooses and executes routine engineering work under the recorded direction. A review run judges each proposed change; the user answers only `flag:needs-user` questions (L79) and the required Mac observation. An approach, repair strategy or next work selection never waits for user confirmation.

Builder and QA runs are one-shot GitHub Actions jobs, and an event ends every wait: a merge, a run's end or a failed check starts the next run. The user ends only the waits labelled `flag:needs-user` (L79). A Builder run builds a work Issue, fixes a PR or reviews a PR's head, each fresh, from one prompt, `.agents/builder.md`. A role's prompt is `.agents/<role>.md`, its skills are in `.agents/skills/`, and data a step reads on demand is in `.agents/data/`.

| Step | Who, where | Skills |
|---|---|---|
| 1 Ask | The user sets direction and files work Issues; a build run specifies an Issue that holds no Key (L34) | — |
| 2 Build | `build.yml` (L23): on each merge, each `ready-for-agent` label and each hour, one Builder run per ready Issue, four at most | prime; spec: to-spec, grill-with-docs, anchor; seam: codebase-design; tdd; UI: emil-design-eng; motion clause: animate |
| 3 Review | `review.yml` (L24): a review run once `check` passes on a PR's head; `fix.yml` (L81): at most three fix runs per Builder PR | review, judge; UI: percy-review |
| 4 Merge | GitHub auto-merge on `check`, `rules`, `review`, `percy` and `macos-line` (L46) | — |
| 5 Observe | `qa.yml` (L66) sweeps `main` every six hours; a `design-critic` run on request | review-animations, break-ui |

## Rules

1. **Done**: GitHub merged the PR once its required checks passed on its head (L46), and `.agents/data/pr.md` and `.agents/data/gates.md` hold.
2. **Slop** fails CI: `just check`, plus no public function without a test or caller.
3. **PR size**: one module directory, about 2000 changed lines.
4. **Contract change** (`contracts/`, a new RPC method, an App seam command, U4's tokens) follows the Contract recipe of `.agents/builder.md`: every caller changes in the same PR, and the review run's approve covers it.
5. **Reviewer input**: the diff, the scenario, this file, earlier `VERDICT:` comments and a checkout; never the author's rationale.
6. **Vocabulary**: no `GLOSSARY.md` _Avoid_ word in a public name outside `crates/agents/src/claude_code/`.
7. **Perf**: cold start < 300 ms, keystroke-to-render < 16 ms p95, 10 idle agents < 150 MB extra RSS; `just perf` fails a regression above 10% (`docs/perf.md`).
8. **Visual change** under `apps/desktop/src` names the Checks it moves, uses only Tokens, and breaks no Check passing on `main`.

## Conventions

- No scenario, no work. Tests are named after their scenario (`fn t3_…` proves T3).
- Work in your own worktree and branch; `pnpm install` and `just check` before pushing; PRs go against `main`.
- Loop machinery, including workflows, merges after independent review and the required checks; reviewers use main’s instructions and inspect the proposed diff.
- Red `main`: revert, never fix forward.
- Regenerate `contracts/generated/` from `crates/contracts`, never hand-edit it.
- No run pushes to `main`; every change merges through a PR.
- What the user reads (the Status issue, a needs-you comment, a PR body, a report): `reduce`, then the `show-me` layout. A verdict is for agents.
