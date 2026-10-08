//! G3 a Worktree's status, and G7 a Rail move never touches a Worktree (`scenarios/worktrees.md`).

use std::path::Path;

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

fn commit_file(dir: &Path, name: &str) {
    std::fs::write(dir.join(name), name).unwrap();
    git(dir, &["add", name]);
    git(dir, &["commit", "-q", "-m", name]);
}

async fn state(f: &Fixture, id: &str) -> Result<Value, rpc::RpcError> {
    f.call("agent.worktreeState", json!({"id": id})).await
}

async fn on(f: &Fixture) -> (String, std::path::PathBuf) {
    f.call("project.setWorktrees", json!({"on": true, "check": null}))
        .await
        .unwrap();
    let node = f.spawn(None, None).await.unwrap();
    let path = node.worktree.expect("a Worktree").path.into();
    (node.id, path)
}

#[tokio::test]
async fn g3_counts_commits_both_ways_and_untracked_files() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    let (id, path) = on(&f).await;
    let project = f.dir.path();
    let before = git(project, &["rev-parse", "main"]);

    assert_eq!(
        state(&f, &id).await.unwrap(),
        json!({"ahead": 0, "behind": 0, "dirty": false})
    );
    commit_file(&path, "agent.txt");
    assert_eq!(
        state(&f, &id).await.unwrap(),
        json!({"ahead": 1, "behind": 0, "dirty": false})
    );
    commit_file(project, "base.txt");
    assert_eq!(
        state(&f, &id).await.unwrap(),
        json!({"ahead": 1, "behind": 1, "dirty": false})
    );
    std::fs::write(path.join("loose.txt"), "x").unwrap();
    let status = git(&path, &["status", "--porcelain=v2"]);
    assert_eq!(
        state(&f, &id).await.unwrap(),
        json!({"ahead": 1, "behind": 1, "dirty": true})
    );
    assert_eq!(git(&path, &["status", "--porcelain=v2"]), status);
    assert_ne!(git(project, &["rev-parse", "main"]), before);
}

#[tokio::test]
async fn g3_without_a_worktree_or_directory_it_fails_by_name() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    let plain = f.spawn(None, None).await.unwrap();
    let err = state(&f, &plain.id).await.unwrap_err();
    assert_eq!(err.code, code::NOT_FOUND);
    assert!(err.message.starts_with("no_worktree: "), "{}", err.message);

    let (id, path) = on(&f).await;
    std::fs::remove_dir_all(&path).unwrap();
    let err = state(&f, &id).await.unwrap_err();
    assert_eq!(err.code, code::NOT_FOUND);
    assert!(
        err.message
            .starts_with(&format!("worktree_missing: {}", path.display())),
        "{}",
        err.message
    );
    assert!(f.tree().await.iter().any(|node| node.id == id));
}

#[tokio::test]
async fn g7_a_rail_move_changes_only_parent_and_order() {
    let f = Fixture::in_git_project("sleep 30", Git::from_env());
    let (id, path) = on(&f).await;
    commit_file(&path, "kept.txt");
    std::fs::write(path.join("README.md"), "edited\n").unwrap();
    std::fs::write(path.join("loose.txt"), "x").unwrap();
    let room = f.room("elsewhere", None).await;
    let snapshot = |f: &Fixture| {
        let project = f.dir.path();
        (
            git(&path, &["diff"]),
            git(&path, &["diff", "--cached"]),
            git(&path, &["status", "--porcelain=v2"]),
            git(project, &["worktree", "list"]),
            git(
                project,
                &["rev-parse", "main", &format!("roundup/agent-{id}")],
            ),
        )
    };
    let before = snapshot(&f);
    let node_before = f.tree().await.into_iter().find(|n| n.id == id).unwrap();
    let state_before = state(&f, &id).await.unwrap();

    f.call("rail.move", json!({"id": id, "parent": room, "index": 0}))
        .await
        .unwrap();
    f.call("rail.move", json!({"id": id, "parent": null, "index": 0}))
        .await
        .unwrap();

    let node = f.tree().await.into_iter().find(|n| n.id == id).unwrap();
    assert_eq!(node.worktree, node_before.worktree);
    assert_eq!(node.terminal_id, node_before.terminal_id);
    assert_eq!(snapshot(&f), before);
    assert_eq!(state(&f, &id).await.unwrap(), state_before);

    let missing = f
        .call("rail.move", json!({"id": id, "parent": "nope", "index": 0}))
        .await
        .unwrap_err();
    assert_eq!(missing.code, code::NOT_FOUND);
    let under_agent = f
        .call("rail.move", json!({"id": room, "parent": id, "index": 0}))
        .await
        .unwrap_err();
    assert_eq!(under_agent.code, code::CONFLICT);
    assert_eq!(snapshot(&f), before);
}
