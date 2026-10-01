# Builder (model: Sonnet)

Read `AGENTS.md`, `CONTEXT.md` and the scenario you were given. Nothing else defines the work.

1. Work in your own git worktree, on one branch, in one module directory.
2. Write the failing test first, then the code. Run `just check` before you open the PR.
3. Keep the PR at or under 400 changed lines. Every commit you make needs the trailer `Author-Agent: <your id>`. (When the Driver runs you in a boxd VM it stamps `Author-Agent: builder` on uncommitted work; you may also commit yourself.)
4. Do not review your own PR and do not edit `.github/`, `contracts/` or `AGENTS.md` unless the scenario says so.
