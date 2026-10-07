# PR

Data for the Builder, and for the Reviewer judging a body.

## Title and body

- The title starts with the Work row's ids: `loop/rules.sh ready` reads them to mark the row in flight.
- `Scenarios: <ids>`; a `Principles: <ids>` line only when a `PRINCIPLES.md` id applies.
- `Moves: D<n>, …` for a change under `apps/desktop/src` (rule 8).
- `macOS: <what the user saw in just app>` for a change under `crates/desktop`, added by the user; Percy renders Chromium only, so WKWebView is the one laptop check.
- Then only what a reader needs and CI does not show: no Shape, Proof, labels, history or boilerplate.

## Commits and branches

- Every authored commit carries `Author-Agent: <id>` (`human-saad` for the user's own); a clean merge of `main` needs none (L54), and a Copilot Autofix accepted on GitHub is authored by `copilot` (L37).
- A Builder's branch is `build/<slug>`, named in its task; a branch left behind with no open PR stops its row from building again until the user deletes it (L23).
- Rows merge in their `After` order. A stacked PR (its scenario says "after X" or "may stack") branches from its base, touches no file the base touches, and merges after it.
- Behind `main`: merge `origin/main` in (a stacked branch merges its base first), `just check`, push; never rebase a branch with a review in flight.
- Lockfile conflict: take main's file and regenerate (`git checkout origin/main -- Cargo.lock && cargo update -w`; `git checkout origin/main -- pnpm-lock.yaml && pnpm install`), committed with the merge.
- A defect needing `contracts/`, `GLOSSARY.md` or the App seam stops the Builder: its draft PR's body says what an Architect run must settle.

## Audit PR

Every scenario id has a test prefixed with its lowercase id (`L` ids: `L<n>` in `loop/*.test.sh`), except W1 (`just check`) and F1, F7 (spike records). An audit adds one test per clause no test asserts, with an id, clause, test table in the PR; it changes no scenario or behaviour and reports an unmet clause to an Architect.
