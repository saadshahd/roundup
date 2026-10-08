//! G4 landing, G5 removal and discard of an Agent, G6 restart (`scenarios/worktrees.md`).

use std::path::{Path, PathBuf};

use agents::worktree::Git;
use rpc::code;
use serde_json::{Value, json};

use crate::common::Fixture;

fn git(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn commit_file(dir: &Path, name: &str, text: &str) {
    std::fs::write(dir.join(name), text).unwrap();
    git(dir, &["add", name]);
    git(dir, &["commit", "-q", "-m", name]);
}

async fn on(f: &Fixture, check: Option<&str>) -> (String, PathBuf, String) {
    f.call("project.setWorktrees", json!({"on": true, "check": check}))
        .await
        .unwrap();
    let node = f.spawn(None, None).await.unwrap();
    let worktree = node.worktree.expect("a Worktree");
    (node.id, worktree.path.into(), worktree.branch)
}

async fn land(f: &Fixture, id: &str) -> Result<Value, rpc::RpcError> {
    f.call("agent.land", json!({"id": id})).await
}

/// What a failed call must leave alone: both branches, the Worktree's status and the Project's.
fn snapshot(f: &Fixture, path: &Path, branch: &str) -> [String; 4] {
    [
        git(f.dir.path(), &["rev-parse", "main", branch]),
        git(path, &["status", "--porcelain=v2"]),
        git(f.dir.path(), &["status", "--porcelain=v2"]),
        git(f.dir.path(), &["worktree", "list"]),
    ]
}

fn assert_refused(err: &rpc::RpcError, code: i64, name: &str) {
    assert_eq!(err.code, code, "{}", err.message);
    assert!(
        err.message.starts_with(&format!("{name}: ")),
        "{}",
        err.message
    );
}

#[tokio::test]
async fn g4_a_passing_check_lands_and_the_worktree_is_level() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    let (id, path, branch) = on(&f, Some("true")).await;
    commit_file(&path, "agent.txt", "a");
    commit_file(f.dir.path(), "base.txt", "b");

    let landed = land(&f, &id).await.unwrap();
    let main = git(f.dir.path(), &["rev-parse", "main"]).trim().to_owned();
    assert_eq!(landed, json!({"base": main}));
    assert_eq!(git(&path, &["rev-parse", "HEAD"]).trim(), main);
    assert_eq!(git(f.dir.path(), &["rev-parse", &branch]).trim(), main);
    assert!(f.dir.path().join("agent.txt").exists());
    assert!(f.dir.path().join("base.txt").exists());
    assert_eq!(
        f.call("agent.worktreeState", json!({"id": id}))
            .await
            .unwrap(),
        json!({"ahead": 0, "behind": 0, "dirty": false})
    );
}

#[tokio::test]
async fn g4_refusals_come_in_the_scenarios_order_and_change_nothing() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    let plain = f.spawn(None, None).await.unwrap();
    assert_refused(
        &land(&f, &plain.id).await.unwrap_err(),
        code::NOT_FOUND,
        "no_worktree",
    );

    let (id, path, branch) = on(&f, None).await;
    commit_file(&path, "agent.txt", "a");
    std::fs::write(path.join("loose.txt"), "x").unwrap();
    let before = snapshot(&f, &path, &branch);
    // check_missing wins over worktree_dirty
    assert_refused(
        &land(&f, &id).await.unwrap_err(),
        code::CONFLICT,
        "check_missing",
    );
    assert_eq!(snapshot(&f, &path, &branch), before);

    f.call("project.setWorktrees", json!({"on": true, "check": "true"}))
        .await
        .unwrap();
    // worktree_dirty wins over base_moved
    git(f.dir.path(), &["checkout", "-q", "-b", "other"]);
    assert_refused(
        &land(&f, &id).await.unwrap_err(),
        code::CONFLICT,
        "worktree_dirty",
    );
    std::fs::remove_file(path.join("loose.txt")).unwrap();
    let before = snapshot(&f, &path, &branch);
    assert_refused(
        &land(&f, &id).await.unwrap_err(),
        code::CONFLICT,
        "base_moved",
    );
    assert_eq!(snapshot(&f, &path, &branch), before);
    git(f.dir.path(), &["checkout", "-q", "main"]);

    // worktree_missing wins over everything after it
    std::fs::remove_dir_all(&path).unwrap();
    assert_refused(
        &land(&f, &id).await.unwrap_err(),
        code::NOT_FOUND,
        "worktree_missing",
    );
}

