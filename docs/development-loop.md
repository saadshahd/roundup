# Development loop

How work becomes merged code with agents doing the typing: each step names its actor, where it runs and its observer. Rule numbers are `AGENTS.md`'s.

**Status: designed and partly enforced; it does not yet run unattended** (see Driver and the rules table).

## Work item

A work item is a file `scenarios/<name>.md` in glossary words:

```
# <name>
Given <Project/Agent/Todo state>
When <one action>
Then <one observable result>
```

Until roundup holds its own Todos, the queue is `.work/queue/<squad>.md`, one scenario per row with its owner.

## Roles

One prompt per role in `.agents/`, which names the skills the role invokes; Builders and Reviewers run Codex or Claude Code with the same checks.

| Role | Does | Never |
|---|---|---|
| Architect (a committee of three, `docs/squads.md`) | Approves contract changes; owns `CONTEXT.md`, ADRs, Tokens and Checks; fixes a rejected PR; turns recurring failures into rules. | Writes feature code. |
| Builder | Failing test first, then code, in its own worktree. Opens a PR. | Reviews its own PR. |
| Reviewer | Judges the diff on rule 5's input; approves or lists defects. | Sees the Builder's rationale. |
| Driver | Starts each step, merges when rule 1 holds, files Todos, stops on its stop conditions. | Writes code or reviews. |
| QA | Runs the UI, drives scenarios, saves screenshots to `artifacts/ux/<scenario>/<step>.png`. | Edits code. |
| Design critic | After a UI PR is approved, scores the harness against the written rules into `artifacts/ux/<id>/report.md`. | Edits code; gates a merge. |
| Slop sweeper | Deletes dead code and duplication (rule 2). | Adds features. |
| Triage | Classifies each failure; names the revert for red main. | Fixes forward. |

## Steps

| # | Step | Actor | Where | Observer |
|---|---|---|---|---|
| 1 | Pick the next scenario | Driver | laptop | the scenario file exists |
| 2 | Write the failing test, then the code | Builder | local worktree, or a boxd VM when unattended (`loop/boxd.sh build`) | `just check` passes. That the test failed first is not machine-checked (gap) |
| 3 | Open a PR (one module, about 2000 lines) with commit trailers `Author-Agent: <id>` | Builder | laptop | `loop/rules.sh size` (rule 3) |
| 4 | CI | GitHub | macOS runner | `check` job (`just check`: fmt, clippy, nextest, machete, oxlint + anti-slop, tsc, fallow) |
| 5 | Review | Reviewer, a different id | laptop or boxd VM | a `VERDICT:` comment (posted by the Driver from a VM), then, block lane only, an empty `Reviewed-by-Agent` commit; `loop/rules.sh trailers` checks the ids (`.agents/reviewer.md`) |
| 6 | Merge | Driver | laptop | `loop/rules.sh merge-ready <pr>` exits 0 (L46); `gh pr merge --match-head-commit <sha>`. The gate never runs PR code |
| 7 | UX observers: screenshots, `checks.json` per step, `loop/rules.sh delta` against `origin/main` (L42), critic report; the Checks that are `vitest` tests gate in step 4, the rest of this step gates nothing | QA, Design critic | macOS for baselines; boxd VM for web-UI-only runs | files in `artifacts/ux/`, critic Todos (`docs/design-system.md`, "Baseline protocol") |
| 8 | Red main, or an unfixed post-merge defect | Triage | laptop | `gh run list` shows failure, or `loop/rules.sh revert-due` prints a PR (L48); revert, never fix forward |

## Done

`AGENTS.md` rule 1. A PR is in the **block** lane unless `loop/rules.sh class` (L44) prints `post`; when unsure, block.

