//! G1 the `worktrees` setting, and G2 spawn provisions a Worktree (`scenarios/worktrees.md`).

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use agents::worktree::Git;
use contracts::agent::{NodeKind, RailNode};
use contracts::project::{ProjectSettings, Worktrees};
use rpc::code;
use serde_json::{Value, json};

use crate::common::Fixture;

async fn set_worktrees(f: &Fixture, on: bool, check: Option<&str>) {
    f.call("project.setWorktrees", json!({"on": on, "check": check}))
        .await
        .unwrap();
}

async fn project_get(f: &Fixture) -> ProjectSettings {
    let value = f.call("project.get", Value::Null).await.unwrap();
    serde_json::from_value(value).unwrap()
}

async fn spawn_in(
    f: &Fixture,
    cwd: &Path,
    parent: Option<&str>,
) -> Result<RailNode, rpc::RpcError> {
    let params = json!({"cwd": cwd.to_string_lossy(), "prompt": Value::Null, "parent": parent});
    let node = f.call("agent.spawn", params).await?;
    Ok(serde_json::from_value(node).unwrap())
}

async fn promote(f: &Fixture, id: &str) -> Result<RailNode, rpc::RpcError> {
    let node = f.call("rail.promote", json!({"id": id})).await?;
    Ok(serde_json::from_value(node).unwrap())
}

fn git_output(project: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .current_dir(project)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// `git branch` and `git worktree list`, to compare before and after a failed call (G2, P5).
fn git_state(project: &Path) -> (String, String) {
    (
        git_output(project, &["branch", "--list"]),
        git_output(project, &["worktree", "list"]),
    )
}

fn real_git() -> String {
    String::from_utf8(
        std::process::Command::new("sh")
            .arg("-c")
            .arg("command -v git")
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_owned()
}

/// A `git` named `git` in `dir`, running `script` with the real one's path as `$REAL`, put in
/// front of the real `PATH`.
fn wrapped_git(dir: &Path, script: &str) -> Git {
    let wrapper = dir.join("git");
    std::fs::write(
        &wrapper,
        format!("#!/bin/sh\nREAL='{}'\n{script}\n", real_git()),
    )
    .unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    Git::with_path(format!(
        "{}:{}",
        dir.display(),
        std::env::var("PATH").unwrap()
    ))
}

/// `git` on `PATH` that fails `git worktree add` and passes every other command to the real one.
fn failing_git(dir: &Path) -> Git {
    wrapped_git(
        dir,
        "if [ \"$1\" = worktree ] && [ \"$2\" = add ]; then echo fake failure >&2; exit 1; fi\nexec \"$REAL\" \"$@\"",
    )
}

/// `git` on `PATH` that fails any command that starts while another is still running, as two
/// concurrent `git` commands on one repository can (`index.lock`).
fn exclusive_git(dir: &Path) -> Git {
    let busy = dir.join("busy");
    wrapped_git(
        dir,
        &format!(
            "mkdir '{busy}' 2>/dev/null || {{ echo concurrent git >&2; exit 1; }}\nsleep 0.05\n\"$REAL\" \"$@\"\ncode=$?\nrmdir '{busy}'\nexit $code",
            busy = busy.display()
        ),
    )
}

#[tokio::test]
async fn g1_setting_defaults_off_and_survives_restart() {
    let f = Fixture::new();
    assert_eq!(
        project_get(&f).await.worktrees,
        Worktrees {
            on: false,
            check: None
        }
    );

    set_worktrees(&f, true, Some("just check")).await;

    let f = f.reopen();
    assert_eq!(
        project_get(&f).await.worktrees,
        Worktrees {
            on: true,
            check: Some("just check".into())
        }
    );
}

#[tokio::test]
async fn g1_empty_check_is_invalid_params() {
    let f = Fixture::new();

    let err = f
        .call("project.setWorktrees", json!({"on": true, "check": ""}))
        .await
        .unwrap_err();

    assert_eq!(err.code, code::INVALID_PARAMS);
    assert_eq!(
        project_get(&f).await.worktrees,
        Worktrees {
            on: false,
            check: None
        }
    );
}

#[tokio::test]
async fn g1_off_leaves_spawn_as_a4_says() {
    let f = Fixture::running("sleep 30");

    let node = f.spawn(None, None).await.unwrap();

    assert_eq!(node.worktree, None);
    assert_eq!(f.tree().await[0].worktree, None);
}

#[tokio::test]
async fn g2_spawn_makes_branch_and_worktree() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let commit = git_output(f.dir.path(), &["rev-parse", "HEAD"])
        .trim()
        .to_owned();

    let node = f.spawn(None, None).await.unwrap();

    let worktree = node.worktree.clone().expect("a Worktree");
    assert_eq!(worktree.branch, format!("roundup/agent-{}", node.id));
    assert_eq!(worktree.base, "main");
    assert_eq!(
        git_output(f.dir.path(), &["rev-parse", &worktree.branch])
            .trim()
            .to_owned(),
        commit,
        "the branch starts at the Project's current commit"
    );
    let path = PathBuf::from(&worktree.path);
    assert_eq!(
        path,
        f.dir
            .path()
            .join(".roundup/worktrees")
            .join(format!("agent-{}", node.id))
    );
    assert!(path.is_dir());
    assert!(
        git_output(f.dir.path(), &["branch", "--list", &worktree.branch])
            .contains(&worktree.branch)
    );
    assert!(
        git_output(f.dir.path(), &["worktree", "list"]).contains(path.to_string_lossy().as_ref())
    );
    assert_eq!(f.tree().await[0].worktree, Some(worktree));
}

#[tokio::test]
async fn g2_base_is_the_branch_checked_out_not_always_main() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    git_output(f.dir.path(), &["checkout", "-q", "-b", "feature"]);
    set_worktrees(&f, true, None).await;

    let node = f.spawn(None, None).await.unwrap();

    assert_eq!(node.worktree.unwrap().base, "feature");
}

