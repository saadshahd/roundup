# Work orders

GitHub Issues labelled `loop:work` are the only work orders. Scenario files hold acceptance specifications, never scheduling tables. Search existing Issues and PRs before creating work; update the existing Issue when it already owns the gap.

An Issue contains an outcome, scope, regression requirements, and these single-line fields:

```text
Scenarios: U83
Specification: [scenarios/ui-surfaces.md](https://github.com/saadshahd/roundup/blob/main/scenarios/ui-surfaces.md)
Key: U83
Priority: 1
Mode: implement
```

Mode is `implement` or `specify`; a specification Issue may propose new scenario headings and files, while an implementation Issue requires them on main. An omitted Mode is `implement`.

An Issue with no `Key:` line is a request, such as a bug report: the queue builds it as `Mode: specify` with Key `issue-<number>` and Priority 50. That build run adds the request's scenarios, then edits the Issue to add the fields above (L34).

Key is an immutable branch identity: letters, digits and single hyphens; its Builder branch is `build/<key>`. Priority is 0 (first) through 100 (last). Native GitHub blocked-by dependencies hold Issue numbers; only a dependency closed as completed frees work. Cancelled dependencies, malformed orders and duplicate Keys need engineering repair, not user escalation. A dependency cycle must be corrected before execution.

Add `ready-for-agent` once scope, acceptance and native dependencies are settled. Only repository collaborators can apply that intake label. Routine engineering decisions use the accepted product/design documents. New specifications must merge before their implementation Issue is eligible: make it blocked by the specification Issue and close that Issue from its specification PR.

A Builder reads its Issue and names it with `Refs #<number>` in the PR. Use `Closes #<number>` only when the Issue's acceptance is demonstrated. A specification-only or partial PR leaves implementation work open. A test name, deleted branch or merged unrelated PR cannot complete work. Put genuine product questions on the Issue and remove `ready-for-agent`; keep engineering repairs in the loop.

GitHub Issues own priority, dependencies and progress. `.agents/data/journey.md` owns the goal and acceptance evidence. `python3 loop/orders.py ready --json` prints the current derived queue; an API failure stops intake visibly.
