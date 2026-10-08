# First journey: keep the next work ready

Module: `scenarios/` and `.agents/data/journey.md`. Ids: J1. The destination and acceptance evidence are defined once in `.agents/data/journey.md`; this row makes its existing next-gap rule runnable.

**J1 audit the journey and queue its next missing behavior (policy).** Given current `origin/main` and the open PRs, inspect the five stages of `.agents/data/journey.md` in order. Run the applicable existing behavioral and rendered observers; record the main SHA, commands and artifact paths or run links for what actually passed. A test name, declared RPC, screenshot of a seeded success, or merged PR alone is not proof of a stage. State the first unproved behavior and the exact missing implementation or observer. Keep unobserved stages unproved.

Use an existing scenario and Work row when they cover that gap. Correct a stale dependency or missing row using only prerequisites whose implementation exists on main. Otherwise specify one bounded missing behavior, its real observer and one implementation Work row, in the file that owns the behavior. Mark its actual prerequisites in After; a missing implementation that a contract test happens to name is still a prerequisite. Keep implementation out of this audit PR. Settled product/design documents decide engineering choices; only a consequential product choice those documents do not settle is a question for the user.

The audit also leaves exactly one next audit row with the next unused J id, referring to this policy. Its After cell contains all ids of the implementation row whose delivery will let that audit make new observations. If that implementation is already in an open PR, wait on its row; do not duplicate its work. If it is independently blocked by another missing implementation, select that prerequisite as the next bounded work instead. Do not add an immediately-ready audit successor that would merely repeat this report. When all five stages have the required behavioral and rendered evidence on one main revision, record that evidence and leave no successor.

Observer: independent review compares the cited evidence with the stage acceptance conditions and the actual proposed Work row. `loop/rules.sh ready` must print the new implementation row's correct ready/waiting/in-flight state and the successor audit as waiting until its prerequisite exists. This is an evidence/specification task, so no test that merely searches this document counts as its completion. A later implementation's tests establish behavior; this policy claims no implementation or native observation.

## Work

| Ids | Item | Owns | Keeps green | After |
|---|---|---|---|---|
| J1 | observe the selected journey, specify its first uncovered behavior and leave one dependent next audit | `.agents/data/journey.md` for evidence and priority; `scenarios/**` only for the selected gap, its prerequisite rows and next audit; new evidence under `artifacts/ux/journey/` | existing scenario acceptance; no product code, seam implementation or weakened Check | H18 U113 |