#[tokio::test]
async fn g2_exclude_worktrees_is_added_once() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let exclude = f.dir.path().join(".git/info/exclude");
    let count = |text: &str| {
        text.lines()
            .filter(|line| line.trim() == ".roundup/")
            .count()
    };

    f.spawn(None, None).await.unwrap();
    assert_eq!(count(&std::fs::read_to_string(&exclude).unwrap()), 1);

    f.spawn(None, None).await.unwrap();
    assert_eq!(count(&std::fs::read_to_string(&exclude).unwrap()), 1);
}

#[tokio::test]
async fn g2_a_project_whose_git_is_a_file_gets_the_exclude_line_in_the_real_git_dir() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let elsewhere = tempfile::tempdir().unwrap();
    let git_dir = elsewhere.path().join("gitdir");
    std::fs::rename(f.dir.path().join(".git"), &git_dir).unwrap();
    std::fs::write(
        f.dir.path().join(".git"),
        format!("gitdir: {}\n", git_dir.display()),
    )
    .unwrap();

    f.spawn(None, None).await.unwrap();

    let exclude = std::fs::read_to_string(git_dir.join("info/exclude")).unwrap();
    assert!(exclude.lines().any(|line| line.trim() == ".roundup/"));
}

#[tokio::test]
async fn g2_cwd_outside_project_is_invalid_params() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let outside = tempfile::tempdir().unwrap();
    let before = git_state(f.dir.path());

    let err = spawn_in(&f, outside.path(), None).await.unwrap_err();

    assert_eq!(err.code, code::INVALID_PARAMS);
    assert!(
        err.message.contains("outside the project folder"),
        "{}",
        err.message
    );
    assert!(f.tree().await.is_empty());
    assert_eq!(git_state(f.dir.path()), before);
}

#[tokio::test]
async fn g2_start_failure_after_provisioning_leaves_no_branch_no_directory() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let before = git_state(f.dir.path());
    std::fs::remove_file(f.dir.path().join(".roundup/rup")).unwrap();

    let err = f.spawn(None, None).await.unwrap_err();

    assert_eq!(err.code, code::INTERNAL);
    assert!(f.tree().await.is_empty());
    assert_eq!(git_state(f.dir.path()), before);
}

#[tokio::test]
async fn g2_exclude_failure_undoes_branch_and_worktree() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let exclude = f.dir.path().join(".git/info/exclude");
    // Read-only, not missing or a directory: `git worktree add` itself reads `info/exclude`
    // (and fails if it can't be read as a file), so only a write failure isolates the step
    // this test targets.
    std::fs::set_permissions(&exclude, std::fs::Permissions::from_mode(0o444)).unwrap();
    let before = git_state(f.dir.path());

    let err = f.spawn(None, None).await.unwrap_err();

    assert_eq!(err.code, code::INTERNAL);
    assert!(
        err.message.starts_with("worktree_failed: "),
        "{}",
        err.message
    );
    assert!(f.tree().await.is_empty());
    assert_eq!(git_state(f.dir.path()), before);
}

