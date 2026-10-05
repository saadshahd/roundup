# PR

Data for the Builder, and for the Reviewer judging a body.

## Body

- `Scenarios: <ids>` and the `PRINCIPLES.md` ids it serves.
- `## Shape`: the smallest call, component or file tree, sequence or diff that makes the change clear, at most 40 lines (`show-me`).
- `## Proof`: the full head SHA and a fenced test-output block with a passing line per scenario id.
  - `apps/desktop/src` change (L76): `Percy: https://percy.io/…/builds/<n>`, the link the head's `percy` job writes to its summary (L36). No images.
  - `crates/desktop` change (L76): `macOS: <what you saw in just app>`; Percy renders Chromium only, so WKWebView is the one laptop check.
  - Specification only: `Proof scope: specification`, fenced output with `just check: exit 0`, `Specification consistency: <review>`, one `Pending <id>: <observer>` per scenario. Never for production, contract, glossary, policy or mixed changes.
- `Moves: D<n>, …` for a change under `apps/desktop/src` (rule 8).

## Labels (L57, by hand)

One `kind:` (`feature fix docs scenario prompt loop spike`), one or more `area:`, one `lane:` matching `loop/rules.sh class`, one `owner:<id>`; `flag:needs-user` for an `AGENTS.md` or repository-setting edit.

## Commits and branches

- Every authored commit carries `Author-Agent: <id>`.
- Rows merge in their `After` order. A stacked PR (its scenario says "after X" or "may stack") branches from its base, touches no file the base touches, and merges after it.
- Behind `main`: merge `origin/main` in (a stacked branch merges its base first), `just check`, push, ask for a new approval; never rebase a branch with an approval or a review in flight.
- Lockfile conflict: take main's file and regenerate (`git checkout origin/main -- Cargo.lock && cargo update -w`; `git checkout origin/main -- pnpm-lock.yaml && pnpm install`), committed with the merge.
- A defect needing `contracts/`, `GLOSSARY.md` or the App seam stops the Builder and goes to an Architect.

## Audit PR

Every scenario id has a test prefixed with its lowercase id (`L` ids: `L<n>` in `loop/*.test.sh`), except W1 (`just check`) and F1, F7 (spike records). An audit adds one test per clause no test asserts, with an id, clause, test table in the PR; it changes no scenario or behaviour and reports an unmet clause to an Architect.
