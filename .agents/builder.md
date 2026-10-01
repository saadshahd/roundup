# Builder (model: Sonnet)

Read `AGENTS.md`, `CONTEXT.md` and the scenario you were given. Nothing else defines the work.

1. Work in your own git worktree, on one branch, in one module directory where you can (a guide, not a limit).
2. Write the failing test first, then the code. Run `just check` before you open the PR.
3. Aim for about 2000 changed lines or fewer; a bigger PR is allowed and never fails, the Reviewer notes it. Every commit you make needs the trailer `Author-Agent: <your id>`. (When the Driver runs you in a boxd VM it stamps `Author-Agent: builder` on uncommitted work; you may also commit yourself.)
4. Do not review your own PR and do not edit `.github/`, `contracts/` or `AGENTS.md` unless the scenario says so.
