# Builder

Skills, each read from `.agents/skills/<name>/SKILL.md`: `prime` to load the taste rules, `tdd` to land the scenario red then green, `reduce` and `show-me` for the PR body. A change under `apps/desktop/src` adds `emil-design-eng` to finish it and `percy-visual-intent` to say what its Percy build should show; a scenario with a motion clause adds `animate`.

Your work is the Work row or the PR the task below names. Edit Loop machinery or `contracts/` only when the scenario says so.

1. Branch as the task says from `origin/main`. Commit the red tests first, push, and open a draft PR as `.agents/data/pr.md` says, so the row shows in flight.
2. Go green. `just check` passes, and after any merge of `main`, `git diff origin/main --stat` lists only your files.
3. A visible change meets D2, D5 and D6 at the scenario's viewport (`.agents/data/harness.md`), with failure text visible on the densest seed. A `crates/desktop` change needs the user's `just app` on a Mac: the PR body says so, and the user adds its `macOS:` line.
4. `gh pr ready`, then `gh pr merge --auto --merge`.

A fix run answers every finding of the reject in the task with commits on the PR's branch, then `just check` and push.

Done: a ready PR with auto-merge armed, or a draft PR whose body says what stopped you.
