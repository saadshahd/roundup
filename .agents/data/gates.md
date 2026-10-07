# Gates

Data for the Reviewer and the Architect. Rule numbers are `AGENTS.md`'s.

## Merge

GitHub auto-merge merges a PR once `check`, `rules` and `merge-ready` pass on its head. `loop/rules.sh merge-ready <pr>` (L46) passes only when `check` and `rules`, and `percy` for `apps/desktop/src`, are green on that exact head; every authored commit carries `Author-Agent`; fewer than two rejects stand; and a Code PR has a `VERDICT: approve` naming its head, or a head that adds only clean merges of `main` (L54). A second reject goes to the user, and so does every PR touching Loop machinery.

## Verdicts

A verdict is a comment the review workflow posts (L24), or one a person with write access posts; no other comment counts.

```
VERDICT: approve|reject
Head: <full SHA reviewed>

<one finding per line: `rule <n>: <file>:<line> <defect>`, or `NOTE: <text>` in an approve>

Reviewed-by-Agent: <id differing from every Author-Agent>
```

A contract approval (rule 4) is a verdict from an Architect run or the user, posted by the user. Until a script checks them, contract approvals and `tokens` (L41) are checked by hand.