#[tokio::test]
async fn g2_two_spawns_have_different_cwds() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let cwd_of = |f: &Fixture, node: &RailNode| {
        f.terminals
            .list()
            .into_iter()
            .find(|t| Some(&t.id) == node.terminal_id.as_ref())
            .unwrap()
            .cwd
    };

    let a = f.spawn(None, None).await.unwrap();
    let b = f.spawn(None, None).await.unwrap();

    assert_ne!(cwd_of(&f, &a), cwd_of(&f, &b));
    assert_ne!(a.worktree.unwrap().path, b.worktree.unwrap().path);
}

#[tokio::test]
async fn g2_two_promotes_have_different_cwds() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let g1 = f.group("g1", None).await;
    let g2 = f.group("g2", None).await;
    let cwd_of = |f: &Fixture, node: &RailNode| {
        f.terminals
            .list()
            .into_iter()
            .find(|t| Some(&t.id) == node.terminal_id.as_ref())
            .unwrap()
            .cwd
    };

    let a = promote(&f, &g1).await.unwrap();
    let b = promote(&f, &g2).await.unwrap();

    assert_ne!(cwd_of(&f, &a), cwd_of(&f, &b));
    assert_ne!(a.worktree.unwrap().path, b.worktree.unwrap().path);
}

#[tokio::test]
async fn g2_subfolder_cwd_maps_into_the_worktree() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let sub = f.dir.path().join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("keep.txt"), "x").unwrap();
    git_output(f.dir.path(), &["add", "sub/keep.txt"]);
    git_output(f.dir.path(), &["commit", "-q", "-m", "add sub"]);

    let node = spawn_in(&f, &sub, None).await.unwrap();

    let worktree_path = PathBuf::from(&node.worktree.unwrap().path);
    let cwd = f
        .terminals
        .list()
        .into_iter()
        .find(|t| Some(&t.id) == node.terminal_id.as_ref())
        .unwrap()
        .cwd;
    assert_eq!(cwd, worktree_path.join("sub").to_string_lossy());
    assert!(Path::new(&cwd).is_dir());
}

#[tokio::test]
async fn g2_failure_leaves_no_branch_no_directory_no_node() {
    let wrapper = tempfile::tempdir().unwrap();
    let f = Fixture::in_git_project("sleep 30", failing_git(wrapper.path()));
    set_worktrees(&f, true, None).await;
    let before = git_state(f.dir.path());

    let err = f.spawn(None, None).await.unwrap_err();

    assert_eq!(err.code, code::INTERNAL);
    // The wrapper's `git worktree add` prints exactly "fake failure" on stderr (`failing_git`);
    // an equality check, not just the prefix, catches a message that drops git's own text.
    assert_eq!(err.message, "worktree_failed: fake failure");
    assert!(f.tree().await.is_empty());
    assert_eq!(git_state(f.dir.path()), before);
    assert!(!f.dir.path().join(".roundup/worktrees").exists());
}

#[tokio::test]
async fn g2_failed_call_emits_no_event() {
    let wrapper = tempfile::tempdir().unwrap();
    let mut f = Fixture::in_git_project("sleep 30", failing_git(wrapper.path()));
    set_worktrees(&f, true, None).await;

    f.spawn(None, None).await.unwrap_err();

    assert_eq!(f.changed(), 0);
}

#[tokio::test]
async fn g2_failed_promote_leaves_a_plain_group() {
    let wrapper = tempfile::tempdir().unwrap();
    let f = Fixture::in_git_project("sleep 30", failing_git(wrapper.path()));
    set_worktrees(&f, true, None).await;
    let group = f.group("team", None).await;
    let before = git_state(f.dir.path());

    let err = promote(&f, &group).await.unwrap_err();

    assert_eq!(err.code, code::INTERNAL);
    let tree = f.tree().await;
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].kind, NodeKind::Group);
    assert!(!tree[0].meta);
    assert_eq!(tree[0].worktree, None);
    assert_eq!(git_state(f.dir.path()), before);
}

