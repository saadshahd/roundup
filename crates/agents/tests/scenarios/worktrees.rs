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

async fn start_door(f: &Fixture, id: &str) -> Result<RailNode, rpc::RpcError> {
    let node = f.call("rail.startDoor", json!({"id": id})).await?;
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
async fn g2_two_start_doors_have_different_cwds() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let g1 = f.room("g1", None).await;
    let g2 = f.room("g2", None).await;
    let cwd_of = |f: &Fixture, node: &RailNode| {
        f.terminals
            .list()
            .into_iter()
            .find(|t| Some(&t.id) == node.terminal_id.as_ref())
            .unwrap()
            .cwd
    };

    let a = start_door(&f, &g1).await.unwrap();
    let b = start_door(&f, &g2).await.unwrap();

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
async fn g2_failed_start_door_leaves_a_stopped_room() {
    let wrapper = tempfile::tempdir().unwrap();
    let f = Fixture::in_git_project("sleep 30", failing_git(wrapper.path()));
    set_worktrees(&f, true, None).await;
    let room = f.room("team", None).await;
    let before = git_state(f.dir.path());

    let err = start_door(&f, &room).await.unwrap_err();

    assert_eq!(err.code, code::INTERNAL);
    let tree = f.tree().await;
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].kind, NodeKind::Room);
    assert!(tree[0].terminal_id.is_none());
    assert_eq!(tree[0].worktree, None);
    assert_eq!(git_state(f.dir.path()), before);
}