#[tokio::test]
async fn g4_a_deleted_base_is_base_moved() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    let (id, path, branch) = on(&f, Some("true")).await;
    commit_file(&path, "agent.txt", "a");
    git(f.dir.path(), &["checkout", "-q", "--detach"]);
    git(f.dir.path(), &["branch", "-D", "main"]);
    let before = git(f.dir.path(), &["rev-parse", &branch]);
    assert_refused(
        &land(&f, &id).await.unwrap_err(),
        code::CONFLICT,
        "base_moved",
    );
    assert_eq!(git(f.dir.path(), &["rev-parse", &branch]), before);
}

#[tokio::test]
async fn g4_a_conflict_is_aborted_and_lists_the_paths() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    let (id, path, branch) = on(&f, Some("true")).await;
    commit_file(&path, "README.md", "from agent\n");
    commit_file(f.dir.path(), "README.md", "from base\n");
    let before = snapshot(&f, &path, &branch);
    let err = land(&f, &id).await.unwrap_err();
    assert_refused(&err, code::CONFLICT, "landing_conflict");
    assert!(err.message.contains("README.md"), "{}", err.message);
    assert_eq!(snapshot(&f, &path, &branch), before);
    assert!(!path.join(".git").is_dir());
    assert_eq!(git(&path, &["status", "--porcelain"]), "");
}

#[tokio::test]
async fn g4_a_failing_check_restores_the_branch_and_keeps_the_last_lines() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    let (id, path, branch) = on(
        &f,
        Some("seq 1 30; echo scribble > left.txt; echo oops >&2; exit 3"),
    )
    .await;
    commit_file(&path, "agent.txt", "a");
    commit_file(f.dir.path(), "base.txt", "b");
    let before = snapshot(&f, &path, &branch);
    let err = land(&f, &id).await.unwrap_err();
    assert_refused(&err, code::CONFLICT, "check_failed");
    assert!(err.message.contains("oops") && err.message.contains("30"));
    assert!(!err.message.contains("\n10\n"), "only the last 20 lines");
    assert_eq!(snapshot(&f, &path, &branch), before);
    assert!(!path.join("left.txt").exists());

    f.call(
        "project.setWorktrees",
        json!({"on": true, "check": "false"}),
    )
    .await
    .unwrap();
    assert_refused(
        &land(&f, &id).await.unwrap_err(),
        code::CONFLICT,
        "check_failed",
    );
    assert_eq!(snapshot(&f, &path, &branch), before);
}

#[tokio::test]
async fn g4_a_check_that_leaves_changes_fails() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    let (id, path, branch) = on(&f, Some("echo x > made-by-check")).await;
    commit_file(&path, "agent.txt", "a");
    let before = snapshot(&f, &path, &branch);
    let err = land(&f, &id).await.unwrap_err();
    assert_refused(&err, code::CONFLICT, "check_failed");
    assert!(
        err.message.ends_with("check left changes"),
        "{}",
        err.message
    );
    assert_eq!(snapshot(&f, &path, &branch), before);
}

#[tokio::test]
async fn g4_a_dirty_project_checkout_is_base_dirty_only_when_it_would_be_touched() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    let (id, path, branch) = on(&f, Some("true")).await;
    commit_file(&path, "agent.txt", "a");
    std::fs::write(f.dir.path().join("agent.txt"), "mine").unwrap();
    let before = snapshot(&f, &path, &branch);
    assert_refused(
        &land(&f, &id).await.unwrap_err(),
        code::CONFLICT,
        "base_dirty",
    );
    assert_eq!(snapshot(&f, &path, &branch), before);
    assert_eq!(
        std::fs::read_to_string(f.dir.path().join("agent.txt")).unwrap(),
        "mine"
    );

    std::fs::remove_file(f.dir.path().join("agent.txt")).unwrap();
    std::fs::write(f.dir.path().join("unrelated"), "mine").unwrap();
    land(&f, &id).await.unwrap();
    assert_eq!(
        std::fs::read_to_string(f.dir.path().join("unrelated")).unwrap(),
        "mine"
    );
}

