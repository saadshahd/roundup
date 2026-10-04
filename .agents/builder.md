# Builder (Codex or Claude Code)

Your work is the scenario you were given; `AGENTS.md` and `CONTEXT.md` frame it.

1. Invoke `sound:prime`, then `tdd`. Stay in your own worktree and branch, in one module directory where you can. A UI change: also invoke `emil-design-eng` and read `docs/design-system.md`; every colour, size, radius, shadow and duration is a Token. A seam change: invoke `codebase-design`.
2. Every commit carries `Author-Agent: <your id>` (in a boxd VM the Driver stamps `builder` on uncommitted work). Edit `.github/`, `contracts/` or `AGENTS.md` only when the scenario says so.
3. After any merge of `main`, `git diff origin/main --stat` lists only your files. Run `just check`, and the loop tests with an empty `HOME` and no git identity, as CI does.
4. Before review, prove each claim on the exact head:
   - Each scenario claim matches what the surrounding code supports; a conflict amends the scenario and queues the missing behaviour.
   - A visible change: D2 spacing and type, D5 contrast and D6 hit areas measured at the scenario's viewport, in `just harness` on its seeds (`window.__checks()` once U137 lands, else `agent-browser eval`); failure text on the densest seed stays visible beside pinned controls; `Moves: D<n>, …` in the body.
   - Bounded state: the largest accepted input, and the named memory gate.
   - No deleted test still covers live behaviour.
5. Open the PR with a body of `## Shape` (the shortest call, component or file tree, sequence or diff that makes the change clear, at most 40 lines) and `## Proof` (the scenario's test output and every measured result; for a visible change, before and after images at the scenario's window size). Proof images go under `artifacts/ux/<scenario>/`; from an `--isolated` VM copy them out with `boxd machine cp` for the Driver to push to `proof/pr-<n>`.
6. Label it (L57): one `kind:`, at least one `area:`, one `lane:` matching `loop/rules.sh class <pr>`, and `flag:needs-user` for an `AGENTS.md` or repository-setting edit.
