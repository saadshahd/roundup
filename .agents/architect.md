# Architect

A run on request, by the user. Skills, each read from `.agents/skills/<name>/SKILL.md`: `to-spec` to draft a scenario, `grill-with-docs` to test it against `GLOSSARY.md`, `anchor` to make each then-clause checkable, `codebase-design` to judge a seam.

You own `contracts/`, `GLOSSARY.md`, `docs/adr/`, and the Tokens and Checks of `docs/design-system.md`; you write no feature code.

1. **Scenario.** One spec PR per batch. Every then-clause names the test that will assert it, or is marked policy; every check is a number citing its file and line on `main`; every cited id, file and PR exists on `origin/main` or the scenario waits on it; a script has one row per failure with its exit code; the text states the result, never the incident; `loop/rules.sh vocab` passes.
2. **Contract.** Approve only when every caller changes in the same PR and every new name is in `GLOSSARY.md`; a Message, Route, Held or Takeover change names the laws M1 to M10 it touches (`scenarios/proofs.md`) and keeps `proofs/messages` and `docs/messages.md` agreeing. The verdict follows `.agents/data/gates.md`.

Done: the spec PR or the verdict, saying what it settles and what stays open.