#[tokio::test]
async fn g5_remove_keeps_unlanded_work_and_deletes_a_clean_agents_worktree() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    let before = (
        git(f.dir.path(), &["branch", "--list"]),
        git(f.dir.path(), &["worktree", "list"]),
    );
    let (id, path, branch) = on(&f, None).await;
    commit_file(&path, "agent.txt", "a");
    std::fs::write(path.join("loose.txt"), "x").unwrap();
    let err = f.call("rail.remove", json!({"id": id})).await.unwrap_err();
    assert_refused(&err, code::CONFLICT, "worktree_unlanded");
    assert!(err.message.contains("dirty 1") && err.message.contains("ahead 1"));
    assert!(path.join("loose.txt").exists());
    assert!(f.tree().await.iter().any(|n| n.id == id));

    git(&path, &["reset", "-q", "--hard", "main"]);
    std::fs::remove_file(path.join("loose.txt")).unwrap();
    f.call("rail.remove", json!({"id": id})).await.unwrap();
    assert!(!path.exists());
    assert_eq!(
        (
            git(f.dir.path(), &["branch", "--list"]),
            git(f.dir.path(), &["worktree", "list"])
        ),
        before
    );
    assert!(!git(f.dir.path(), &["branch", "--list"]).contains(&branch));
}

#[tokio::test]
async fn g5_a_write_while_stopping_keeps_the_stopped_agent_and_its_files() {
    let f = Fixture::in_git_project(
        "trap 'echo late > late.txt; exit' HUP TERM\necho up > up\nwhile :; do sleep 0.1; done",
        Git::from_env(),
    );
    let (id, path, branch) = on(&f, None).await;
    crate::common::until_file(&path.join("up")).await;
    std::fs::remove_file(path.join("up")).unwrap();
    let err = f.call("rail.remove", json!({"id": id})).await.unwrap_err();
    assert_refused(&err, code::CONFLICT, "worktree_unlanded");
    let node = f.tree().await.into_iter().find(|n| n.id == id).unwrap();
    assert_eq!(node.status.unwrap().kind, contracts::Kind::Done);
    assert_eq!(
        std::fs::read_to_string(path.join("late.txt")).unwrap(),
        "late\n"
    );
    assert!(git(f.dir.path(), &["branch", "--list"]).contains(&branch));
}

#[tokio::test]
async fn g5_discard_drops_dirty_and_ahead_work_and_refuses_without_a_worktree() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    let before = (
        git(f.dir.path(), &["branch", "--list"]),
        git(f.dir.path(), &["worktree", "list"]),
    );
    let plain = f.spawn(None, None).await.unwrap();
    let err = f
        .call("agent.discard", json!({"id": plain.id}))
        .await
        .unwrap_err();
    assert_refused(&err, code::NOT_FOUND, "no_worktree");
    let terminal = f
        .call("rail.spawnTerminal", json!({"parent": null}))
        .await
        .map(|n| n["id"].as_str().unwrap().to_owned());
    if let Ok(terminal) = terminal {
        let err = f
            .call("agent.discard", json!({"id": terminal}))
            .await
            .unwrap_err();
        assert_refused(&err, code::NOT_FOUND, "no_worktree");
    }
    f.call("rail.remove", json!({"id": plain.id}))
        .await
        .unwrap_or_default();

    let (id, path, _) = on(&f, None).await;
    commit_file(&path, "agent.txt", "a");
    std::fs::write(path.join("loose.txt"), "x").unwrap();
    assert_eq!(
        f.call("agent.discard", json!({"id": id})).await.unwrap(),
        Value::Null
    );
    assert!(!path.exists());
    assert!(f.tree().await.iter().all(|n| n.id != id));
    let after = (
        git(f.dir.path(), &["branch", "--list"]),
        git(f.dir.path(), &["worktree", "list"]),
    );
    assert_eq!(after, before);
}

