# Worktrees

Design for `scenarios/worktrees.md` (G1 to G7). Terms: Worktree, Base, Landing (`GLOSSARY.md`).

## Principles served

| Id | Gate and answer |
|---|---|
| P2 | Does a new path let two Agents with Kind `working` share one cwd? Yes while a Project's `worktrees` setting is off, which is the default and `docs/wireframes.md` decision 12. With it on, no: G2's test spawns two Agents, promotes two Meta-agents and compares their cwds; `agent.spawn` and `rail.promote` are the two paths that start an Agent. P2 stays a goal, as it says. |
| P4 | Does code outside `crates/agents/<vendor>/` read a vendor string, and does `AgentAdapter` gain a seam? No to both. Provisioning runs in `crates/agents` before the Agent's Terminal starts, and the Adapter trait gains nothing: the working directory it is given is the Worktree. |
| P5 | Does a path write the Base other than rebase, check, fast-forward? No: `agent.land` is the only writer. No gap: with the Project's `check` `null`, `agent.land` refuses (`check_missing`) instead of skipping, because P5 requires "check green" and nothing in it allows a skip. Isolation (G2) needs no check, so the setting may be on without one. Does a rejected Agent leave a worktree or branch? No: G5's test compares `git worktree list` and `git branch` before and after. |

P5 says "a dropped or rejected Agent leaves no worktree and no branch". The user's rule here is that removal never deletes unlanded or dirty work. Both hold: `rail.remove` refuses when work would be lost (`worktree_unlanded`), and `agent.discard` is the deliberate drop that deletes it. P5's "dropped" means `agent.discard`. The user confirmed this reading.

## Shape

- **Library.** The `git` command, run with `std::process::Command` and no shell, in `crates/agents/src/worktree.rs`. `git status --porcelain=v2 --branch` gives branch, ahead, behind and dirty in one call; `git worktree add`, `git rebase` and `git merge --ff-only` are the same operations a user runs, and the user's hooks and config apply. `git2` is rejected: a native dependency, and its rebase reports conflicts differently from git. The cost is `git` on `PATH`; a missing one is `worktree_failed`.
- **State.** The `nodes` table (`crates/agents/src/rail.rs`) gains `worktree_path`, `worktree_branch`, `worktree_base`, all null for a node with no Worktree, and `worktree_state` (`provisioning` from before `git worktree add` runs until the Worktree exists and just before the Agent's Terminal starts, then `ready`; no RPC returns it). A restart rolls back only a `provisioning` node, so it never deletes a Worktree an Agent has begun to use. Live status (ahead, behind, dirty) is never stored: `agent.worktreeState` asks git on each call, so `rail.tree` stays cheap with ten Agents.
- **Setting.** One row in a new `settings` table of `agents.db`, the file the Agents module opens in the Project's `.roundup/` (`worktrees` on/off, `check` command or null), written by `project.setWorktrees`, read by `project.get`. `docs/architecture.md` still says one `roundup.db`; each module has its own file, and this doc does not edit that line. The G1 to G3 row owns `agents.db`.
- **Home is logical, the path is not.** An Agent's Home (`GLOSSARY.md`: the Group or Meta-agent it sits in, stored as the node's `parent` and `order`, a row in SQLite) and its Worktree's `worktree_path` (a directory chosen once, from the Agent's id) never derive from each other. A `rail.move` writes the first and never reads or writes the second, so a moved Agent keeps its working directory, its uncommitted edits and its `git diff` byte for byte (G7). Restart finds Worktrees by the stored path (G6), not by Rail position. A Worktree cannot change Project: a Rail belongs to one Project (one Daemon, one database), so a cross-Project `parent` is `NOT_FOUND`, and a repository is one Project's.
- **Where.** `<Project folder>/.roundup/worktrees/agent-<id>`, inside the Project folder because A4 pre-trusts only a working directory inside it. `.roundup/` goes in `.git/info/exclude`, which is local, so the user's tracked files and `git status` do not change.
- **Landing.** In the Worktree: `git rebase <base>`; on conflict `git rebase --abort` and report the paths (`git diff --name-only --diff-filter=U`, read before the abort). Then the `check` command with `sh -c` in the Worktree. Then `git -C <Project folder> merge --ff-only <branch>`. Each step records the branch's starting commit, so a later failure resets it. The Daemon runs one `git` command at a time per Project (a mutex in `worktree.rs`), because `git worktree add`, `rebase` and `merge` take the repository's locks and two Agents spawning at once would otherwise fail on `index.lock`. A node is stored as `provisioning` before `git worktree add` runs and set to `ready` before the Terminal starts, so a Daemon killed in between is rolled back on the next start (G6).

