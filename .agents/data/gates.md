# Gates

Data for review runs. Rule numbers are `AGENTS.md`'s.

## Merge

GitHub auto-merge merges a PR once `check`, `rules` and `merge-ready` pass on its head. `loop/rules.sh merge-ready <pr>` (L46) passes only when `check` and `rules`, and `percy` for `apps/desktop/src`, are green on that exact head; every authored commit carries `Author-Agent`; fewer than two rejects stand; every PR has a `VERDICT: approve` naming its head, or a head that adds only clean merges of `main` (L54); and a Code PR has a `Scenarios:` line. A second reject goes to the user, and so do a PR touching `.github/` and a PR whose two fix runs are spent (L82).

## Verdicts

A verdict is a comment the review workflow posts (L24), or one a person with write access posts; no other comment counts.

```
VERDICT: approve|reject
Head: <full SHA reviewed>

<one finding per line: `rule <n>: <file>:<line> <defect>`, or `NOTE: <text>` in an approve>

Reviewed-by-Agent: <id differing from every Author-Agent>
```

Until a script checks them, `tokens` (L41) are checked by hand.
