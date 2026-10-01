//! A4 spawn, and A2, A3, A8 as seen through a real Terminal running a fake `claude`.

use std::path::Path;
use std::time::Duration;

use contracts::Kind;
use contracts::agent::{NodeKind, RailNode};
use rpc::code;
use serde_json::{Value, json};

use crate::common::Fixture;

impl Fixture {
    async fn spawn(&self, parent: Option<&str>) -> Result<RailNode, rpc::RpcError> {
        let cwd = self.dir.path().to_string_lossy().into_owned();
        let node = self
            .call(
                "agent.spawn",
                json!({"cwd": cwd, "prompt": null, "parent": parent}),
            )
            .await?;
        Ok(serde_json::from_value(node).unwrap())
    }
}

async fn until_file(path: &Path) -> String {
    for _ in 0..500 {
        if let Ok(text) = std::fs::read_to_string(path)
            && !text.is_empty()
        {
            return text;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("{} was never written", path.display());
}

fn status_of<'a>(tree: &'a [RailNode], id: &str) -> &'a contracts::Status {
    tree.iter()
        .find(|node| node.id == id)
        .and_then(|node| node.status.as_ref())
        .expect("an Agent has a Status")
}

#[tokio::test]
async fn a4_spawn_runs_claude_with_the_agents_settings_and_lists_an_agent() {
    let mut f = Fixture::running("echo \"$@\" > \"$(dirname \"$0\")/argv\"; sleep 30");
    f.changed();
    let node = f.spawn(None).await.unwrap();

    assert_eq!(node.kind, NodeKind::Agent);
    assert!(node.terminal_id.is_some() && !node.meta);
    assert_eq!(node.status.as_ref().map(|s| s.kind), Some(Kind::Working));
    assert_eq!(f.tree().await, std::slice::from_ref(&node));
    assert_eq!(f.changed(), 1);

    let argv = until_file(&f.dir.path().join("argv")).await;
    let settings = argv
        .trim()
        .strip_prefix("--settings ")
        .expect("--settings first");
    let settings: Value =
        serde_json::from_str(&std::fs::read_to_string(settings).unwrap()).unwrap();
    assert_eq!(
        settings["hooks"]["Stop"][0]["hooks"][0]["command"],
        format!(
            "'{}' signal {}",
            f.dir.path().join("rup").display(),
            node.id
        )
    );
    let trusted = f.dir.path().canonicalize().unwrap();
    let config: Value =
        serde_json::from_str(&std::fs::read_to_string(f.dir.path().join("claude.json")).unwrap())
            .unwrap();
    assert_eq!(
        config["projects"][trusted.to_str().unwrap()]["hasTrustDialogAccepted"],
        true
    );
}

#[tokio::test]
async fn a4_spawn_nests_under_a_group_and_never_under_an_agent() {
    let f = Fixture::running("sleep 30");
    let group = f.group("team", None).await;
    let child = f.spawn(Some(&group)).await.unwrap();
    assert_eq!(child.parent.as_deref(), Some(group.as_str()));

    let err = f.spawn(Some(&child.id)).await.unwrap_err();
    assert_eq!(err.code, code::CONFLICT);
    assert_eq!(f.tree().await.len(), 2);
}

#[tokio::test]
async fn a4_a_cwd_outside_the_project_folder_is_refused_and_leaves_nothing() {
    let f = Fixture::running("sleep 30");
    let err = f
        .call(
            "agent.spawn",
            json!({"cwd": "/usr", "prompt": null, "parent": null}),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, code::INVALID_PARAMS);
    assert!(f.tree().await.is_empty());
    assert!(!f.dir.path().join("claude.json").exists());
}

#[tokio::test]
async fn a4_a_spawn_that_cannot_start_leaves_no_agent_behind() {
    let f = Fixture::running("sleep 30");
    let err = f
        .call(
            "agent.spawn",
            json!({"cwd": "/no/such/dir", "prompt": null, "parent": null}),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, code::INVALID_PARAMS);
    assert!(f.tree().await.is_empty());
}

#[tokio::test]
async fn a3_a_clean_exit_makes_the_agent_done() {
    let f = Fixture::running("exit 0");
    let node = f.spawn(None).await.unwrap();
    let tree = f
        .until(|t| status_of(t, &node.id).kind != Kind::Working)
        .await;
    assert_eq!(status_of(&tree, &node.id).kind, Kind::Done);
}

#[tokio::test]
async fn a3_a_non_zero_exit_makes_the_agent_an_error() {
    let f = Fixture::running("exit 3");
    let node = f.spawn(None).await.unwrap();
    let tree = f
        .until(|t| status_of(t, &node.id).kind != Kind::Working)
        .await;
    let status = status_of(&tree, &node.id);
    assert_eq!(
        (status.kind, status.label.as_str()),
        (Kind::Error, "exited 3")
    );
}

#[tokio::test]
async fn a2_a_star_title_from_the_terminal_ends_working() {
    let f = Fixture::running("printf '\\033]0;\\342\\234\\263 Claude Code\\007'; sleep 30");
    let node = f.spawn(None).await.unwrap();
    let tree = f
        .until(|t| status_of(t, &node.id).kind != Kind::Working)
        .await;
    assert_eq!(status_of(&tree, &node.id).kind, Kind::Idle);
}

#[tokio::test]
async fn a8_an_agent_whose_terminal_is_gone_comes_back_done() {
    let f = Fixture::running("sleep 30");
    let node = f.spawn(None).await.unwrap();
    let tree = f.reopen().tree().await;
    assert_eq!(tree[0].id, node.id);
    assert_eq!(tree[0].status.as_ref().map(|s| s.kind), Some(Kind::Done));
    assert_eq!(tree[0].terminal_id, node.terminal_id);
}
