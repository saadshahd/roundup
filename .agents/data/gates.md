# Gates

Data for the Builder, Reviewer and Driver. Rule numbers are `AGENTS.md`'s.

## Lanes

`loop/rules.sh class <pr>` (L44) prints `block` or `post`; unsure → block.

| Lane | Merges on | After merge |
|---|---|---|
| block | `merge-ready` (L46): CI `check` and `rules` green on the exact head, base `main`, an independent approval commit (carried across `main` merges by L54), `proof` (L53), fewer than 3 rejects or an Architect pick (L45) | — |
| post | `merge-ready`: green CI, base `main`, `Author-Agent` trailers | an independent verdict within 60 minutes (a Stall, L28); a newest-verdict reject 60 minutes after merge is reverted (L48), unless a newer independent approve on the same head withdraws it |

By hand until their scripts exist: `revert-due` (L48), `dispatch` (L49: a scenario's newest change merged block-lane or with an independent approve), labels (L57), `tokens` (L41), contract approval (rule 4).

## PR body

- `Scenarios: <ids>` and the `PRINCIPLES.md` ids it serves.
- `## Shape`: the smallest call, component or file tree, sequence or diff that makes the change clear, at most 40 lines (`show-me`).
- `## Proof`: the full head SHA and a fenced test-output block with a passing line per scenario id.
  - Visible change: a `Shows:` line and one line per scenario and capture, e.g. `U1 before [image](https://github.com/OWNER/REPO/blob/proof/pr-12/proof/before.png) 1280×800`, at a window size the scenario names. `proof/pr-<n>` holds only media under `proof/`.
  - Specification only: `Proof scope: specification`, fenced output with `just check: exit 0`, `Specification consistency: <review>`, one `Pending <id>: <observer>` per scenario. Never for production, contract, glossary, policy or mixed changes.
  - Unchanged rendering (L76): no proof branch when the independent approve carries `Visual: unchanged`, `Reviewed-head: <sha>` and `Reviewed-by-Agent: <id>` on an ancestor with the head's tree.
- `Moves: D<n>, …` for a change under `apps/desktop/src` (rule 8).

## Labels (L57)

One `kind:` (`feature fix docs scenario prompt loop spike`), one or more `area:`, one `lane:` matching `class`, one `owner:<id>`; `flag:needs-user` for an `AGENTS.md` or repository-setting edit.

## Trailers and verdicts

- Every authored commit: `Author-Agent: <id>`.
- Verdict comment, first line `VERDICT: approve` or `VERDICT: reject`, then: the full SHA reviewed; commands run with exit codes; mutations tried; every finding with its rule; for a reject, `complete: <n> findings, <m> mutants run` (L63); a line `Reviewed-by-Agent: <id>` differing from every `Author-Agent`.
- Block lane approval: then an empty commit carrying only `Reviewed-by-Agent: <id>`. Post lane: no commit.
- Contract approval: `ARCHITECT: approve <full sha>` (or `reject`); round-cap picks: `ARCHITECT: split`, `amend`, `retire`.

## Rejects

| Reject | Then |
|---|---|
| 1st | a free architect other than the author makes one push in the Builder's branch (L63) |
| re-review | the brief carries every earlier `VERDICT:` comment and the diff since the rejected head; unchanged text blocks only for a broken rule or a correctness defect |
| 3rd | `rounds` (L45) blocks until an Architect other than the author picks; the user picks when the author is the only Architect |

## Branches

- Rows merge in their `After` order. A stacked PR (its scenario says "after X" or "may stack") branches from its base, touches no file the base touches, and merges after it.
- Behind `main`: merge `origin/main` in (a stacked branch merges its base first), `just check`, push, ask for a new approval; never rebase a branch with an approval or a review in flight.
- Lockfile conflict: take main's file and regenerate (`git checkout origin/main -- Cargo.lock && cargo update -w`; `git checkout origin/main -- pnpm-lock.yaml && pnpm install`), committed with the merge.
- A defect needing `contracts/`, `GLOSSARY.md` or the App seam stops the Builder and goes to an Architect.

## Audit PR

Every scenario id has a test prefixed with its lowercase id (`L` ids: `L<n>` in `loop/*.test.sh`), except W1 (`just check`) and F1, F7 (spike records). An audit adds one test per clause no test asserts, with an id, clause, test table in the PR; it changes no scenario or behaviour and reports an unmet clause to an Architect.

## Stops

`main` red over 30 minutes · `loop/out/PAUSED` exists · a contract change without architect approval · the same perf budget breached twice.
