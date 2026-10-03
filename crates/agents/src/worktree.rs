//! Git worktrees for Agents (G1/G2, `scenarios/worktrees.md`). The `git` command, run with
//! `std::process::Command` and no shell: `git worktree add` and `git branch` are the same
//! operations a user runs, and the user's own hooks and config apply (`docs/worktrees.md`).
//! `git2` is rejected there: a native dependency whose rebase reports conflicts differently.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use rpc::{RpcError, code};

/// What provisioning makes. `RailNode.worktree` (`contracts::agent::Worktree`) stores the same
/// three values as strings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: String,
    pub base: String,
}

/// Runs one Project's `git`, one command at a time: `git worktree add` and the rest take the
/// repository's locks, and two Agents provisioned at once would otherwise fail on `index.lock`.
#[derive(Clone)]
pub struct Git {
    /// Resolves the `git` binary. A test prepends a wrapper's directory, with the real `PATH`
    /// after it, so every command the wrapper does not fake still reaches the real `git`.
    path: OsString,
    lock: Arc<Mutex<()>>,
}

impl Git {
    pub fn from_env() -> Self {
        Self::with_path(std::env::var_os("PATH").unwrap_or_default())
    }

    pub fn with_path(path: impl Into<OsString>) -> Self {
        Self {
            path: path.into(),
            lock: Arc::new(Mutex::new(())),
        }
    }

    fn run(&self, project: &Path, args: &[&str]) -> Result<String, String> {
        let _hold = self
            .lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let output = Command::new("git")
            .current_dir(project)
            .env("PATH", &self.path)
            .args(args)
            .output()
            .map_err(|err| err.to_string())?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(if stderr.is_empty() {
                format!("git {} failed", args.join(" "))
            } else {
                stderr
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    /// Branch `roundup/agent-<id>` at `project`'s current commit, and a worktree for it at
    /// `<project>/.roundup/worktrees/agent-<id>`; `.roundup/` is added to `.git/info/exclude`
    /// if it is not there. Every step is undone on any failure, so `git worktree list` and
    /// `git branch` are left as they were before the call.
    pub fn provision(&self, project: &Path, id: &str) -> Result<Worktree, RpcError> {
        let commit = self.run(project, &["rev-parse", "HEAD"]).map_err(|_| {
            invalid(
                "not_a_git_project",
                format!("{} is not a git project with a commit", project.display()),
            )
        })?;
        let base = self
            .run(project, &["symbolic-ref", "--short", "HEAD"])
            .map_err(|_| invalid("no_base", "HEAD is detached"))?;
        let branch = format!("roundup/agent-{id}");
        let path = worktree_path(project, id);

        self.run(project, &["branch", &branch, &commit])
            .map_err(failed)?;
        let path_str = path.to_string_lossy().into_owned();
        if let Err(err) = self.run(project, &["worktree", "add", &path_str, &branch]) {
            self.undo_branch(project, &branch);
            return Err(failed(err));
        }
        // `git rev-parse --git-path` resolves to the real `.git` dir even when `project/.git`
        // is a file pointing elsewhere (a submodule, or a Project that is itself a worktree).
        let exclude = match self.run(project, &["rev-parse", "--git-path", "info/exclude"]) {
            Ok(relative) => project.join(relative),
            Err(err) => {
                self.undo(project, &path, &branch);
                return Err(failed(err));
            }
        };
        if let Err(err) = exclude_worktrees(&exclude) {
            self.undo(project, &path, &branch);
            return Err(failed(err.to_string()));
        }
        Ok(Worktree { path, branch, base })
    }

    /// Undo a successful `provision`: remove the worktree directory and its branch, so
    /// `git worktree list` and `git branch` go back to what they were before it ran.
    pub fn discard(&self, project: &Path, worktree: &Worktree) {
        self.undo(project, &worktree.path, &worktree.branch);
    }

    fn undo(&self, project: &Path, path: &Path, branch: &str) {
        self.undo_worktree(project, path);
        self.undo_branch(project, branch);
    }

    fn undo_worktree(&self, project: &Path, path: &Path) {
        let path_str = path.to_string_lossy().into_owned();
        if let Err(err) = self.run(project, &["worktree", "remove", "--force", &path_str]) {
            eprintln!(
                "agents: could not remove worktree {}: {err}",
                path.display()
            );
            let _ = std::fs::remove_dir_all(path);
            if let Err(err) = self.run(project, &["worktree", "prune"]) {
                eprintln!("agents: could not prune worktrees: {err}");
            }
        }
    }

    fn undo_branch(&self, project: &Path, branch: &str) {
        if let Err(err) = self.run(project, &["branch", "-D", branch]) {
            eprintln!("agents: could not remove branch {branch}: {err}");
        }
    }
}

fn worktree_path(project: &Path, id: &str) -> PathBuf {
    project
        .join(".roundup")
        .join("worktrees")
        .join(format!("agent-{id}"))
}

fn invalid(name: &str, detail: impl std::fmt::Display) -> RpcError {
    RpcError::new(code::INVALID_PARAMS, format!("{name}: {detail}"))
}

fn failed(detail: impl std::fmt::Display) -> RpcError {
    RpcError::internal(format!("worktree_failed: {detail}"))
}

/// Add `.roundup/` to `exclude` (the repository's `info/exclude`), once: it is local, so the
/// user's tracked files and `git status` do not change.
fn exclude_worktrees(exclude: &Path) -> std::io::Result<()> {
    if let Some(parent) = exclude.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let existing = std::fs::read_to_string(exclude).unwrap_or_default();
    if existing.lines().any(|line| line.trim() == ".roundup/") {
        return Ok(());
    }
    let mut text = existing;
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(".roundup/\n");
    std::fs::write(exclude, text)
}

/// `cwd` mapped into `worktree_path`, the same subpath it was of `project`; `None` when `cwd` is
/// not under `project` at all (left for the usual cwd validation to reject).
pub fn map_cwd(project: &Path, cwd: &Path, worktree_path: &Path) -> Option<PathBuf> {
    let relative = cwd.strip_prefix(project).ok()?;
    Some(worktree_path.join(relative))
}
