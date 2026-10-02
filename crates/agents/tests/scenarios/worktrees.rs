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

/// `git` on `PATH` that fails `git worktree add` and passes every other command to the real one.
fn failing_git(dir: &Path) -> Git {
    let real = String::from_utf8(
        std::process::Command::new("sh")
            .arg("-c")
            .arg("command -v git")
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_owned();
    let wrapper = dir.join("git");
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nif [ \"$1\" = worktree ] && [ \"$2\" = add ]; then echo fake failure >&2; exit 1; fi\nexec '{real}' \"$@\"\n"
        ),
    )
    .unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    Git::with_path(format!(
        "{}:{}",
        dir.display(),
        std::env::var("PATH").unwrap()
    ))
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

    let node = f.spawn(None, None).await.unwrap();

    let worktree = node.worktree.clone().expect("a Worktree");
    assert_eq!(worktree.branch, format!("roundup/agent-{}", node.id));
    assert_eq!(worktree.base, "main");
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
    assert!(
        err.message.starts_with("worktree_failed: "),
        "{}",
        err.message
    );
    assert!(f.tree().await.is_empty());
    assert_eq!(git_state(f.dir.path()), before);
    assert!(!f.dir.path().join(".roundup/worktrees").exists());
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