- **Block lane**: CI `check` green; the scenarios the PR names pass, in e2e where end to end; zero anti-slop findings; one approval whose `Reviewed-by-Agent: <id>` differs from every `Author-Agent` on the PR; the body names the `PRINCIPLES.md` ids it serves, the Driver confirms they exist and the Reviewer answers each from the diff; the body has `## Shape` and `## Proof` (`loop/rules.sh proof`, L53). Proof is test output, plus before and after screenshots or a short video at the scenario's window size when the PR changes what the user sees; an approval without it does not count.
- **Specification-only** PR (L53's `Proof scope: specification`): done on baseline CI, an independent consistency review and approval. Its observers stay pending and are never reported as run. Never for production, contract, glossary, policy or mixed changes.
- **Post lane** (`docs/` prose, queue files, screened scenarios): merges on `loop/rules.sh merge-ready` (L46); an independent Reviewer posts a verdict within 60 minutes of the merge (L47). Until `revert-due` (L48) exists, the Driver assigns that Reviewer and reverts a post PR whose newest independent verdict is a reject 60 minutes after the merge; only a newer independent approve on the same head withdraws it.
- Also reverted: work a Builder started on a scenario with no approval (L49). A PR rejected 3 times merges only after an Architect picks split, amend or retire (L45).

## Before review

The Driver checks the Builder's `## Proof` (`.agents/builder.md` step 4) against the exact head, and its `kind:`, `area:` and `lane:` labels. A missing result or label goes back to the Builder before a Reviewer spends a round on it.

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
| 1 Done | `check` green (CI); `loop/rules.sh merge-ready <pr>` before every merge (L46, which runs `base`, `rounds`, and `trailers` or the post-lane trailer check); an overdue post-merge review is a stall of kind f (L47) and `revert-due` (L48) lists reverts; `trailers` also runs in `.github/workflows/loop.yml` (not a required check); anti-slop via `pnpm lint` | e2e scenarios do not exist yet; principle gates (`PRINCIPLES.md`) are answered by the Reviewer from the diff, for the ids the PR body names, by hand; their observers (P2 to P5) are not written |
| 2 Slop | `pnpm lint` (anti-slop), `pnpm slop` (fallow), `cargo machete crates`, clippy | "public function with no test or caller" and "comment restates the line below" have no machine check beyond anti-slop's own rules |
| 3 PR size guide | `loop/rules.sh size` (advisory only: prints when a PR spans more than one module directory or exceeds about 2000 changed lines; exits 0 whatever the size; an unknown base ref still fails) | "generated files" means `*.lock`, `pnpm-lock.yaml`, `*/generated/*`; extend as generators appear |
| 4 Contract change | the Architect's `ARCHITECT: approve` comment, by hand | CODEOWNERS and a required review are GitHub settings; ask the user first |
| 5 Reviewer input | the Driver's brief | not machine-checkable |
| 6 Vocabulary | `loop/rules.sh vocab`: Avoid words from `CONTEXT.md` against public Rust items, TS exports and `contracts/` text | UI strings and RPC names outside `contracts/` are not scanned; enum variants are not scanned |
| 7 Perf budget | `just perf` (`docs/perf.md`) | keystroke-to-render is limit-only, macOS only (`just perf-keystroke`) |
| 8 Visual change | `loop/rules.sh delta` (L42); the Checks as `vitest` tests (U130 to U137) in `just check` | `tokens` (L41) is not written; until U137 lands, the critic measures by hand and `delta` has no input; the PR body's `Moves:` line is read by the Reviewer, not a script |

Required status checks and branch protection are GitHub settings on `main`, outside the repo. The repo does not record whether they are set: the Driver reads them with `gh api repos/{owner}/{repo}/branches/main/protection`, and `merge-ready` (L46) checks the required checks itself. Changing them needs the user's go-ahead.

## Driver

Today a Codex or Claude Code session on the laptop runs the steps with subagents, `loop/boxd.sh` and `gh`; its stop conditions are in `.agents/driver.md`. No unattended Driver exists, so "the loop closes without the user" is not yet true.

## Human gates

After the daemon and modules demo (Phase 2), the UI screenshot pack (Phase 3), and the MVP (Phase 5), the Driver writes one report: screenshots, perf numbers, open questions, rules added or changed, and a `go / change X` choice. Between gates the user is not asked.

## Rejects and re-reviews

- **After the first reject an architect fixes** (L63): a free architect other than the author makes one push in the Builder's branch; no further Builder round.
- **At the third reject** `loop/rules.sh rounds` (L45) blocks the merge until an Architect other than the author posts `ARCHITECT: split`, `amend` or `retire` (the user picks when the author is the only Architect). The pick and its reason go in the PR's queue row. A round count alone never closes a PR.
- **A re-review gets the earlier findings**: each earlier `VERDICT:` comment and the diff from the rejected head to the new head, computed on the laptop. A finding on unchanged text blocks only for a broken rule or a correctness defect. The accepted cost is that earlier findings may anchor the Reviewer.
- **A sweep counts copies, not lines**: it passes when each named duplicate ends as one copy and `pnpm slop` reports no new one.

## Shared files

Two PRs from different squads touch different files (L50): a scenario file names its module on a `Module:` line, queue rows live in `.work/queue/<squad>.md`, and `.work/queue.md` keeps the protocol and the id table. `merge=union` does not help: GitHub's merge ignores custom drivers.

## Loop on the loop

Triage classifies each failure; the Architect's batch `retro` turns a class that recurs 3 times into a rule and deletes a rule with no recurrence for 3 batches (`.agents/architect.md`). Each human-gate report lists what changed and why.

## Proof format (L53, L54, L76)

`loop/rules.sh` reads GitHub with bounded `gh api` calls and never checks out the PR; run it from a trusted `main` checkout. `merge-ready` checks `check` and `rules` on the exact head, an empty approval commit included.

- **Every PR**: `## Proof` names the full head SHA and holds a fenced test-output block with a passing line for each id on its `Scenarios:` line.
- **Visible PR**: also a `Shows:` line and one line per scenario and capture, such as `U1 before [image](https://github.com/OWNER/REPO/blob/proof/pr-12/proof/before.png) 1280×800`, at a window size the scenario names. The `proof/pr-<n>` branch holds only media under `proof/`, within L53's limits. The gate checks references, sizes and captions; the Reviewer checks what the images show.
- **Specification-only PR**: `Proof scope: specification`, fenced output with `just check: exit 0`, `Specification consistency: <review>` and one `Pending <id>: <observer>` per scenario; the newest independent approval covers this tree.
- **Unchanged rendering** (L76, `scenarios/loop-proof.md`): test proof without a proof branch when the independent approve carries `Visual: unchanged`, `Reviewed-head: <sha>` and `Reviewed-by-Agent: <id>`, on an ancestor with the head's tree. A tree-changing merge needs a fresh review; an author's claim never waives images.
- Approval carry across merges of `main` is L54; labels (L57) and contract approval stay Driver gates.
