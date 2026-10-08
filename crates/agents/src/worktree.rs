//! Git worktrees for Agents (G1/G2, `scenarios/worktrees.md`). The `git` command, run with
//! `std::process::Command` and no shell: `git worktree add` and `git branch` are the same
//! operations a user runs, and the user's own hooks and config apply (`docs/worktrees.md`).
//! `git2` is rejected there: a native dependency whose rebase reports conflicts differently.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use contracts::agent::WorktreeState;
use rpc::{RpcError, code};

/// What provisioning makes. `RailNode.worktree` (`contracts::agent::Worktree`) stores the same
/// three values as strings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: String,
    pub base: String,
}

#[derive(Clone, Debug)]
pub struct Provision {
    pub worktree: Worktree,
    pub commit: String,
    pub owner: String,
}

impl From<&contracts::agent::Worktree> for Worktree {
    fn from(record: &contracts::agent::Worktree) -> Self {
        Self {
            path: PathBuf::from(&record.path),
            branch: record.branch.clone(),
            base: record.base.clone(),
        }
    }
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

    pub fn plan(&self, project: &Path, id: &str, attempt: &str) -> Result<Provision, RpcError> {
        let commit = self
            .run(project, &["rev-parse", "HEAD"])
            .map_err(|_| invalid("not_a_git_project", project.display()))?;
        let base = self
            .run(project, &["symbolic-ref", "--short", "HEAD"])
            .map_err(|_| invalid("no_base", "HEAD is detached"))?;
        let worktree = Worktree {
            path: worktree_path(project, id),
            branch: format!("roundup/agent-{id}"),
            base,
        };
        if worktree.path.exists()
            || self
                .reference(project, &format!("refs/heads/{}", worktree.branch))?
                .is_some()
        {
            return Err(failed("provisioning destination already exists"));
        }
        Ok(Provision {
            worktree,
            commit,
            owner: format!("refs/roundup/provisioning/{id}/{attempt}"),
        })
    }

    pub fn provision(&self, project: &Path, plan: &Provision) -> Result<Worktree, RpcError> {
        let input = format!(
            "start\ncreate refs/heads/{} {}\ncreate {} {}\nprepare\ncommit\n",
            plan.worktree.branch, plan.commit, plan.owner, plan.commit
        );
        self.update_refs(project, &input)?;
        let result = (|| {
            self.run(
                project,
                &[
                    "worktree",
                    "add",
                    &plan.worktree.path.to_string_lossy(),
                    &plan.worktree.branch,
                ],
            )
            .map_err(failed)?;
            let exclude = self
                .run(project, &["rev-parse", "--git-path", "info/exclude"])
                .map_err(failed)?;
            exclude_worktrees(&project.join(exclude)).map_err(failed)?;
            Ok(plan.worktree.clone())
        })();
        if result.is_err() {
            self.recover(project, plan)?;
        }
        result
    }

    fn reference(&self, project: &Path, reference: &str) -> Result<Option<String>, RpcError> {
        let output = {
            let _hold = self.lock.lock().expect("git lock");
            Command::new("git")
                .current_dir(project)
                .env("PATH", &self.path)
                .args(["show-ref", "--verify", "--hash", reference])
                .output()
                .map_err(failed)?
        };
        if output.status.success() {
            return Ok(Some(String::from_utf8_lossy(&output.stdout).trim().into()));
        }
        if output.status.code() == Some(1) {
            return Ok(None);
        }
        // show-ref reports an absent exact ref as 128 on older Git versions.
        if !self
            .run(project, &["for-each-ref", "--format=%(refname)", reference])
            .map_err(failed)?
            .lines()
            .any(|line| line == reference)
        {
            return Ok(None);
        }
        Err(failed(String::from_utf8_lossy(&output.stderr)))
    }