#[tokio::test]
async fn g2_failed_promote_leaves_children_where_they_were() {
    let wrapper = tempfile::tempdir().unwrap();
    let f = Fixture::in_git_project("sleep 30", failing_git(wrapper.path()));
    set_worktrees(&f, true, None).await;
    let group = f.group("team", None).await;
    let child = f.group("child", Some(&group)).await;
    let before = git_state(f.dir.path());

    let err = promote(&f, &group).await.unwrap_err();

    assert_eq!(err.code, code::INTERNAL);
    let tree = f.tree().await;
    let group_node = tree.iter().find(|node| node.id == group).unwrap();
    assert_eq!(group_node.kind, NodeKind::Group);
    assert!(!group_node.meta);
    let child_node = tree.iter().find(|node| node.id == child).unwrap();
    assert_eq!(child_node.parent.as_deref(), Some(group.as_str()));
    assert_eq!(git_state(f.dir.path()), before);
}

#[tokio::test]
async fn g2_not_a_git_project_no_base_and_cwd_not_in_worktree() {
    // not_a_git_project: no `.git` at all.
    {
        let f = Fixture::in_git_project("sleep 30", Git::from_env());
        set_worktrees(&f, true, None).await;
        std::fs::remove_dir_all(f.dir.path().join(".git")).unwrap();

        let err = f.spawn(None, None).await.unwrap_err();

        assert_eq!(err.code, code::INVALID_PARAMS);
        assert!(
            err.message.starts_with("not_a_git_project: "),
            "{}",
            err.message
        );
        assert!(f.tree().await.is_empty());
    }

    // not_a_git_project: a git repository with no commit.
    {
        let f = Fixture::in_git_project("sleep 30", Git::from_env());
        set_worktrees(&f, true, None).await;
        std::fs::remove_dir_all(f.dir.path().join(".git")).unwrap();
        git_output(f.dir.path(), &["init", "-q", "-b", "main"]);

        let err = f.spawn(None, None).await.unwrap_err();

        assert_eq!(err.code, code::INVALID_PARAMS);
        assert!(
            err.message.starts_with("not_a_git_project: "),
            "{}",
            err.message
        );
        assert!(f.tree().await.is_empty());
    }

    // no_base: HEAD is detached.
    {
        let f = Fixture::in_git_project("sleep 30", Git::from_env());
        set_worktrees(&f, true, None).await;
        let commit = git_output(f.dir.path(), &["rev-parse", "HEAD"])
            .trim()
            .to_owned();
        git_output(f.dir.path(), &["checkout", "-q", "--detach", &commit]);

        let err = f.spawn(None, None).await.unwrap_err();

        assert_eq!(err.code, code::INVALID_PARAMS);
        assert!(err.message.starts_with("no_base: "), "{}", err.message);
        assert!(f.tree().await.is_empty());
    }

    // cwd_not_in_worktree: an untracked subfolder, missing from the fresh worktree.
    {
        let f = Fixture::in_git_project("sleep 30", Git::from_env());
        set_worktrees(&f, true, None).await;
        let sub = f.dir.path().join("untracked");
        std::fs::create_dir_all(&sub).unwrap();
        let before = git_state(f.dir.path());

        let err = spawn_in(&f, &sub, None).await.unwrap_err();

        assert_eq!(err.code, code::INVALID_PARAMS);
        assert!(
            err.message.starts_with("cwd_not_in_worktree: "),
            "{}",
            err.message
        );
        assert!(f.tree().await.is_empty());
        assert_eq!(git_state(f.dir.path()), before);
    }
}

#[tokio::test]
async fn g2_terminal_gets_no_worktree() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let before = git_state(f.dir.path());

    let node = f
        .call(
            "rail.spawnTerminal",
            json!({"cwd": f.dir.path().to_string_lossy(), "parent": null}),
        )
        .await
        .unwrap();
    let node: RailNode = serde_json::from_value(node).unwrap();

    assert_eq!(node.worktree, None);
    assert_eq!(git_state(f.dir.path()), before);
}

#[tokio::test]
async fn g2_start_failure_of_promote_leaves_a_plain_group() {
    let mut f = Fixture::in_git_project("sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let group = f.group("team", None).await;
    let before = git_state(f.dir.path());
    f.changed();
    std::fs::remove_file(f.dir.path().join(".roundup/rup")).unwrap();

    let err = promote(&f, &group).await.unwrap_err();

    assert_eq!(err.code, code::INTERNAL);
    let tree = f.tree().await;
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].kind, NodeKind::Group);
    assert!(!tree[0].meta);
    assert_eq!(tree[0].worktree, None);
    assert_eq!(git_state(f.dir.path()), before);
    assert_eq!(f.changed(), 0);
}

