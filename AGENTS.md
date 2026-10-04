# AGENTS.md

roundup is built by agents in a loop. Every identifier, RPC method and UI string uses a `GLOSSARY.md` term (read it when naming one), or adds one in the same PR.

## The loop

Each role's prompt is `.agents/<role>.md`; data a step reads on demand is `.agents/data/`.

| Step | Role | Flow |
|---|---|---|
| 1 Specify | Architect or squad writer | `/compose to-spec to draft the scenario, grill-with-docs to test it against the glossary, anchor to make each then-clause checkable` |
| 2 Dispatch | Driver | `loop/rules.sh ready`, then `/compose router to hand a ready row to a Builder, handoff to pass its context` |
| 3 Build | Builder | `/compose sound:prime to load the taste rules, tdd to land the scenario red then green, reduce and show-me to write Shape and Proof` |
| 4 Review | Reviewer | `/compose sound:review to read the diff against the taste rules, judge to give one cited verdict` |
| 5 Merge | Driver | `loop/rules.sh merge-ready <pr>` |
| 6 Observe | QA, Design critic | `/compose break-ui to stress each screen, review-animations to score the motion` |
| 7 Recover | Triage, Architect | `/compose diagnosing-bugs to classify the failure, retro to turn a recurring class into a rule` |
| 8 Sweep | Slop sweeper | `/compose reduce to leave each duplicate as one copy` |

Claude names a moo skill `sound:prime`; Codex names it `prime`.

## Rules

1. **Done**: `loop/rules.sh merge-ready <pr>` passes and `.agents/data/pr.md` and `.agents/data/gates.md` hold.
2. **Slop** fails CI: `just check`, plus no public function without a test or caller.
3. **PR size**: aim for one module directory and about 2000 changed lines; `loop/rules.sh size` only advises.
4. **Contract change** (`contracts/`, a new RPC method, an App seam command, U4's tokens) needs a `docs/squads.md` architect other than the author. Change every caller in the same PR.
5. **Reviewer input**: the diff, the scenario, this file, earlier `VERDICT:` comments and a checkout; never the author's rationale.
6. **Vocabulary**: `GLOSSARY.md`'s _Avoid_ words stay out of public names, except under `crates/agents/src/claude_code/`.
7. **Perf**: cold start < 300 ms, keystroke-to-render < 16 ms p95, 10 idle agents < 150 MB extra RSS; `just perf` fails a regression above 10% (`docs/perf.md`).
8. **Visual change** under `apps/desktop/src` names the `docs/design-system.md` Checks it moves, uses only Tokens, and breaks no Check passing on `main`.

## Conventions

- No scenario, no work. Tests are named after their scenario (`fn t3_…` proves T3).
- Work in your own worktree and branch; `pnpm install` and `just check` before pushing; PRs go against `main`.
- Red `main`: revert, never fix forward.
- Regenerate `contracts/generated/` from `crates/contracts`, never hand-edit it.
- Only an architect pushes to `main`, and only `contracts/`, core crates and docs.
- Everything a human reads (a PR body, a verdict, a report): `reduce`, then the `show-me` layout.
- On a boxd VM (optional; `.agents/data/boxd.md`): keep every GitHub credential and the real `CLAUDE_CODE_OAUTH_TOKEN` off it and never widen the token's hosts; a VM holding boxd's GitHub login runs no unreviewed code, only publishing proof and reading or commenting on PRs.