    fn update_refs(&self, project: &Path, input: &str) -> Result<(), RpcError> {
        let _hold = self.lock.lock().expect("git lock");
        let mut child = Command::new("git")
            .current_dir(project)
            .env("PATH", &self.path)
            .args(["update-ref", "--stdin"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(failed)?;
        child
            .stdin
            .take()
            .expect("piped stdin")
            .write_all(input.as_bytes())
            .map_err(failed)?;
        let output = child.wait_with_output().map_err(failed)?;
        if !output.status.success() {
            return Err(failed(String::from_utf8_lossy(&output.stderr)));
        }
        Ok(())
    }

    pub fn finish(&self, project: &Path, plan: &Provision) -> Result<(), RpcError> {
        if let Some(owner) = self.reference(project, &plan.owner)? {
            if owner != plan.commit {
                return Err(failed("provisioning ownership changed"));
            }
            self.run(project, &["update-ref", "-d", &plan.owner, &plan.commit])
                .map_err(failed)?;
        }
        Ok(())
    }

    pub fn recover(&self, project: &Path, plan: &Provision) -> Result<(), RpcError> {
        let branch_ref = format!("refs/heads/{}", plan.worktree.branch);
        let branch = self.reference(project, &branch_ref)?;
        let owner = self.reference(project, &plan.owner)?;
        if owner.is_none() {
            if branch.is_some() || plan.worktree.path.exists() {
                return Err(failed("unowned provisioning destination; preserving it"));
            }
            return Ok(());
        }
        if owner.as_deref() != Some(&plan.commit)
            || branch.as_ref().is_some_and(|head| head != &plan.commit)
        {
            return Err(failed(
                "provisioning ownership or branch changed; preserving it",
            ));
        }
        let expected_path = canonical_path(&plan.worktree.path);
        let mut registered = false;
        for (path, reference) in self.registered_worktrees(project)? {
            if path == expected_path {
                if reference.as_deref() != Some(&branch_ref) {
                    return Err(failed("provisioning path belongs to another branch"));
                }
                registered = true;
            } else if reference.as_deref() == Some(&branch_ref) {
                return Err(failed("provisioning branch belongs to another path"));
            }
        }
        if plan.worktree.path.exists() {
            if !registered {
                return Err(failed("provisioning path is not its registered Worktree"));
            }
            if !self
                .run(&plan.worktree.path, &["status", "--porcelain"])
                .map_err(failed)?
                .is_empty()
            {
                return Err(failed("unfinished Worktree has edits; preserving it"));
            }
        }
        if registered {
            self.run(
                project,
                &["worktree", "remove", &plan.worktree.path.to_string_lossy()],
            )
            .map_err(failed)?;
        }
        if branch.is_some() {
            self.run(project, &["update-ref", "-d", &branch_ref, &plan.commit])
                .map_err(failed)?;
        }
        self.finish(project, plan)
    }

    /// A22: the saved Worktree still names a real directory on its saved branch, checked against
    /// Git itself rather than trusted from the record. The directory's cleanliness is not this
    /// check's concern (resume never cares whether it is landed).
    pub fn verify_resumable(&self, project: &Path, worktree: &Worktree) -> Result<(), RpcError> {
        if !worktree.path.is_dir() {
            return Err(RpcError::conflict(format!(
                "worktree_missing: {}",
                worktree.path.display()
            )));
        }
        let branch_ref = format!("refs/heads/{}", worktree.branch);
        if self.reference(project, &branch_ref)?.is_none() {
            return Err(RpcError::conflict(format!(
                "worktree_branch_missing: {}",
                worktree.branch
            )));
        }
        let expected_path = canonical_path(&worktree.path);
        match self
            .registered_worktrees(project)?
            .into_iter()
            .find(|(path, _)| *path == expected_path)
        {
            Some((_, Some(reference))) if reference == branch_ref => Ok(()),
            Some((_, Some(_))) => Err(RpcError::conflict(
                "worktree_mismatched: registered to another branch",
            )),
            Some((_, None)) => Err(RpcError::conflict(
                "worktree_detached: no branch checked out",
            )),
            None => Err(RpcError::conflict("worktree_unregistered")),
        }
    }

    /// G3: `ahead` and `behind` between the branch and its Base, and whether the Worktree has
    /// changes outside `.roundup/`. Runs no command that writes.
    pub fn state(&self, project: &Path, worktree: &Worktree) -> Result<WorktreeState, RpcError> {
        if !worktree.path.is_dir() {
            return Err(RpcError::new(
                code::NOT_FOUND,
                format!("worktree_missing: {}", worktree.path.display()),
            ));
        }
        let range = format!("{}...refs/heads/{}", worktree.base, worktree.branch);
        let counts = self
            .run(project, &["rev-list", "--left-right", "--count", &range])
            .map_err(failed)?;
        let mut counts = counts.split_whitespace().map(|n| n.parse::<u32>());
        let (Some(Ok(behind)), Some(Ok(ahead))) = (counts.next(), counts.next()) else {
            return Err(failed("unreadable commit counts"));
        };
        Ok(WorktreeState {
            ahead,
            behind,
            dirty: self.dirty(worktree)?,
        })
    }

    /// Whether the Worktree has a staged, unstaged or untracked change outside `.roundup/`.
    fn dirty(&self, worktree: &Worktree) -> Result<bool, RpcError> {
        let status = self
            .run(
                &worktree.path,
                &[
                    "--no-optional-locks",
                    "status",
                    "--porcelain",
                    "-z",
                    "--untracked-files=all",
                ],
            )
            .map_err(failed)?;
        Ok(status
            .split('\0')
            .filter(|entry| !entry.is_empty())
            .any(|entry| !entry.get(3..).is_some_and(|p| p.starts_with(".roundup/"))))
    }

    pub fn require_landed(&self, project: &Path, worktree: &Worktree) -> Result<(), RpcError> {
        let dirty = if worktree.path.exists() {
            self.run(
                &worktree.path,
                &["status", "--porcelain", "-z", "--untracked-files=all"],
            )
            .map_err(failed)?
            .split('\0')
            .filter(|entry| !entry.is_empty())
            .count()
        } else {
            0
        };
        let ahead = match self.reference(project, &format!("refs/heads/{}", worktree.branch))? {
            Some(head) => self
                .run(
                    project,
                    &["rev-list", "--count", &format!("{}..{head}", worktree.base)],
                )
                .map_err(failed)?,
            None if !worktree.path.exists() => "0".into(),
            None => return Err(failed("recorded Worktree branch is missing")),
        };
        if dirty != 0 || ahead != "0" {
            return Err(RpcError::conflict(format!(
                "worktree_unlanded: dirty {dirty}, ahead {ahead}"
            )));
        }
        Ok(())
    }

    /// G4: rebase the branch onto the Base, run `check` in the Worktree, fast-forward the Base.
    /// Every failure leaves the branch, the Worktree and the Base as they were. The checks run in
    /// the order the scenario lists, so one code wins when several hold.
    pub fn land(
        &self,
        project: &Path,
        worktree: &Worktree,
        check: Option<&str>,
    ) -> Result<String, RpcError> {
        if !worktree.path.is_dir() {
            return Err(RpcError::new(
                code::NOT_FOUND,
                format!("worktree_missing: {}", worktree.path.display()),
            ));
        }
        let Some(check) = check else {
            return Err(RpcError::conflict(
                "check_missing: the Project has no check; set one with project.setWorktrees",
            ));
        };
        if self.dirty(worktree)? {
            return Err(RpcError::conflict(
                "worktree_dirty: the Worktree has changes",
            ));
        }
        let base_ref = format!("refs/heads/{}", worktree.base);
        let moved =
            || RpcError::conflict(format!("base_moved: {} is not checked out", worktree.base));
        let checked_out = self.run(project, &["symbolic-ref", "HEAD"]).ok();
        if checked_out.as_deref() != Some(&base_ref)
            || self.reference(project, &base_ref)?.is_none()
        {
            return Err(moved());
        }
        let before = self
            .run(&worktree.path, &["rev-parse", "HEAD"])
            .map_err(failed)?;
        let undo =
            |err: RpcError| match self.run(&worktree.path, &["reset", "--hard", "-q", &before]) {
                Ok(_) => {
                    let _ = self.run(&worktree.path, &["clean", "-fdq"]);
                    err
                }
                Err(reset) => failed(format!("{err}; could not restore the branch: {reset}")),
            };

        if let Err(err) = self.run(&worktree.path, &["rebase", &base_ref]) {
            let paths = self
                .run(&worktree.path, &["diff", "--name-only", "--diff-filter=U"])
                .unwrap_or_default();
            self.run(&worktree.path, &["rebase", "--abort"])
                .map_err(|abort| failed(format!("could not abort the rebase: {abort}")))?;
            return Err(if paths.is_empty() {
                failed(err)
            } else {
                RpcError::conflict(format!(
                    "landing_conflict: {}",
                    paths.lines().collect::<Vec<_>>().join(", ")
                ))
            });
        }

        let output = Command::new("sh")
            .current_dir(&worktree.path)
            .args(["-c", "exec 2>&1; eval \"$1\"", "sh", check])
            .stdin(Stdio::null())
            .output()
            .map_err(|err| undo(failed(err)))?;
        if !output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            let lines: Vec<&str> = text.lines().collect();
            let tail = lines[lines.len().saturating_sub(20)..].join("\n");
            return Err(undo(RpcError::conflict(format!("check_failed: {tail}"))));
        }
        if self.dirty(worktree).map_err(undo)? {
            return Err(undo(RpcError::conflict("check_failed: check left changes")));
        }

        let touched = self
            .run(
                project,
                &["diff", "--name-only", "-z", &base_ref, &worktree.branch],
            )
            .map_err(|err| undo(failed(err)))?;
        let touched: Vec<&str> = touched.split('\0').filter(|p| !p.is_empty()).collect();
        let status = self
            .run(
                project,
                &[
                    "--no-optional-locks",
                    "status",
                    "--porcelain",
                    "-z",
                    "--untracked-files=all",
                ],
            )
            .map_err(|err| undo(failed(err)))?;
        let mut entries = status.split('\0').filter(|e| !e.is_empty());
        while let Some(entry) = entries.next() {
            let path = entry.get(3..).unwrap_or_default();
            if matches!(entry.get(..1), Some("R" | "C")) {
                entries.next();
            }
            if touched.contains(&path) {
                return Err(undo(RpcError::conflict(format!(
                    "base_dirty: {path} has changes in the Project folder"
                ))));
            }
        }
        if self
            .run(project, &["merge", "--ff-only", "-q", &worktree.branch])
            .is_err()
        {
            return Err(undo(moved()));
        }
        self.run(project, &["rev-parse", &base_ref]).map_err(failed)
    }

    /// G5: delete the Worktree's directory and branch whatever their state; a directory already
    /// gone is not an error, and only git's record of it is removed.
    pub fn discard(&self, project: &Path, worktree: &Worktree) -> Result<(), RpcError> {
        let reference = format!("refs/heads/{}", worktree.branch);
        let path = canonical_path(&worktree.path);
        let registered = self
            .registered_worktrees(project)?
            .into_iter()
            .find(|(entry, _)| *entry == path);
        match registered {
            Some((_, branch)) if branch.is_none() || branch.as_deref() == Some(&reference) => {
                self.run(
                    project,
                    &[
                        "worktree",
                        "remove",
                        "--force",
                        &worktree.path.to_string_lossy(),
                    ],
                )
                .map_err(failed)?;
            }
            Some(_) => return Err(failed("recorded Worktree path belongs to another branch")),
            None if worktree.path.exists() => {
                return Err(failed("recorded Worktree path is unregistered"));
            }
            None => {}
        }
        if let Some(head) = self.reference(project, &reference)? {
            self.run(project, &["update-ref", "-d", &reference, &head])
                .map_err(failed)?;
        }
        Ok(())
    }

    fn registered_worktrees(
        &self,
        project: &Path,
    ) -> Result<Vec<(PathBuf, Option<String>)>, RpcError> {
        let listing = self
            .run(project, &["worktree", "list", "--porcelain", "-z"])
            .map_err(failed)?;
        Ok(listing
            .split("\0\0")
            .filter_map(|record| {
                let path = record
                    .split('\0')
                    .find_map(|value| value.strip_prefix("worktree "))?;
                let branch = record
                    .split('\0')
                    .find_map(|value| value.strip_prefix("branch "))
                    .map(str::to_owned);
                Some((canonical_path(Path::new(path)), branch))
            })
            .collect())
    }

    pub fn remove_landed(&self, project: &Path, worktree: &Worktree) -> Result<(), RpcError> {
        let reference = format!("refs/heads/{}", worktree.branch);
        let expected = self.reference(project, &reference)?;
        self.require_landed(project, worktree)?;
        let path = canonical_path(&worktree.path);
        let registered = self
            .registered_worktrees(project)?
            .into_iter()
            .find(|(entry, _)| *entry == path);
        match registered {
            Some((_, branch)) if expected.is_some() && branch.as_deref() == Some(&reference) => {
                self.run(
                    project,
                    &["worktree", "remove", &worktree.path.to_string_lossy()],
                )
                .map_err(failed)?;
            }
            Some(_) => return Err(failed("recorded Worktree path belongs to another branch")),
            None if worktree.path.exists() => {
                return Err(failed("recorded Worktree path is unregistered"));
            }
            None => {}
        }
        if let Some(expected) = expected {
            self.run(project, &["update-ref", "-d", &reference, &expected])
                .map_err(failed)?;
        }
        Ok(())
    }
}

/// `path` as git lists it, even when the directory is gone: the nearest existing ancestor is
/// resolved (a symlinked temp directory) and the missing rest is appended.
fn canonical_path(path: &Path) -> PathBuf {
    if let Ok(resolved) = path.canonicalize() {
        return resolved;
    }
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => canonical_path(parent).join(name),
        _ => path.to_owned(),
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