#[tokio::test]
async fn g2_a_record_failure_after_provisioning_leaves_no_branch_no_directory() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let before = git_state(f.dir.path());
    // The Rail's own write of the finished Worktree is the one step this trigger refuses.
    rusqlite::Connection::open(f.dir.path().join(".roundup/agents.db"))
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER refuse_ready BEFORE UPDATE OF worktree_state ON nodes
             WHEN NEW.worktree_state = 'ready' BEGIN SELECT RAISE(ABORT, 'refused'); END;",
        )
        .unwrap();

    let err = f.spawn(None, None).await.unwrap_err();

    assert_eq!(err.code, code::INTERNAL);
    assert!(f.tree().await.is_empty());
    assert_eq!(git_state(f.dir.path()), before);
}

#[tokio::test]
async fn g2_concurrent_provisions_never_run_two_git_commands_at_once() {
    let wrapper = tempfile::tempdir().unwrap();
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    let git = exclusive_git(wrapper.path());

    let made: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|n| {
                let (git, project) = (&git, f.dir.path());
                scope.spawn(move || git.provision(project, &format!("agent-{n}")))
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let mut paths: Vec<_> = made.into_iter().map(|w| w.unwrap().path).collect();
    paths.sort();
    paths.dedup();
    assert_eq!(paths.len(), 4);
}

#[tokio::test]
async fn g2_spawn_at_the_project_root_runs_in_the_worktree_root() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;

    let node = f.spawn(None, None).await.unwrap();

    let cwd = f
        .terminals
        .list()
        .into_iter()
        .find(|t| Some(&t.id) == node.terminal_id.as_ref())
        .unwrap()
        .cwd;
    assert_eq!(
        std::fs::canonicalize(cwd).unwrap(),
        std::fs::canonicalize(node.worktree.unwrap().path).unwrap()
    );
}

fn failing_commands_git(dir: &Path, conditions: &str) -> Git {
    wrapped_git(
        dir,
        &format!("if {conditions}; then echo fake failure >&2; exit 1; fi\nexec \"$REAL\" \"$@\""),
    )
}

/// `git rev-parse --git-path`, the step after `git worktree add` that locates `info/exclude`.
const GIT_PATH_FAILS: &str = "[ \"$1\" = rev-parse ] && [ \"$2\" = --git-path ]";

async fn terminal_cwd(f: &Fixture, node: &RailNode) -> String {
    f.terminals
        .list()
        .into_iter()
        .find(|t| Some(&t.id) == node.terminal_id.as_ref())
        .unwrap()
        .cwd
}

#[tokio::test]
async fn g1_setting_twice_stores_the_later_value() {
    let f = Fixture::new();
    set_worktrees(&f, true, Some("just check")).await;

    set_worktrees(&f, false, None).await;

    assert_eq!(
        project_get(&f).await.worktrees,
        Worktrees {
            on: false,
            check: None
        }
    );
}

#[tokio::test]
async fn g2_an_exclude_file_with_no_final_newline_keeps_its_last_line() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let exclude = f.dir.path().join(".git/info/exclude");
    std::fs::write(&exclude, "target").unwrap();

    f.spawn(None, None).await.unwrap();

    assert_eq!(
        std::fs::read_to_string(exclude).unwrap(),
        "target\n.roundup/\n"
    );
}

#[tokio::test]
async fn g2_a_repository_with_no_info_directory_still_gets_the_exclude_line() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    std::fs::remove_dir_all(f.dir.path().join(".git/info")).unwrap();

    f.spawn(None, None).await.unwrap();

    let exclude = std::fs::read_to_string(f.dir.path().join(".git/info/exclude")).unwrap();
    assert_eq!(exclude, ".roundup/\n");
}

#[tokio::test]
async fn g2_a_padded_exclude_entry_counts_as_present() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let exclude = f.dir.path().join(".git/info/exclude");
    std::fs::write(&exclude, "  .roundup/ \r\n").unwrap();

    f.spawn(None, None).await.unwrap();

    assert_eq!(
        std::fs::read_to_string(exclude).unwrap(),
        "  .roundup/ \r\n"
    );
}