#[tokio::test]
async fn g2_failed_start_door_leaves_children_where_they_were() {
    let wrapper = tempfile::tempdir().unwrap();
    let f = Fixture::in_git_project("sleep 30", failing_git(wrapper.path()));
    set_worktrees(&f, true, None).await;
    let room = f.room("team", None).await;
    let child = f.room("child", Some(&room)).await;
    let before = git_state(f.dir.path());

    let err = start_door(&f, &room).await.unwrap_err();

    assert_eq!(err.code, code::INTERNAL);
    let tree = f.tree().await;
    let group_node = tree.iter().find(|node| node.id == room).unwrap();
    assert_eq!(group_node.kind, NodeKind::Room);
    assert!(group_node.terminal_id.is_none());
    let child_node = tree.iter().find(|node| node.id == child).unwrap();
    assert_eq!(child_node.parent.as_deref(), Some(room.as_str()));
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
async fn g2_start_failure_of_start_door_leaves_a_stopped_room() {
    let mut f = Fixture::in_git_project("sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let room = f.room("team", None).await;
    let before = git_state(f.dir.path());
    f.changed();
    std::fs::remove_file(f.dir.path().join(".roundup/rup")).unwrap();

    let err = start_door(&f, &room).await.unwrap_err();

    assert_eq!(err.code, code::INTERNAL);
    let tree = f.tree().await;
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].kind, NodeKind::Room);
    assert!(tree[0].terminal_id.is_none());
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
                scope.spawn(move || {
                    let plan = git.plan(project, &format!("agent-{n}"), "1")?;
                    git.provision(project, &plan)
                })
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
async fn g2_g6_failed_cleanup_retains_ownership_until_reopen_recovers_it() {
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

    assert_ne!(git_state(f.dir.path()), before);
    assert!(f.dir.path().join(".roundup/worktrees/agent-1").exists());
    assert!(f.tree().await[0].worktree.is_some());
    let f = f.reopen();
    assert_eq!(git_state(f.dir.path()), before);
    assert!(f.tree().await.is_empty());
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
async fn g2_a_failed_start_door_emits_no_event() {
    let wrapper = tempfile::tempdir().unwrap();
    let mut f = Fixture::in_git_project("sleep 30", failing_git(wrapper.path()));
    set_worktrees(&f, true, None).await;
    let room = f.room("team", None).await;
    f.changed();

    start_door(&f, &room).await.unwrap_err();

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

#[tokio::test]
async fn g2_g6_door_retry_reuses_recorded_worktree_and_preserves_uncommitted_files() {
    let f = Fixture::in_git_project("exec sleep 30", Git::from_env());
    f.call("project.setWorktrees", json!({"on":true,"check":null}))
        .await
        .unwrap();
    let room = f
        .call("rail.createRoom", json!({"name":"room","parent":null}))
        .await
        .unwrap();
    let id = room["id"].as_str().unwrap();
    let first = f.call("rail.startDoor", json!({"id":id})).await.unwrap();
    let path = std::path::PathBuf::from(first["worktree"]["path"].as_str().unwrap());
    std::fs::write(path.join("uncommitted.txt"), "keep me").unwrap();
    f.call("agent.stop", json!({"id":id})).await.unwrap();
    let rup = f.dir.path().join(".roundup/rup");
    std::fs::remove_file(&rup).unwrap();
    assert!(f.call("rail.startDoor", json!({"id":id})).await.is_err());
    assert_eq!(
        std::fs::read_to_string(path.join("uncommitted.txt")).unwrap(),
        "keep me"
    );
    std::fs::write(rup, "").unwrap();
    let second = f.call("rail.startDoor", json!({"id":id})).await.unwrap();
    assert_eq!(second["worktree"], first["worktree"]);
    assert_eq!(second["attempt"], "3");
    f.call("agent.stop", json!({"id":id})).await.unwrap();
    let renamed = path.with_extension("retained");
    std::fs::rename(&path, &renamed).unwrap();
    let err = f
        .call("rail.startDoor", json!({"id":id}))
        .await
        .unwrap_err();
    assert!(err.message.starts_with("worktree_missing:"));
    let tree = f.call("rail.tree", Value::Null).await.unwrap();
    assert_eq!(tree[0]["worktree"], first["worktree"]);
    std::fs::rename(renamed, path).unwrap();
}

#[tokio::test]
async fn g5_removing_unlanded_work_refuses_before_stopping_the_door() {
    for (dirty, ahead) in [(true, false), (false, true), (true, true)] {
        let f = Fixture::in_git_project("exec sleep 30", Git::from_env());
        set_worktrees(&f, true, None).await;
        let room = f
            .call("rail.createRoom", json!({"name":"room","parent":null}))
            .await
            .unwrap();
        let id = room["id"].as_str().unwrap();
        let node = start_door(&f, id).await.unwrap();
        let worktree = node.worktree.clone().unwrap();
        let path = Path::new(&worktree.path);
        if ahead {
            std::fs::write(path.join("committed"), "keep").unwrap();
            git_output(path, &["add", "committed"]);
            git_output(path, &["commit", "-qm", "keep"]);
        }
        if dirty {
            std::fs::write(path.join("dirty"), "keep").unwrap();
        }
        let before = git_state(f.dir.path());
        let err = f.call("rail.remove", json!({"id":id})).await.unwrap_err();
        assert_eq!(err.code, code::CONFLICT);
        assert!(err.message.starts_with("worktree_unlanded:"));
        assert_eq!(err.message.contains("dirty 1"), dirty);
        assert_eq!(err.message.contains("ahead 1"), ahead);
        assert_eq!(git_state(f.dir.path()), before);
        assert!(
            f.terminals
                .list()
                .iter()
                .any(|t| Some(&t.id) == node.terminal_id.as_ref() && t.running)
        );
        f.call("agent.stop", json!({"id":id})).await.unwrap();
    }
}

#[tokio::test]
async fn g5_removing_clean_room_removes_only_its_worktree() {
    let f = Fixture::in_git_project("exec sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let before = git_state(f.dir.path());
    let room = f
        .call("rail.createRoom", json!({"name":"room","parent":null}))
        .await
        .unwrap();
    let id = room["id"].as_str().unwrap();
    let node = start_door(&f, id).await.unwrap();
    f.call("rail.remove", json!({"id":id})).await.unwrap();
    assert_eq!(git_state(f.dir.path()), before);
    assert!(!Path::new(&node.worktree.unwrap().path).exists());
}

#[tokio::test]
async fn g5_a_write_during_stop_keeps_the_stopped_door_and_worktree() {
    let f = Fixture::in_git_project(
        "trap 'echo keep > stopped-write; exit' HUP\necho ready > ready\nwhile :; do read line; done",
        Git::from_env(),
    );
    set_worktrees(&f, true, None).await;
    let room = f
        .call("rail.createRoom", json!({"name":"room","parent":null}))
        .await
        .unwrap();
    let id = room["id"].as_str().unwrap();
    let node = start_door(&f, id).await.unwrap();
    let worktree = node.worktree.unwrap();
    let path = Path::new(&worktree.path);
    crate::common::until_file(&path.join("ready")).await;
    std::fs::remove_file(path.join("ready")).unwrap();
    let err = f.call("rail.remove", json!({"id":id})).await.unwrap_err();
    assert!(err.message.starts_with("worktree_unlanded:"));
    let retained = f
        .tree()
        .await
        .into_iter()
        .find(|node| node.id == id)
        .unwrap();
    assert_eq!(retained.terminal_id, None);
    assert_eq!(retained.status.unwrap().kind, contracts::Kind::Done);
    assert_eq!(retained.worktree, Some(worktree.clone()));
    assert_eq!(
        std::fs::read_to_string(path.join("stopped-write")).unwrap(),
        "keep\n"
    );
}

#[tokio::test]
async fn g5_missing_worktree_does_not_allow_deleting_an_ahead_branch() {
    let f = Fixture::in_git_project("exec sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let room = f
        .call("rail.createRoom", json!({"name":"room","parent":null}))
        .await
        .unwrap();
    let id = room["id"].as_str().unwrap();
    let node = start_door(&f, id).await.unwrap();
    f.call("agent.stop", json!({"id":id})).await.unwrap();
    let worktree = node.worktree.unwrap();
    let path = Path::new(&worktree.path);
    git_output(path, &["commit", "--allow-empty", "-qm", "keep"]);
    std::fs::remove_dir_all(path).unwrap();
    let err = f.call("rail.remove", json!({"id":id})).await.unwrap_err();
    assert!(err.message.contains("ahead 1"));
    assert_eq!(f.tree().await[0].worktree, Some(worktree.clone()));
    git_output(
        f.dir.path(),
        &[
            "update-ref",
            &format!("refs/heads/{}", worktree.branch),
            git_output(f.dir.path(), &["rev-parse", &worktree.base]).trim(),
        ],
    );
    f.call("rail.remove", json!({"id":id})).await.unwrap();
    assert!(f.tree().await.is_empty());
    assert!(!git_output(f.dir.path(), &["branch", "--list"]).contains(&worktree.branch));
}

#[tokio::test]
async fn g6_reopened_door_reuses_commits_and_dirty_files_without_launching_on_open() {
    let f = Fixture::in_git_project("exec sleep 30", Git::from_env());
    set_worktrees(&f, true, None).await;
    let room = f
        .call("rail.createRoom", json!({"name":"room","parent":null}))
        .await
        .unwrap();
    let id = room["id"].as_str().unwrap().to_owned();
    let node = start_door(&f, &id).await.unwrap();
    let worktree = node.worktree.unwrap();
    let path = Path::new(&worktree.path);
    git_output(path, &["commit", "--allow-empty", "-qm", "keep"]);
    std::fs::write(path.join("dirty"), "keep").unwrap();
    let head = git_output(path, &["rev-parse", "HEAD"]);
    f.call("agent.stop", json!({"id":id})).await.unwrap();
    let f = f.reopen();
    assert!(f.terminals.list().is_empty());
    set_worktrees(&f, false, None).await;
    let next = start_door(&f, &id).await.unwrap();
    assert_eq!(next.worktree, Some(worktree.clone()));
    assert_eq!(git_output(path, &["rev-parse", "HEAD"]), head);
    assert_eq!(std::fs::read_to_string(path.join("dirty")).unwrap(), "keep");
    f.call("agent.stop", json!({"id":id})).await.unwrap();
}

#[tokio::test]
async fn g6_recovery_preserves_changed_or_unowned_provisioning_resources() {
    for change in ["dirty", "commit", "owner"] {
        let f = Fixture::in_git_project("exec sleep 30", Git::from_env());
        let git = Git::from_env();
        let plan = git.plan(f.dir.path(), "owned", "1").unwrap();
        let worktree = git.provision(f.dir.path(), &plan).unwrap();
        match change {
            "dirty" => std::fs::write(worktree.path.join("keep"), "user work").unwrap(),
            "commit" => {
                git_output(
                    &worktree.path,
                    &["commit", "--allow-empty", "-qm", "user work"],
                );
            }
            "owner" => {
                git_output(f.dir.path(), &["update-ref", "-d", &plan.owner]);
            }
            _ => unreachable!(),
        }
        let before = git_state(f.dir.path());
        let head = git_output(&worktree.path, &["rev-parse", "HEAD"]);
        let err = git.recover(f.dir.path(), &plan).unwrap_err();
        assert!(err.message.starts_with("worktree_failed:"));
        assert_eq!(git_state(f.dir.path()), before);
        assert_eq!(git_output(&worktree.path, &["rev-parse", "HEAD"]), head);
        if change == "dirty" {
            assert_eq!(
                std::fs::read_to_string(worktree.path.join("keep")).unwrap(),
                "user work"
            );
        }
    }
}

#[tokio::test]
async fn g5_rejected_slow_precheck_keeps_the_live_door_watched() {
    use std::sync::Arc;
    use std::time::Duration;
    let guard = tempfile::tempdir().unwrap();
    let held = guard.path().join("held");
    let release = guard.path().join("release");
    let git = wrapped_git(
        guard.path(),
        &format!(
            "if [ \"$1\" = status ]; then echo held > '{}'; while [ ! -e '{}' ]; do sleep 0.01; done; fi\nexec \"$REAL\" \"$@\"",
            held.display(),
            release.display()
        ),
    );
    let f = Arc::new(Fixture::in_git_project(
        "echo ready > ready\nwhile read line; do if [ \"$line\" = exit ]; then exit 0; fi; printf '\\033]0;◐ Claude Code\\007'; done",
        git,
    ));
    set_worktrees(&f, true, None).await;
    let id = f.room("room", None).await;
    let node = start_door(&f, &id).await.unwrap();
    let path = PathBuf::from(node.worktree.unwrap().path);
    crate::common::until_file(&path.join("ready")).await;
    std::fs::remove_file(path.join("ready")).unwrap();
    std::fs::write(path.join("dirty"), "keep").unwrap();
    f.call(
        "agent.signal",
        json!({"id":id,"attempt":node.attempt,"payload":{"hook_event_name":"Stop"}}),
    )
    .await
    .unwrap();
    let removing = {
        let f = Arc::clone(&f);
        let id = id.clone();
        tokio::spawn(async move { f.call("rail.remove", json!({"id":id})).await })
    };
    crate::common::until_file(&held).await;
    let mut titles = f
        .terminals
        .subscribe(node.terminal_id.as_ref().unwrap())
        .unwrap();
    f.terminals
        .write(node.terminal_id.as_ref().unwrap(), b"title\n")
        .await
        .unwrap();
    let mut title_received = false;
    let observed = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if matches!(titles.recv().await.unwrap(), contracts::EventData::TerminalTitle(ref title) if title.title.starts_with('◐')) {
                title_received = true;
                break;
            }
        }
        loop {
            if f.tree().await[0].status.as_ref().unwrap().kind == contracts::Kind::Working {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    std::fs::write(release, "go").unwrap();
    let refused = removing.await.unwrap().unwrap_err();
    assert!(refused.message.starts_with("worktree_unlanded:"));
    f.terminals
        .write(node.terminal_id.as_ref().unwrap(), b"exit\n")
        .await
        .unwrap();
    assert!(
        observed.is_ok(),
        "a rejected removal must not revoke its live watcher; title_received={title_received}; tree={:?}; terminal={:?}",
        f.tree().await,
        f.terminals.snapshot(node.terminal_id.as_ref().unwrap())
    );
    f.until(|tree| tree[0].status.as_ref().unwrap().kind == contracts::Kind::Done)
        .await;
}

#[tokio::test]
async fn g5_retry_finishes_a_partial_worktree_removal() {
    let guard = tempfile::tempdir().unwrap();
    let fail = guard.path().join("fail");
    let git = wrapped_git(
        guard.path(),
        &format!(
            "if [ \"$1\" = update-ref ] && [ \"$2\" = -d ] && [ -e '{}' ]; then case \"$3\" in refs/heads/*) rm '{}'; echo refused >&2; exit 1;; esac; fi\nexec \"$REAL\" \"$@\"",
            fail.display(),
            fail.display()
        ),
    );
    let f = Fixture::in_git_project("exec sleep 30", git);
    set_worktrees(&f, true, None).await;
    let before = git_state(f.dir.path());
    let id = f.room("room", None).await;
    let node = start_door(&f, &id).await.unwrap();
    std::fs::write(fail, "once").unwrap();
    f.call("rail.remove", json!({"id":id})).await.unwrap_err();
    assert!(!Path::new(&node.worktree.as_ref().unwrap().path).exists());
    assert_eq!(f.tree().await[0].worktree, node.worktree);
    f.call("rail.remove", json!({"id":id})).await.unwrap();
    assert!(f.tree().await.is_empty());
    assert_eq!(git_state(f.dir.path()), before);
}

#[tokio::test]
async fn g5_branch_advance_after_safety_check_is_not_deleted() {
    let guard = tempfile::tempdir().unwrap();
    let advance = guard.path().join("advance");
    let count = guard.path().join("count");
    let git = wrapped_git(
        guard.path(),
        &format!(
            "if [ \"$1\" = rev-list ]; then n=0; [ ! -e '{}' ] || read n < '{}'; n=$((n+1)); echo \"$n\" > '{}'; if [ \"$n\" = 2 ]; then \"$REAL\" \"$@\" || exit; read branch commit < '{}'; \"$REAL\" update-ref \"$branch\" \"$commit\"; exit; fi; fi\nexec \"$REAL\" \"$@\"",
            count.display(),
            count.display(),
            count.display(),
            advance.display()
        ),
    );
    let f = Fixture::in_git_project("exec sleep 30", git);
    set_worktrees(&f, true, None).await;
    let id = f.room("room", None).await;
    let node = start_door(&f, &id).await.unwrap();
    let worktree = node.worktree.unwrap();
    let commit = git_output(
        f.dir.path(),
        &[
            "commit-tree",
            "HEAD^{tree}",
            "-p",
            "HEAD",
            "-m",
            "keep concurrent work",
        ],
    );
    std::fs::write(
        advance,
        format!("refs/heads/{} {}\n", worktree.branch, commit.trim()),
    )
    .unwrap();
    f.call("rail.remove", json!({"id":id})).await.unwrap_err();
    assert_eq!(
        git_output(f.dir.path(), &["rev-parse", &worktree.branch]),
        commit
    );
    assert_eq!(f.tree().await[0].worktree, Some(worktree));
    assert!(
        f.call("rail.remove", json!({"id":id}))
            .await
            .unwrap_err()
            .message
            .contains("ahead 1")
    );
}

#[tokio::test]
async fn g6_a_record_recovery_refuses_keeps_its_work_and_the_project_opens() {
    for record in ["legacy", "unowned"] {
        let f = Fixture::in_git_project("exec sleep 30", Git::from_env());
        set_worktrees(&f, true, None).await;
        let node = f.spawn(None, None).await.unwrap();
        let path = PathBuf::from(node.worktree.clone().unwrap().path);
        std::fs::write(path.join("keep"), "user work").unwrap();
        f.call("agent.stop", json!({"id": node.id})).await.unwrap();
        // A crash during `git` leaves the row provisioning: before ownership proofs with no
        // owner at all, or with an owner ref that no longer proves the destination is ours.
        let owner = if record == "legacy" {
            "NULL"
        } else {
            "'refs/roundup/gone'"
        };
        rusqlite::Connection::open(f.dir.path().join(".roundup/agents.db"))
            .unwrap()
            .execute(
                &format!(
                    "UPDATE nodes SET worktree_state = 'provisioning', worktree_owner = {owner},
                     worktree_commit = 'deadbeef' WHERE id = ?"
                ),
                [&node.id],
            )
            .unwrap();
        let before = git_state(f.dir.path());

        let f = f.reopen();

        let kept = f.tree().await.into_iter().find(|n| n.id == node.id);
        assert_eq!(kept.and_then(|n| n.worktree), node.worktree, "{record}");
        assert_eq!(git_state(f.dir.path()), before, "{record}");
        assert_eq!(
            std::fs::read_to_string(path.join("keep")).unwrap(),
            "user work"
        );
        let other = f.spawn(None, None).await.unwrap();
        f.call("agent.stop", json!({"id": other.id})).await.unwrap();
    }
}