## Contract change (separate PR, rule 4)

| Method | Params | Result |
|---|---|---|
| `project.setWorktrees` | `{on, check}` | `null` |
| `project.get` | `null` | `{worktrees: {on, check}}` |
| `agent.worktreeState` | `{id}` | `{ahead, behind, dirty}` |
| `agent.land` | `{id}` | `{base}` |
| `agent.discard` | `{id}` | `null` |

`RailNode` gains `worktree: {path, branch, base} | null`; every caller of `RailNode` (the webview's `railFixture`, `seeds.ts`, the e2e tests) changes in that PR. `rail.remove` gains the error `worktree_unlanded` (A16 does not; it is G5's), and `agent.spawn` and `rail.promote` the codes of G2. The error codes are existing `rpc::code` constants with a lowercase prefix in the message (see `scenarios/worktrees.md`), so `crates/rpc` is untouched. `agent.worktreeState` returns `{ahead, behind, dirty}`, which is not a Status (`GLOSSARY.md`), so its name avoids that word. The Agents module owns the `project.*` methods (it adds `project` to its namespaces; the App's `project` command is a seam command, not RPC). No UI scenario is written here: a Rail badge for ahead, behind and dirty, and a Landing button, come after this lands.

## Open for the user

- `~/.claude.json` keeps a pre-trust entry (A4) for each Worktree path; discarding or removing a Worktree does not clean it, and a Worktree path is never reused, so the entries are only clutter.

- The `check` command runs with the user's shell and rights. It is only what the user typed into their own Project's setting.
- A Project nested in another repository uses the repository that `git rev-parse --show-toplevel` finds; G2 does not cover a Project that is a subfolder of a repository. Say if that must work.

## Considered alternatives

| | A. Daemon, on for every git Project | B. Daemon, behind the setting (this design) | C. caller or helper provisions; the Daemon takes a cwd | D. the vendor's own worktree option |
|---|---|---|---|---|
| Isolation guarantee | Yes for every Agent | Yes when on; none when off (P2 stays a goal) | None the Daemon can check: P2's observer is a hope | Claude Code only |
| Crash between add and spawn | Rolled back on start (G6) | Same | A script's problem; orphans nobody owns | The vendor's, and invisible to roundup |
| Orphans, dirty removal | `rail.remove` refuses; `agent.discard` is explicit | Same | No rule; a helper may delete work | The vendor asks the user on exit, inside the Terminal |
| Disk per worktree (`node_modules`, `target`) | Every Project pays it | Only an opted-in Project | The caller's choice | Vendor symlink settings, by its docs |
| Restart recovery | Stored paths, G6 | Same | None | Guess the vendor's directory layout |
| Daemon complexity, perf (rule 7) | One module, git at spawn only | Same, plus one setting | None | A parser of vendor behaviour |
| Fit with `AgentAdapter` (P4) | Vendor-neutral | Vendor-neutral | Vendor-neutral | Breaks P4: each vendor has its own or none |
| Surprise to the user | Spawns slow down, disk grows, in a Project that never asked | Only when switched on | Whatever the helper does | An unrelated prompt when a Terminal closes |

Rule 7: cold start and idle RSS do not change in A, B or D, since `git` runs only at spawn, landing and status, never in `rail.tree`, and holds no memory between calls. Not measured: `git worktree add` time on a large repository, and disk per worktree. Neither is in the budget, but both are what A would make every user pay.

Claude Code has `--worktree <name>` (`code.claude.com/docs/en/worktrees`, read 2026-10-02): it creates `.claude/worktrees/<name>` on `worktree-<name>`, from the default branch unless `worktree.baseRef` is `head`, and in an interactive session it asks whether to keep or remove a worktree that holds work. It lives in the user's `.claude/`, names its own branch, has no landing and no Base roundup can read without learning its layout. It does not meet P4, and G2 to G6 could not be tested without the real `claude`. It is not a substitute; nothing stops a user from using it by hand in a Terminal.

**Recommendation: B, confirmed by the user.** It is the only option that gives a testable isolation guarantee, vendor neutrality and recovery without making every Project pay. Evidence that would change it:
- Measured disk and spawn time that are small on the user's real Projects would favour A, and the setting's default could flip then.
- Agents sharing a cwd in practice (a lost edit, a corrupt index) would favour A.
- A second vendor with its own good worktree support, with a way to read its branch, base and path, would favour D for that vendor's Adapter.
