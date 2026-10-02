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
| Architect (a committee of three, `docs/squads.md`) | Approves `contracts/` changes, App seam commands and shared tokens; owns `CONTEXT.md` and ADRs; settles conflicts between squads. Turns recurring failures into new rules (see Loop on the loop). | Writes feature code. |
| Builder | Failing test first, then code, in one worktree, one module where you can. Opens a PR. | Reviews its own PR. |
| Reviewer | Reads the diff, the linked scenario, `AGENTS.md` and, on a re-review, the earlier verdicts, with a checkout to run `loop/rules.sh`. Answers each `PRINCIPLES.md` gate the PR body names, from the diff. Approves or lists defects. | Sees the Builder's rationale or chat. |
| Driver | Starts each step, merges when rule 1 holds, stops on the conditions below. | Writes code or reviews. |
| QA | Runs the UI, drives scenarios, saves screenshots to `artifacts/ux/<scenario>/<step>.png`. | Edits code. |
| Design critic | After a UI PR is approved, drives the app in the harness, captures screenshots and motion frames, scores them against the written rules in its prompt and writes `artifacts/ux/<id>/report.md`. Lists each broken rule in its report; the Driver files a Todo for it. | Edits code; gates a merge. |
| Slop sweeper | Deletes dead code and duplication (rule 2). | Adds features. |
| Triage | Tags each failure with a class and files a Todo; assigns reverts of red main. | Fixes forward. |

## Steps

