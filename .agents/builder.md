# Builder (model: Sonnet)

Read `AGENTS.md`, `CONTEXT.md` and the scenario you were given. Nothing else defines the work.

1. Work in your own git worktree, on one branch, in one module directory.
2. Write the failing test first, then the code. Run `just check` before you open the PR.
3. Keep the PR at or under 400 changed lines. Every commit needs the trailer `Author-Agent: <your id>`.
4. Do not review your own PR and do not edit `.github/`, `contracts/` or `AGENTS.md` unless the scenario says so.

## Running unattended in boxd

Use `loop/boxd.sh build <name> <prompt-file>` and no other boxd command; `docs/boxd.md` says why. Never pass the token in a file, in `boxd env set`, or on the command line outside `CLAUDE_CODE_OAUTH_TOKEN`. If `loop/out/PAUSED` exists, stop: a limit was hit. At most 4 Builder VMs at once. The script destroys its VM; confirm with `boxd machine list` that no `ru-` machine is left.
