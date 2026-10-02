# Worktrees

Design for `scenarios/worktrees.md` (G1 to G6). Terms: Worktree, Base, Landing (`CONTEXT.md`).

## Principles served

| Id | Gate and answer |
|---|---|
| P2 | Does a new path let two Agents with Kind `working` share one cwd? Yes while a Project's `worktrees` setting is off, which is the default and `docs/wireframes.md` decision 12. With it on, no: G2's test spawns two Agents and compares their cwds. P2 stays a goal, as it says. |
| P4 | Does code outside `crates/agents/<vendor>/` read a vendor string, and does `AgentAdapter` gain a seam? No to both. Provisioning runs in `crates/agents` before `AgentAdapter::spawn`, which still receives a `cwd`. |
| P5 | Does a path write the Base other than rebase, check, fast-forward? No: `agent.land` is the only writer. One gap: when the Project's `check` is `null`, Landing runs no check, so P5's "check green" step is the user's choice, not enforced. Does a rejected Agent leave a worktree or branch? No: G5's test compares `git worktree list` and `git branch` before and after. |

P5 says "a dropped or rejected Agent leaves no worktree and no branch". The user's rule here is that removal never deletes unlanded or dirty work. Both hold: `rail.remove` refuses when work would be lost (`worktree_unlanded`), and `agent.discard` is the deliberate drop that deletes it. P5's "dropped" means `agent.discard`.

## Shape

- **Library.** The `git` command, run with `std::process::Command` and no shell, in `crates/agents/src/worktree.rs`. `git status --porcelain=v2 --branch` gives branch, ahead, behind and dirty in one call; `git worktree add`, `git rebase` and `git merge --ff-only` are the same operations a user runs, and the user's hooks and config apply. `git2` is rejected: a native dependency, and its rebase reports conflicts differently from git. The cost is `git` on `PATH`; a missing one is `worktree_failed`.
- **State.** The `nodes` table (`crates/agents/src/rail.rs`) gains `worktree_path`, `worktree_branch`, `worktree_base`, all null for a node with no Worktree. Live status (ahead, behind, dirty) is never stored: `agent.worktreeStatus` asks git on each call, so `rail.tree` stays cheap with ten Agents.
- **Setting.** One row per Project in a new `settings` table of `.roundup/roundup.db` (`worktrees` on/off, `check` command or null), written by `project.setWorktrees`, read by `project.get`. The Daemon opens that database already (`docs/architecture.md`, Data).
- **Where.** `<Project folder>/.roundup/worktrees/agent-<id>`, inside the Project folder because A4 pre-trusts only a working directory inside it. `.roundup/` goes in `.git/info/exclude`, which is local, so the user's tracked files and `git status` do not change.
- **Landing.** In the Worktree: `git rebase <base>`; on conflict `git rebase --abort` and report the paths (`git diff --name-only --diff-filter=U`, read before the abort). Then the `check` command with `sh -c` in the Worktree. Then `git -C <Project folder> merge --ff-only <branch>`. Each step records the branch's starting commit, so a later failure resets it.

## Contract change (separate PR, rule 4)

| Method | Params | Result |
|---|---|---|
| `project.setWorktrees` | `{on, check}` | `null` |
| `project.get` | `null` | `{worktrees: {on, check}}` |
| `agent.worktreeStatus` | `{id}` | `{ahead, behind, dirty}` |
| `agent.land` | `{id}` | `{base}` |
| `agent.discard` | `{id}` | `null` |

`RailNode` gains `worktree: {path, branch, base} | null`; every caller of `RailNode` (the webview's `railFixture`, `seeds.ts`, the e2e tests) changes in that PR. `rail.remove` gains the error `worktree_unlanded`, and `agent.spawn` the codes of G2. No UI scenario is written here: a Rail badge for ahead, behind and dirty, and a Landing button, come after this lands.

## Open for the user

- The `check` command runs with the user's shell and rights. It is only what the user typed into their own Project's setting.
- A Project nested in another repository uses the repository that `git rev-parse --show-toplevel` finds; G2 does not cover a Project that is a subfolder of a repository. Say if that must work.
