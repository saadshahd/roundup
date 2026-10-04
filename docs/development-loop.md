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

One prompt file per role in `.agents/`. Builders and Reviewers can run Codex or Claude Code; the role's checks are the same for either tool.

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
| 6 | Merge | Driver | laptop | `loop/rules.sh merge-ready <pr>` exits 0 (L46: the base is `main`, required checks passed, fewer than 3 rejects or an Architect pick, and the lane's trailer condition). The Driver passes the printed SHA to `gh pr merge --match-head-commit <sha>`; the gate never merges or executes PR code |
| 7 | UX observers: screenshots, `checks.json` per step, `loop/rules.sh delta` against `origin/main` (L42), critic report; the Checks that are `vitest` tests gate in step 4, the rest of this step gates nothing | QA, Design critic | macOS for baselines; boxd VM for web-UI-only runs | files in `artifacts/ux/`, critic Todos (`docs/design-system.md`, "Baseline protocol") |
| 8 | Red main, or an unfixed post-merge defect | Triage | laptop | `gh run list` shows failure, or `loop/rules.sh revert-due` prints a PR (L48); revert, never fix forward |

## Done

`AGENTS.md` rule 1. A PR is in the **block** lane unless `loop/rules.sh class` (L44) prints `post`; when unsure, block.

- **Block lane**: CI `check` green; the scenarios the PR names pass, in e2e where end to end; zero anti-slop findings; one approval whose `Reviewed-by-Agent: <id>` differs from every `Author-Agent` on the PR; the body names the `PRINCIPLES.md` ids it serves, the Driver confirms they exist and the Reviewer answers each from the diff; the body has `## Shape` and `## Proof` (`loop/rules.sh proof`, L53). Proof is test output, plus before and after screenshots or a short video at the scenario's window size when the PR changes what the user sees; an approval without it does not count.
- **Specification-only** PR (L53's `Proof scope: specification`): done on baseline CI, an independent consistency review and approval. Its observers stay pending and are never reported as run. Never for production, contract, glossary, policy or mixed changes.
- **Post lane** (`docs/` prose, queue files, screened scenarios): merges on `loop/rules.sh merge-ready` (L46); an independent Reviewer posts a verdict within 60 minutes of the merge (L47). Until `revert-due` (L48) exists, the Driver assigns that Reviewer and reverts a post PR whose newest independent verdict is a reject 60 minutes after the merge; only a newer independent approve on the same head withdraws it.
- Also reverted: work a Builder started on a scenario with no approval (L49). A PR rejected 3 times merges only after an Architect picks split, amend or retire (L45).

## Before review

Before handing a head to an independent Reviewer, the Driver checks the Builder's `## Proof` against that exact head and the PR's `kind:`, `area:` and `lane:` labels. For a visible PR, the Builder measures D2 spacing and type, D5 contrast and D6 hit areas in the browser at the named viewport, then captures before and after images; a stale proof branch is refreshed before review. The Builder drives failure states on the densest named seed and measures whether messages remain visible beside pinned controls. For a bounded Daemon feature, the Builder tests the largest accepted input and runs its memory gate. The Driver also compares scenario claims with existing behavior and records any separate missing behavior in the queue. These checks come from the D5 and D6 reject on #262, D2 and stale proof reject on #264, maximum-window and memory reject on #265, the hidden error and Meta-agent folding conflict on #268, and the missing labels found on #265. A missing result or label is an open pre-review task, so it is resolved before a Reviewer spends a round on it.

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
| 4 Contract change | none | needs CODEOWNERS plus a required review. That is a GitHub setting; ask the user first |
| 5 Reviewer input | by construction in `.agents/reviewer.md` and the Driver's invocation | not machine-checkable |
| 6 Vocabulary | `loop/rules.sh vocab`: Avoid words from `CONTEXT.md` against public Rust items, TS exports and `contracts/` text | UI strings and RPC names outside `contracts/` are not scanned; enum variants are not scanned |
| 7 Perf budget | none | needs a UI and a bench; Phase 5 |
| 8 Visual change | `loop/rules.sh delta` (L42); `loop/rules.sh tokens` (L41) arrives with the Builder PR for U130; the Checks as `vitest` tests (U130 to U137) in `just check` | until U137 lands, the critic measures by hand and `delta` has no input; the PR body's `Moves:` line is read by the Reviewer, not a script |

Required status checks and branch protection are GitHub settings on `main`, outside the repo. The repo does not record whether they are set: the Driver reads them with `gh api repos/{owner}/{repo}/branches/main/protection`, and `merge-ready` (L46) checks the required checks itself. Changing them needs the user's go-ahead.

## Driver

Something has to start each step. Today that is a Codex or Claude Code session on the laptop running the steps with subagents, `loop/boxd.sh` and `gh`. There is no unattended driver. The planned one is a dedicated coding-agent session that reads `.work/queue.md` and runs steps 1 to 8. Until it exists, "the loop closes without the user" is not true; only the human gates below are unattended-ready.

Stop conditions, checked by the Driver before each step: CI red on `main` for more than 30 minutes; `loop/out/PAUSED` exists (a rate or usage limit was hit); a contract change without an architect's approval; the same perf budget breached twice.

## Human gates

After the daemon and modules demo (Phase 2), the UI screenshot pack (Phase 3), and the MVP (Phase 5), the Driver writes one report: screenshots, perf numbers, open questions, rules added or changed, and a `go / change X` choice. Between gates the user is not asked.

## Where boxd fits

Optional, never required to merge. Use it for unattended Builders (the VM is the sandbox for `--dangerously-skip-permissions`), for parallel Builders (up to `BOXD_MAX_VMS`, default 12), and for web-UI QA runs. See `docs/boxd.md`. The rules for agents using it are in `AGENTS.md`.

## Sweeps, round caps and re-reviews

Three rules for every PR. Rule 5 in `AGENTS.md` carries the re-review input; the rest is loop policy, changed here.

**A sweep's observer counts copies, not lines.** A sweep PR (slop, duplication) passes when each duplicate ends as one copy and `pnpm slop` reports no new duplicate. The duplicates are the ones its queue row names or, when the row leaves them to the Builder (as `sweep-rupd-harness` does), the list in the PR body, which the Reviewer checks against the diff. The changed-line count is advisory only, as rule 3 treats PR size: a sweep that adds more lines than it deletes is not a defect when it merges real duplication. (PR #91 closed over +52/-20 although it removed real duplication.)

**A round cap never closes a PR silently.** The Driver counts a PR's rejects, and `loop/rules.sh rounds` (L45) counts them for the merge gate: at the third, merge is blocked until an Architect other than the author posts `ARCHITECT: split` or `amend`; `ARCHITECT: retire` closes the PR and `rounds` never passes it again. On the third, the nearest free architect other than the author takes the PR over at once, fixes it in the same branch with one push (L63), and picks exactly one of: amend the observer (it was wrong or unreachable), split the PR (it is two things), or retire it. When the author is the only Architect, the user picks. The pick and its reason go in the PR's queue row, or in a new row in `.work/queue.md` when the PR has none, so the work and what was learned stay in the repo. A round count alone never closes a PR.

**After the first reject an architect fixes** (L63): the Driver runs no further Builder round, and the architect's one push in the Builder's branch is the next head. **A re-review gets the earlier findings.** After a reject, the Driver puts into the next Reviewer's prompt each earlier verdict comment (a comment whose first line is `VERDICT:`) and the diff from the rejected head to the new head, computed on the laptop because a VM checkout has neither the comments nor that head. The Reviewer checks each earlier finding is fixed, then reviews the delta. A finding on text unchanged since the rejected head blocks only when it breaks a rule of `AGENTS.md` or is a correctness defect (the text is wrong, not merely disliked); a taste finding on unchanged text is noted and does not block. The author's rationale stays out (rule 5). The cost is accepted: earlier findings may anchor the Reviewer, and without them docs PRs took up to seven rounds, each finding something new in text it had passed.

## Shared files

Every docs PR used to edit `.work/queue.md` and the table in `scenarios/README.md`, so each merge to `main` made the next PR conflict and void its review. `.gitattributes` with `merge=union` does not fix it: GitHub's own merge ignores custom drivers and attributes. Observed on 2026-10-02 with two scratch PRs (#177 and #178, closed) that inserted a row at the same place in both files, with `merge=union` on both files in the base branch: after #177 merged, GitHub reported #178 as `CONFLICTING` and `DIRTY`. A local `git merge` kept both rows, which is why the attribute looks as if it works.

The fix is that two PRs from different squads touch different files (L50): a scenario file declares its own module on a `Module:` line, so `scenarios/README.md` needs no table; the queue's rows live in one file per squad, `.work/queue/<squad>.md` (`docs/squads.md`, "Moving the queue"); `.work/queue.md` keeps the protocol text and the id table that `loop/rules.sh ranges` (L43) reads. Two PRs of the same squad still conflict, which is a real overlap. `loop/rules.sh queue` checks the shape. The move rewrites the files every open docs PR touches, so a Builder does it once, in one PR, when no docs PR is in the merge lane, and the Driver announces it first.

## Loop on the loop

Triage tags each failure with a class (contract-drift, flaky-test, vocab, perf, slop-rule, ux-checklist). When one class recurs 3 times in a cycle, the Architect studies the cases, writes the rule as a yes/no question or a machine check, and lands it as a contract-change PR. A rule with no recurrence for 3 cycles is deleted. Each human-gate report lists what changed and why.

## Merge gate input and proof

`loop/rules.sh base|class|rounds|merge-ready|proof <pr>` reads GitHub data with
paginated, bounded `gh api` calls. It fetches missing commit objects without
checking out the PR. Run the gate from the trusted default-branch checkout.
`merge-ready` checks both `check` and `rules` on the exact printed head SHA,
including when that head is an empty approval commit. It ignores the
`merge-ready` status itself to avoid a circular dependency.
Loop rules that describe screenshots do not make a loop-only PR visible.

The block lane requires an empty independent approval, either at the head or
carried through later merges of `main` that pass L54, and runs `proof`. The post lane needs Author-Agent
trailers on non-merge commits and both successful checks. The `loop` workflow runs
`ci-trailers <pr>`, which applies the same lane-specific trailer rules. Labels (L57) and architect approval
for contract changes remain separate gates for the Driver.

For implementation proof, in `## Proof` name the full head SHA and include a fenced
test-output block with a passing line for each scenario id on its `Scenarios:` line. A visible PR also
has a `Shows:` line and one line per scenario and before/after capture, such as
`U1 before [image](https://github.com/OWNER/REPO/blob/proof/pr-12/proof/before.png) 1280×800`.
The scenario text must name that window size. The proof branch contains only
regular media files under `proof/`, within L53's size limits. Capture review
and confirmation that a harness caption identifies its seed remain Reviewer
checks; the gate checks references, dimensions, file metadata and captions.

For a product-specification PR, L53 permits an explicit `Proof scope: specification`
line instead. Its path restrictions exclude production, contracts, glossary and
policy changes, including mixed changes and both sides of a rename. Keep Shape,
the exact head and actual fenced baseline output containing `just check: exit 0`.
Under `## Proof`, add `Specification consistency: <review>` and one
`Pending <id>: <future executable observer>` line for every named scenario.
The newest independent approval must cover this tree. This approves the
specification only; scenario execution and native/visual proof remain obligations
of its implementation. Normal lane, architect, approval, base and CI gates still
apply. This is not permission to describe an unimplemented scenario as passed.

L76 (`scenarios/loop-proof.md`) lets a component code change with unchanged
rendering use test proof without publishing a `proof/pr-<n>` branch. The independent
Reviewer must post an approving verdict with the exact line `Visual: unchanged`,
a full `Reviewed-head: <sha>` and one `Reviewed-by-Agent: <id>` distinct from every
author. The newest verdict for the current tree must carry that attestation.
The reviewed commit must be an ancestor with the same Git tree as the head;
an empty approval commit preserves it, but a merge that changes the tree needs
fresh review. Test proof still names the exact current head. The Driver can
prepare test proof before review, then run `proof` after the verdict and refresh
the exact-head test output after the approval commit. An author-only claim never
waives image proof. The Reviewer judges visibility from the diff: an actual
visible change still needs L53 before/after media at the scenario window size.
L54 approval carry and the separate L57 label gate still apply.
