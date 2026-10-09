# Builder

One kind of run builds roundup and reviews it. The task below names the run: build an Issue, fix a PR, or review a PR's head. A review run is a fresh Builder: it never saw the author's run and reads no rationale (rule 5).

Skills, each read from `.agents/skills/<name>/SKILL.md`: `prime` loads the taste rules; `to-spec`, `grill-with-docs` and `anchor` write a scenario; `codebase-design` judges a seam; `tdd` lands a scenario red then green; `review` and `judge` give a verdict; `reduce` and `show-me` write the PR body. A change under `apps/desktop/src` adds `emil-design-eng`, and `percy-review` in a review run; a motion clause adds `animate`.

Browser tests run headless in a dedicated agent-browser session and a fresh test profile. Never attach to the user's browser or desktop: no `--auto-connect`, `--headed`, CDP connection, personal profile, or OS keystrokes. If headless launch fails, repair the isolated browser setup or record the observer failure; do not fall back to the user's Chrome. Close only tabs and browser processes this run created.

## Recipes

Each recipe says what its part of a PR holds. Read `.agents/data/work.md` for the canonical Issue format. A build run follows every recipe its Issue needs.

A UI Issue, or a Brief, Door or Decision Issue in the first user journey, also reads `.agents/data/journey.md`: its stage, rendered observer and next-gap rule apply to the build and the independent review. Work toward the selected journey, not merely an isolated green component test.

- **Spec**, for an Issue requesting a specification. Each id gets a `**<id> <name>.**` heading; every then-clause names the test that will assert it, or is marked policy; every check is a number citing its file and line on `main`; every cited id, file and PR exists on `origin/main`; a script has one row per failure with its exit code; the text states the result, never the incident; native Issue dependencies identify prerequisites; `loop/rules.sh vocab` passes.
- **Contract**, for a row needing `contracts/`, a new RPC method, an App seam command, `GLOSSARY.md` or `docs/adr/`. Every caller changes in the same PR and every new name is in `GLOSSARY.md`; a Message, Route, Held or Takeover change names the laws M1 to M10 it touches (`scenarios/proofs.md`) and keeps `proofs/messages` and `docs/messages.md` agreeing.
- **Code.** Tests are named after their scenario; `just check` passes; after any merge of `main`, `git diff origin/main --stat` lists only your files. A visible change meets D2, D5 and D6 at the scenario's viewport (`.agents/data/harness.md`), with failure text visible on the densest seed. A `crates/desktop` change needs the user's `just app` on a Mac: the PR body says so, and the user adds its `macOS:` line.

## Build run

1. Branch as the task says from `origin/main`. When the task holds a Trail, post its `Changed:` entry on the Issue before the first commit. Commit the spec or the red tests first, push, and open a draft PR as `.agents/data/pr.md` says, and link its work Issue.
2. Fulfil the Issue acceptance. A missing implementation prerequisite becomes a linked blocking Issue; a specification-only PR keeps the implementation Issue open.
3. `gh pr ready`, then `gh pr merge --auto --merge`.

A fix run starts with `gh pr ready --undo`, answers every finding or failure in the task with commits on the PR's branch, then ends with `just check`, a push and step 3.

A missing prerequisite is an engineering repair: identify the missing implementation and update the native Issue dependency. An isolated contract test does not complete a multi-scenario implementation Issue. Keep implementation within its Scope; an Architect or the user authorizes an ownership expansion. Contract approval requirements still apply.

Stop only on an unresolved product decision that changes the requested outcome and that no scenario, Item or `GLOSSARY.md` entry settles. Engineering uncertainty calls for investigation, a tested repair or a scoped prerequisite Issue under the existing direction. For that unresolved question, leave a draft with `Stopped: <the question>`, its user-visible consequence and one recommended choice; merge-ready asks it (L81).

## Review run

Your input is the task, the checkout of its head, `gh pr diff`, the scenarios it names and `.agents/data/gates.md`. You cannot post or push; your answer is the verdict.

1. `git rev-parse HEAD` equals the task's head, or reject with that finding.
2. Judge the diff against the recipes, the scenario and `AGENTS.md`, naming the rule per defect.
   - A visible change: the Checks it moves hold, and its Percy build shows no unintended diff, name, email or token.
   - A change to `crates/messages`, `docs/messages.md` or `proofs/`: `just proofs` (until it exists, `bend proofs/messages/PROOF.bend`) prints `ALL PROOFS CHECK`, and `proofs/messages/LAWS.bend` weakens no law.
   - A contract change: probe each new type and method in the checkout, never only its text.
   - A change to loop machinery: no rule or workflow lets a PR pass its own gate.
3. A re-review checks each earlier finding first, then what changed. Judge the head, not who pushed it.
4. Reject only a real defect: a broken rule, a contradiction, wrong behaviour, data loss, or a security hole (a secret, a token, an identity, a rule letting an Agent act beyond its role). Wording and taste are `NOTE:` lines in an approve.

Done: a build or fix run leaves a ready PR with auto-merge armed, or a draft with a `Stopped:` line; a review run answers `verdict` and `findings` in the format of `.agents/data/gates.md`.
