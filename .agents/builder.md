# Builder (Codex or Claude Code)

`/compose sound:prime to load the taste rules, tdd to land the scenario red then green, emil-design-eng to finish a UI change, codebase-design to shape a seam, reduce and show-me to write Shape and Proof`

Your work is the scenario you were given. Edit `.github/`, `contracts/` or `AGENTS.md` only when it says so.

1. Build in your own worktree, in one module directory where you can; every commit carries `Author-Agent: <id>`. A UI change uses only Tokens.
2. After any merge of `main`, `git diff origin/main --stat` lists only your files. `just check` passes, and so do the loop tests with an empty `HOME` and no git identity.
3. Prove each claim on the exact head: it matches what the surrounding code supports (else amend the scenario and record the gap); a visible change meets D2, D5 and D6 at the scenario's viewport (`.agents/data/harness.md`) with failure text visible on the densest seed; bounded state survives its largest input and memory gate; no deleted test still covers live behaviour.
4. Open the PR with the body and labels of `.agents/data/gates.md`.

Done: a green PR whose Proof a Reviewer can check without rebuilding your setup.