#[tokio::test]
async fn g2_a_failure_locating_the_exclude_file_leaves_nothing() {
    let wrapper = tempfile::tempdir().unwrap();
    let f = Fixture::in_git_project(
        "sleep 30",
        failing_commands_git(wrapper.path(), GIT_PATH_FAILS),
    );
    set_worktrees(&f, true, None).await;
    let before = git_state(f.dir.path());

    let err = f.spawn(None, None).await.unwrap_err();

    assert_eq!(err.code, code::INTERNAL);
    assert_eq!(git_state(f.dir.path()), before);
    assert!(!f.dir.path().join(".roundup/worktrees/agent-1").exists());
}

#[tokio::test]
async fn g2_when_git_cannot_remove_a_worktree_its_directory_and_registration_still_go() {
    let wrapper = tempfile::tempdir().unwrap();
    let removal_fails = "[ \"$1\" = worktree ] && [ \"$2\" = remove ]";
    let f = Fixture::in_git_project(
        "sleep 30",
        failing_commands_git(
            wrapper.path(),
            &format!("{{ {GIT_PATH_FAILS}; }} || {{ {removal_fails}; }}"),
        ),
    );
    set_worktrees(&f, true, None).await;
    let before = git_state(f.dir.path());

    f.spawn(None, None).await.unwrap_err();

    assert_eq!(git_state(f.dir.path()), before);
    assert!(!f.dir.path().join(".roundup/worktrees/agent-1").exists());
}

#[tokio::test]
async fn g2_a_dotdot_cwd_is_judged_by_where_it_leads() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let sub = f.dir.path().join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("keep.txt"), "x").unwrap();
    git_output(f.dir.path(), &["add", "sub/keep.txt"]);
    git_output(f.dir.path(), &["commit", "-q", "-m", "add sub"]);
    let before = git_state(f.dir.path());

    let out = spawn_in(&f, &sub.join("../.."), None).await.unwrap_err();
    let back = spawn_in(&f, &sub.join(".."), None).await.unwrap();

    assert_eq!(out.code, code::INVALID_PARAMS);
    assert!(
        out.message.contains("outside the project folder"),
        "{}",
        out.message
    );
    assert_eq!(
        git_state(f.dir.path()).0.lines().count(),
        before.0.lines().count() + 1
    );
    let worktree = std::fs::canonicalize(back.worktree.clone().unwrap().path).unwrap();
    assert_eq!(
        std::fs::canonicalize(terminal_cwd(&f, &back).await).unwrap(),
        worktree
    );
}

#[tokio::test]
async fn g2_a_symlink_to_the_project_is_a_cwd_inside_it() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let elsewhere = tempfile::tempdir().unwrap();
    let link = elsewhere.path().join("link");
    std::os::unix::fs::symlink(f.dir.path(), &link).unwrap();

    let node = spawn_in(&f, &link, None).await.unwrap();

    let worktree = std::fs::canonicalize(node.worktree.clone().unwrap().path).unwrap();
    assert_eq!(
        std::fs::canonicalize(terminal_cwd(&f, &node).await).unwrap(),
        worktree
    );
}

#[tokio::test]
async fn g2_a_failed_promote_emits_no_event() {
    let wrapper = tempfile::tempdir().unwrap();
    let mut f = Fixture::in_git_project("sleep 30", failing_git(wrapper.path()));
    set_worktrees(&f, true, None).await;
    let group = f.group("team", None).await;
    f.changed();

    promote(&f, &group).await.unwrap_err();

    assert_eq!(f.changed(), 0);
}

#[tokio::test]
async fn g2_a_project_opened_through_a_symlink_accepts_its_own_folder() {
    let mut f = Fixture::in_git_project("sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let elsewhere = tempfile::tempdir().unwrap();
    let link = elsewhere.path().join("project");
    std::os::unix::fs::symlink(f.dir.path(), &link).unwrap();
    let bin = f.dir.path().join("fake-claude");
    let (agents, terminals) = crate::common::open_in_with_git(
        &link.join(".roundup"),
        &f.bus,
        &bin.to_string_lossy(),
        Git::from_env(),
    );
    (f.agents, f.terminals) = (agents, terminals);

    let at_link = spawn_in(&f, &link, None).await.unwrap();
    let at_real = spawn_in(&f, f.dir.path(), None).await.unwrap();

    for node in [&at_link, &at_real] {
        let worktree = std::fs::canonicalize(node.worktree.clone().unwrap().path).unwrap();
        let cwd = std::fs::canonicalize(terminal_cwd(&f, node).await).unwrap();
        assert_eq!(cwd, worktree);
    }
}