#[tokio::test]
async fn g5_a_directory_already_gone_is_not_an_error() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    let before = (
        git(f.dir.path(), &["branch", "--list"]),
        git(f.dir.path(), &["worktree", "list"]),
    );

    let (clean, path, _) = on(&f, None).await;
    f.call("agent.stop", json!({"id": clean})).await.unwrap();
    std::fs::remove_dir_all(&path).unwrap();
    f.call("rail.remove", json!({"id": clean})).await.unwrap();

    let (ahead, path, _) = on(&f, None).await;
    f.call("agent.stop", json!({"id": ahead})).await.unwrap();
    commit_file(&path, "agent.txt", "a");
    std::fs::remove_dir_all(&path).unwrap();
    let err = f
        .call("rail.remove", json!({"id": ahead}))
        .await
        .unwrap_err();
    assert_refused(&err, code::CONFLICT, "worktree_unlanded");
    assert!(err.message.contains("ahead 1"));
    let user = f.dir.path().join(".roundup/user-gone");
    git(
        f.dir.path(),
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "user-gone",
            user.to_str().unwrap(),
        ],
    );
    std::fs::remove_dir_all(&user).unwrap();
    f.call("agent.discard", json!({"id": ahead})).await.unwrap();
    assert!(git(f.dir.path(), &["worktree", "list"]).contains("user-gone"));
    git(
        f.dir.path(),
        &["worktree", "remove", "--force", user.to_str().unwrap()],
    );
    git(f.dir.path(), &["branch", "-D", "user-gone"]);

    let after = (
        git(f.dir.path(), &["branch", "--list"]),
        git(f.dir.path(), &["worktree", "list"]),
    );
    assert_eq!(after, before);
}

#[tokio::test]
async fn g6_worktrees_are_found_by_stored_path_after_a_restart() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    let (id, path, branch) = on(&f, Some("true")).await;
    commit_file(&path, "agent.txt", "a");
    std::fs::write(path.join("loose.txt"), "x").unwrap();
    let stranger = f.dir.path().join(".roundup/stranger");
    git(
        f.dir.path(),
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "user-branch",
            stranger.to_str().unwrap(),
        ],
    );
    let room = f.room("elsewhere", None).await;
    f.call("rail.move", json!({"id": id, "parent": room, "index": 0}))
        .await
        .unwrap();
    f.call("agent.stop", json!({"id": id})).await.unwrap();
    let tree = f.tree().await;
    let state = f
        .call("agent.worktreeState", json!({"id": id}))
        .await
        .unwrap();
    let lists = (
        git(f.dir.path(), &["branch", "--list"]),
        git(f.dir.path(), &["worktree", "list"]),
    );

    let f = f.reopen();
    let placed = |tree: &[contracts::agent::RailNode]| {
        tree.iter()
            .map(|n| (n.id.clone(), n.parent.clone(), n.worktree.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(placed(&f.tree().await), placed(&tree));
    assert_eq!(
        f.call("agent.worktreeState", json!({"id": id}))
            .await
            .unwrap(),
        state
    );
    assert_eq!(
        (
            git(f.dir.path(), &["branch", "--list"]),
            git(f.dir.path(), &["worktree", "list"])
        ),
        lists
    );
    assert_eq!(
        std::fs::read_to_string(path.join("loose.txt")).unwrap(),
        "x"
    );
    assert!(git(f.dir.path(), &["branch", "--list"]).contains(&branch));
    assert!(stranger.exists());

    // a user-made worktree is no node's, so no call reaches it
    assert!(f.tree().await.iter().all(|n| {
        n.worktree
            .as_ref()
            .is_none_or(|w| w.branch != "user-branch")
    }));
    assert!(
        f.call("agent.discard", json!({"id": "user-branch"}))
            .await
            .is_err()
    );
    assert!(stranger.exists());

    std::fs::remove_dir_all(&path).unwrap();
    let f = f.reopen();
    let err = f
        .call("agent.worktreeState", json!({"id": id}))
        .await
        .unwrap_err();
    assert_refused(&err, code::NOT_FOUND, "worktree_missing");
    assert!(f.tree().await.iter().any(|n| n.id == id));
}
