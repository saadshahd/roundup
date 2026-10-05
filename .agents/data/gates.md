# Gates

Data for the Reviewer, Driver and Architect. Rule numbers are `AGENTS.md`'s.

## Lanes

`loop/rules.sh class <pr>` (L44) prints `block` or `post`; unsure → block.

| Lane | Merges on | After merge |
|---|---|---|
| block | `merge-ready` (L46): CI `check` and `rules` green on the exact head, base `main`, an independent approval commit (carried across `main` merges by L54), `proof` (L53, with L76's passing `percy` check and Percy link for a visible PR), fewer than 3 rejects or an Architect pick (`rounds`, L45) | — |
| post | `merge-ready`: green CI, base `main`, `Author-Agent` trailers | an independent verdict within 60 minutes (a Stall, L28); a newest-verdict reject 60 minutes after merge is reverted (L48), unless a newer independent approve on the same head withdraws it |

By hand until their scripts exist: `revert-due` (L48), `dispatch` (L49: a scenario's newest change merged block-lane or with an independent approve), labels (L57), `tokens` (L41), contract approval (rule 4).

## Verdicts

- Comment, first line `VERDICT: approve` or `VERDICT: reject`, then: the full SHA reviewed; commands run with exit codes; mutations tried; every finding with its rule; for a reject, `complete: <n> findings, <m> mutants run` (L63); a line `Reviewed-by-Agent: <id>` differing from every `Author-Agent`.
- Block lane approval: then an empty commit carrying only `Reviewed-by-Agent: <id>`. Post lane: no commit.
- Contract approval: `ARCHITECT: approve <full sha>` (or `reject`); after a 3rd reject, `ARCHITECT: split`, `amend` or `retire`.
