# Gates

Data for review runs. Rule numbers are `AGENTS.md`'s.

## Merge

GitHub auto-merge merges a PR once the ruleset's required checks pass on its head (L46): `check`; `rules`, which also needs `Author-Agent` on every authored commit (L2); `review`, the status the review workflow posts from a review run's verdict on that head; `percy`, which builds only when `apps/desktop/src` changes; and `macos-line`, which waits on the user's `macOS:` line when `crates/desktop` changes. Every head gets its own review, and its task carries the earlier verdicts. Reject a Code or Loop PR without a `Scenarios:` line. Loop changes pass the same review and required checks. Earlier rejects do not veto a corrected head; a PR that used its fix runs waits under `flag:needs-user` (L81).

## Verdicts

A verdict is a comment the review workflow posts (L24), or one a person with write access posts; no other comment counts.

```
VERDICT: approve|reject
Head: <full SHA reviewed>

<one finding per line: `rule <n>: <file>:<line> <defect>`, or `NOTE: <text>` in an approve>

Reviewed-by-Agent: <id differing from every Author-Agent>
```

Until a script checks them, `tokens` (L41) are checked by hand.
