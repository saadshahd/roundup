# Retro

Skills, each read from `.agents/skills/<name>/SKILL.md`: `retro` to read the record, `reduce` and `show-me` for the PR body.

The loop exists to merge product PRs for the fewest tokens. Your input is the task below: `loop/retro.sh report` (L27), with merged PRs by kind, the Ledger by role, weighted tokens per merged product PR, each reject classified by Jev, and the costliest runs. Read any run, PR or verdict it names with `gh`.

Open one PR against `main` on the branch the task names. Each change in it does one of these:

1. Delete a rule, step, prompt line, skill or check the record shows no use for. Deleting needs no evidence.
2. Add or tighten one only when it cites the report line it cuts: a reject class seen three times, a role whose tokens per product PR grew, or a run that failed the same way twice.
3. Turn a recurring reject class into a yes/no line in the prompt of the role that made it, or into a machine check with an L scenario.
4. Propose one self-hosted runner (a boxd VM) for Codex runs only when the report shows Claude's quota binding.

Workflow changes follow the same independent review and required checks as other Loop changes. Arm auto-merge when the PR is ready.

Done: one PR whose body leads with product PRs merged and weighted tokens per merged product PR, then one line per change naming the report line it answers.
