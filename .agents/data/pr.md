# PR

Data for Builder runs: a build or fix run writing a PR, a review run judging its body.

## Title and body

- The title starts with the Issue's scenario ids. Add `Refs #<issue>`; add `Closes #<issue>` only for fully delivered acceptance.
- `Scenarios: <ids>`; a `Principles: <ids>` line only when a `PRINCIPLES.md` id applies.
- `Moves: D<n>, …` for a change under `apps/desktop/src` (rule 8).
- `macOS: <what the user saw in just app>` for a change under `crates/desktop`, added by the user; Percy renders Chromium only, so WKWebView is the one laptop check.
- `Stopped: <the question>` on a draft its Builder leaves on a question only the user can answer; the end of its run asks the user (L81).
- Then only what a reader needs and CI does not show: no Shape, Proof, labels, history or boilerplate.

## Commits and branches

- Every authored commit carries `Author-Agent: <id>` (`human-saad` for the user's own) in its final trailer block, with no blank line before `Co-Authored-By` or another trailer; a clean merge of `main` needs none (L54), and a Copilot Autofix accepted on GitHub is authored by `copilot` (L37).
- A Builder's branch is `build/<key>`, named by its Issue; fix runs push to it. Historical spec-only PRs do not mark implementation done (L23).
- Issues merge in their native blocked-by dependency order. A stacked PR (its scenario says "after X" or "may stack") branches from its base, touches no file the base touches, and merges after it.
- Behind `main`: merge `origin/main` in (a stacked branch merges its base first), `just check`, push; never rebase a branch with a review in flight.
- Lockfile conflict: take main's file and regenerate (`git checkout origin/main -- Cargo.lock && cargo update -w`; `git checkout origin/main -- pnpm-lock.yaml && pnpm install`), committed with the merge.
- A row needing `contracts/`, `GLOSSARY.md` or the App seam changes them in its own PR, by the Contract recipe of `.agents/builder.md`.

## Audit PR

Every scenario id has a test prefixed with its lowercase id (`L` ids: `L<n>` in `loop/*.test.sh`), except W1 (`just check`) and F1, F7 (spike records). An audit adds one test per clause no test asserts, with an id, clause, test table in the PR; it changes no scenario or behaviour and creates or updates a work Issue for each unmet clause.
