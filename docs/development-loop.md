# Development loop

How work becomes merged code with agents doing the typing. Every step names its actor, where it runs, and the observer that says whether it happened. Rules referenced by number are in `AGENTS.md`.

**Status: the loop is designed and partly enforced. It does not yet run unattended.** See "Driver" and "Gaps".

## Work item

A work item is a file `scenarios/<name>.md` in glossary words:

```
# <name>
Given <Project/Agent/Todo state>
When <one action>
Then <one observable result>
```

No scenario, no work. Before roundup can hold its own Todos, the queue is a markdown list in `.work/queue.md`, one scenario per line, owner agent id next to it.

## Roles

One prompt file per role in `.agents/`. Builders run Sonnet; Reviewer and Architect run Opus.

| Role | Does | Never |
|---|---|---|
| Architect | Owns `contracts/`, `CONTEXT.md`, ADRs. Approves contract changes. | Writes feature code. |
| Builder | Failing test first, then code, in one worktree, one module. Opens a small PR. | Reviews its own PR. |
| Reviewer | Reads diff + linked scenario + `AGENTS.md`, nothing else. Approves or lists defects. | Sees the Builder's rationale or chat. |
| QA/Driver | Runs the UI, drives scenarios, saves screenshots to `artifacts/ux/<scenario>/<step>.png`. | Edits code. |
| Design critic | Scores each screenshot against `docs/wireframes.md`'s checklist. Files Todos. | Edits code. |
| Slop sweeper | Deletes dead code and duplication (rule 2). | Adds features. |
| Triage | Tags each failure with a class and files a Todo; assigns reverts of red main. | Fixes forward. |

## Steps

| # | Step | Actor | Where | Observer |
|---|---|---|---|---|
| 1 | Pick the next scenario | Driver | laptop | the scenario file exists |
| 2 | Write the failing test, then the code | Builder | local worktree, or a boxd VM when unattended (`loop/boxd.sh build`) | the new test fails first, then `just check` passes |
| 3 | Open a small PR with commit trailers `Author-Agent: <id>` | Builder | laptop | `loop/rules.sh size` (rule 3) and `trailers` (rule 1) |
| 4 | CI | GitHub | macOS runner | `check` job (`just check`: fmt, clippy, nextest, machete, oxlint + anti-slop, tsc, fallow) |
| 5 | Review | Reviewer, a different id | laptop | an empty commit with trailer `Reviewed-by-Agent: <id>`, checked by `loop/rules.sh trailers` |
| 6 | Merge when rule 1 holds | Driver | laptop | all checks green |
| 7 | UX observers: screenshots, snapshot diff, critic score | QA/Driver, Design critic | macOS for baselines; boxd VM for web-UI-only runs | files in `artifacts/ux/`, critic Todos |
| 8 | Red main | Triage | laptop | `gh run list` shows failure; revert, never fix forward |

## Rules and what enforces them today

| Rule | Enforced by | Gap |
|---|---|---|
| 1 Done | `check` green (CI); different-agent approval via `loop/rules.sh trailers` in `.github/workflows/loop.yml`; anti-slop via `pnpm lint` | e2e scenarios do not exist yet |
| 2 Slop | `pnpm lint` (anti-slop), `pnpm slop` (fallow), `cargo machete crates`, clippy | "public function with no test or caller" and "comment restates the line below" have no machine check beyond anti-slop's own rules |
| 3 Small PR | `loop/rules.sh size` | "generated files" means `*.lock`, `pnpm-lock.yaml`, `*/generated/*`; extend as generators appear |
| 4 Contract change | none | needs CODEOWNERS plus a required review. That is a GitHub setting; ask the user first |
| 5 Reviewer input | by construction in `.agents/reviewer.md` and the Driver's invocation | not machine-checkable |
| 6 Vocabulary | `loop/rules.sh vocab`: Avoid words from `CONTEXT.md` against public Rust items, TS exports and `contracts/` text | UI strings and RPC names outside `contracts/` are not scanned; enum variants are not scanned |
| 7 Perf budget | none | needs a UI and a bench; Phase 5 |

Required status checks and branch protection are GitHub settings on `main`. They are not set up and are outside the repo; they need the user's go-ahead.

## Driver

Something has to start each step. Today that is a Claude Code session on the laptop running the steps with the Agent tool, `loop/boxd.sh` and `gh`. There is no unattended driver. The planned one is a dedicated Claude Code session in the `loop` skill's dynamic mode, started by the user, that reads `.work/queue.md` and runs steps 1 to 8. Until it exists, "the loop closes without the user" is not true; only the human gates below are unattended-ready.

Stop conditions, checked by the Driver before each step: CI red on `main` for more than 30 minutes; `loop/out/PAUSED` exists (a rate or usage limit was hit); a contract change without Architect approval; the same perf budget breached twice.

## Human gates

After the daemon and modules demo (Phase 2), the UI screenshot pack (Phase 3), and the MVP (Phase 5), the Driver writes one report: screenshots, perf numbers, open questions, rules added or changed, and a `go / change X` choice. Between gates the user is not asked.

## Where boxd fits

Optional, never required to merge. Use it for unattended Builders (the VM is the sandbox for `--dangerously-skip-permissions`), for parallel Builders up to 4, and for web-UI QA runs. See `docs/boxd.md`. The rules for agents using it are in `AGENTS.md`.

## Loop on the loop

Triage tags each failure with a class (contract-drift, flaky-test, vocab, perf, slop-rule, ux-checklist). When one class recurs 3 times in a cycle, a Rules agent studies the cases, writes the rule as a yes/no question or a machine check, and lands it as a contract-change PR. A rule with no recurrence for 3 cycles is deleted. Each human-gate report lists what changed and why.