| # | Step | Actor | Where | Observer |
|---|---|---|---|---|
| 1 | Pick the next scenario | Driver | laptop | the scenario file exists |
| 2 | Write the failing test, then the code | Builder | local worktree, or a boxd VM when unattended (`loop/boxd.sh build`) | `just check` passes. That the test failed first is not machine-checked (gap) |
| 3 | Open a PR (one module, about 2000 lines) with commit trailers `Author-Agent: <id>` | Builder | laptop | `loop/rules.sh size` (rule 3) |
| 4 | CI | GitHub | macOS runner | `check` job (`just check`: fmt, clippy, nextest, machete, oxlint + anti-slop, tsc, fallow) |
| 5 | Review | Reviewer, a different id | laptop | the verdict is posted first as a PR comment, by the Reviewer if it has `gh`, else by the Driver (`docs/boxd.md`, "Where a Reviewer's verdict goes"); the Driver checks the comment by hand. **Block lane:** an empty commit carrying only `Reviewed-by-Agent: <id>` follows it, and `loop/rules.sh trailers` checks that it differs from every `Author-Agent` (two self-asserted strings, not identities); the same actor pushes it. **Post lane** (`loop/rules.sh class` prints `post`, L44): no approval commit; the verdict is due 60 minutes after the merge (L47). The verdict carries a line `Reviewed-by-Agent: <id>` (L44 to L49 count only verdicts whose id differs from every author), and the post lane's Reviewer pushes no approval commit. Only a real defect gets `VERDICT: reject`; wording, process and taste findings are notes in an approve |
| 6 | Merge | Driver | laptop | `loop/rules.sh merge-ready <pr>` exits 0 (L46: the base is `main`, required checks passed, fewer than 4 rejects or an Architect pick, and the lane's trailer condition). Until it exists on `main`, the Driver runs `base <pr>` and `trailers` by hand and every PR is block lane |
| 7 | UX observers: screenshots, `checks.json` per step, `loop/rules.sh delta` against `origin/main` (L42), critic report; the Checks that are `vitest` tests gate in step 4, the rest of this step gates nothing | QA, Design critic | macOS for baselines; boxd VM for web-UI-only runs | files in `artifacts/ux/`, critic Todos (`docs/design-system.md`, "Baseline protocol") |
| 8 | Red main, or an unfixed post-merge defect | Triage | laptop | `gh run list` shows failure, or `loop/rules.sh revert-due` prints a PR (L48); revert, never fix forward |

## Driving the App in a browser

For QA and Reviewers who need the real App without a Daemon, Tauri or a display. `just harness <seed> <port>` (defaults `tree-40`, `5199`; the port is strict, so pass another when it is taken) serves it on a fake Daemon at `http://localhost:<port>/harness.html?seed=<seed>`, which `agent-browser` opens like any page. Scenario U26 defines the seeds.

| Seed | Starts with |
|---|---|
| `first-run` | no open Project |
| `agents-10` | ten Agents, one per Kind in turn |
| `tree-40` | forty nodes (nested Groups, a Meta-agent, Terminals), eight Todos, four Pads |
| `daemon-exits` | `agents-10`, then `daemon-exited` with code 1 once loaded |
| `conflict` | `agents-10`, where the first call after loading fails with `CONFLICT` |

The page exposes `window.__fake`, so `agent-browser eval` can drive it:

- `__fake.setStatus("agent-3", "needs-you", "asks: keep v1?")` sends `agent.status`.
- `__fake.writeOutput("t-agent-3", "hello\r\n")` sends `terminal.output` to that Terminal. Nothing shows until that Agent is selected in the Rail, because the centre pane shows the selected node's Terminal.
- `__fake.emit({actor: {kind: "user", id: "you", parent: null}, name: "rail.changed"})` sends any Event.
- `__fake.failNext(-32003, "name already taken")` makes the next call of any method reject.
- `__fake.app.exitDaemon({code: 1})` sends `daemon-exited`; `__fake.app.calls` lists every call made.

Every write the App can make (spawn, create Group, rename, promote, move, stop, kill, and the Todo and Pad writes) changes the fake Daemon and sends the Events the real one sends, so the Rail and Shelf refresh. It is a fake: it checks no Route, Block or Provenance, and it says nothing about macOS rendering or timing. Read the DOM, never the numbers.

## Rules and what enforces them today

| Rule | Enforced by | Gap |
|---|---|---|
| 1 Done | `check` green (CI); `loop/rules.sh merge-ready <pr>` before every merge (L46, which runs `base`, `rounds`, and `trailers` or the post-lane trailer check); an overdue post-merge review is a stall of kind f (L47) and `revert-due` (L48) lists reverts; until L44 to L49 are on `main` these run by hand and every PR is block lane; `trailers` also runs in `.github/workflows/loop.yml` (not a required check); anti-slop via `pnpm lint` | e2e scenarios do not exist yet; principle gates (`PRINCIPLES.md`) are answered by the Reviewer from the diff, for the ids the PR body names, by hand; their observers (P2 to P5) are not written |
| 2 Slop | `pnpm lint` (anti-slop), `pnpm slop` (fallow), `cargo machete crates`, clippy | "public function with no test or caller" and "comment restates the line below" have no machine check beyond anti-slop's own rules |
| 3 PR size guide | `loop/rules.sh size` (advisory only: prints when a PR spans more than one module directory or exceeds about 2000 changed lines; exits 0 whatever the size; an unknown base ref still fails) | "generated files" means `*.lock`, `pnpm-lock.yaml`, `*/generated/*`; extend as generators appear |
| 4 Contract change | none | needs CODEOWNERS plus a required review. That is a GitHub setting; ask the user first |
| 5 Reviewer input | by construction in `.agents/reviewer.md` and the Driver's invocation | not machine-checkable |
| 6 Vocabulary | `loop/rules.sh vocab`: Avoid words from `CONTEXT.md` against public Rust items, TS exports and `contracts/` text | UI strings and RPC names outside `contracts/` are not scanned; enum variants are not scanned |
| 7 Perf budget | none | needs a UI and a bench; Phase 5 |
| 8 Visual change | `loop/rules.sh delta` (L42); `loop/rules.sh tokens` (L41) arrives with the Builder PR for U130; the Checks as `vitest` tests (U130 to U137) in `just check` | until U137 lands, the critic measures by hand and `delta` has no input; the PR body's `Moves:` line is read by the Reviewer, not a script |

Required status checks and branch protection are GitHub settings on `main`, outside the repo. The repo does not record whether they are set: the Driver reads them with `gh api repos/{owner}/{repo}/branches/main/protection`, and `merge-ready` (L46) checks the required checks itself. Changing them needs the user's go-ahead.

## Driver

Something has to start each step. Today that is a Claude Code session on the laptop running the steps with the Agent tool, `loop/boxd.sh` and `gh`. There is no unattended driver. The planned one is a dedicated Claude Code session in the `loop` skill's dynamic mode, started by the user, that reads `.work/queue.md` and runs steps 1 to 8. Until it exists, "the loop closes without the user" is not true; only the human gates below are unattended-ready.

Stop conditions, checked by the Driver before each step: CI red on `main` for more than 30 minutes; `loop/out/PAUSED` exists (a rate or usage limit was hit); a contract change without an architect's approval; the same perf budget breached twice.

## Human gates

After the daemon and modules demo (Phase 2), the UI screenshot pack (Phase 3), and the MVP (Phase 5), the Driver writes one report: screenshots, perf numbers, open questions, rules added or changed, and a `go / change X` choice. Between gates the user is not asked.

## Where boxd fits

Optional, never required to merge. Use it for unattended Builders (the VM is the sandbox for `--dangerously-skip-permissions`), for parallel Builders (up to `BOXD_MAX_VMS`, default 12), and for web-UI QA runs. See `docs/boxd.md`. The rules for agents using it are in `AGENTS.md`.

## Sweeps, round caps and re-reviews

Three rules for every PR. Rule 5 in `AGENTS.md` carries the re-review input; the rest is loop policy, changed here.

**A sweep's observer counts copies, not lines.** A sweep PR (slop, duplication) passes when each duplicate ends as one copy and `pnpm slop` reports no new duplicate. The duplicates are the ones its queue row names or, when the row leaves them to the Builder (as `sweep-rupd-harness` does), the list in the PR body, which the Reviewer checks against the diff. The changed-line count is advisory only, as rule 3 treats PR size: a sweep that adds more lines than it deletes is not a defect when it merges real duplication. (PR #91 closed over +52/-20 although it removed real duplication.)

**A round cap never closes a PR silently.** The Driver counts a PR's rejects, and `loop/rules.sh rounds` (L45) counts them for the merge gate: at the fourth, merge is blocked until an Architect other than the author posts `ARCHITECT: split` or `amend`; `ARCHITECT: retire` closes the PR and `rounds` never passes it again. After the second, an architect other than the author picks exactly one of: amend the observer (it was wrong or unreachable), split the PR (it is two things), or retire it. When the author is the only Architect, the user picks. The pick and its reason go in the PR's queue row, or in a new row in `.work/queue.md` when the PR has none, so the work and what was learned stay in the repo. A round count alone never closes a PR.

**A re-review gets the earlier findings.** After a reject, the Driver puts into the next Reviewer's prompt each earlier verdict comment (a comment whose first line is `VERDICT:`) and the diff from the rejected head to the new head, computed on the laptop because a VM checkout has neither the comments nor that head. The Reviewer checks each earlier finding is fixed, then reviews the delta. A finding on text unchanged since the rejected head blocks only when it breaks a rule of `AGENTS.md` or is a correctness defect (the text is wrong, not merely disliked); a taste finding on unchanged text is noted and does not block. The author's rationale stays out (rule 5). The cost is accepted: earlier findings may anchor the Reviewer, and without them docs PRs took up to seven rounds, each finding something new in text it had passed.

## Loop on the loop

Triage tags each failure with a class (contract-drift, flaky-test, vocab, perf, slop-rule, ux-checklist). When one class recurs 3 times in a cycle, the Architect studies the cases, writes the rule as a yes/no question or a machine check, and lands it as a contract-change PR. A rule with no recurrence for 3 cycles is deleted. Each human-gate report lists what changed and why.
