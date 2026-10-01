//! A10 a Terminal in the Rail, and A12 for Terminal nodes.

use contracts::agent::{NodeKind, RailNode};
use rpc::code;
use serde_json::{Value, json};

use crate::common::Fixture;

impl Fixture {
    async fn spawn_terminal(&self, parent: Option<&str>) -> Result<RailNode, rpc::RpcError> {
        let cwd = self.dir.path().to_string_lossy().into_owned();
        self.spawn_terminal_in(&cwd, parent).await
    }

    async fn spawn_terminal_in(
        &self,
        cwd: &str,
        parent: Option<&str>,
    ) -> Result<RailNode, rpc::RpcError> {
        let node: Value = self
            .call("rail.spawnTerminal", json!({"cwd": cwd, "parent": parent}))
            .await?;
        Ok(serde_json::from_value(node).unwrap())
    }
}

fn shell_name() -> String {
    std::env::var_os("SHELL")
        .and_then(|shell| {
            std::path::Path::new(&shell)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "shell".into())
}

#[tokio::test]
async fn a10_a_terminal_node_runs_the_login_shell_and_is_announced() {
    let mut f = Fixture::new();
    f.changed();

    let node = f.spawn_terminal(None).await.unwrap();

    assert_eq!(
        (node.kind, node.name.as_str()),
        (NodeKind::Terminal, shell_name().as_str())
    );
    assert_eq!(node.status, None);
    let terminal_id = node.terminal_id.clone().expect("a Terminal runs behind it");
    let running = f.terminals.list();
    assert_eq!(running.len(), 1);
    assert_eq!((&running[0].id, running[0].running), (&terminal_id, true));
    assert_eq!(f.tree().await, [node]);
    assert_eq!(f.changed(), 1);
}

#[tokio::test]
async fn a10_a_terminal_is_placed_last_under_its_parent() {
    let f = Fixture::running("sleep 30");
    let group = f.group("team", None).await;
    f.spawn(Some(&group), None).await.unwrap();

    let node = f.spawn_terminal(Some(&group)).await.unwrap();

    assert_eq!(
        (node.parent.as_deref(), node.order),
        (Some(group.as_str()), 1)
    );
}

#[tokio::test]
async fn a10_nothing_nests_under_an_agent_or_a_terminal_and_an_unknown_parent_is_not_found() {
    let f = Fixture::running("sleep 30");
    let agent = f.spawn(None, None).await.unwrap();
    let terminal = f.spawn_terminal(None).await.unwrap();

    let under_agent = f.spawn_terminal(Some(&agent.id)).await.unwrap_err();
    let under_terminal = f.spawn_terminal(Some(&terminal.id)).await.unwrap_err();
    let under_unknown = f.spawn_terminal(Some("999")).await.unwrap_err();

    assert_eq!(
        (under_agent.code, under_terminal.code, under_unknown.code),
        (code::CONFLICT, code::CONFLICT, code::NOT_FOUND)
    );
    assert_eq!(f.tree().await.len(), 2);
    assert_eq!(f.terminals.list().len(), 2);
}

#[tokio::test]
async fn a10_a_cwd_that_is_not_a_directory_adds_no_node() {
    let mut f = Fixture::new();
    f.changed();
    let file = f.dir.path().join("fake-claude");

    let err = f
        .spawn_terminal_in(&file.to_string_lossy(), None)
        .await
        .unwrap_err();

    assert_eq!(err.code, code::INVALID_PARAMS);
    assert!(f.tree().await.is_empty());
    assert_eq!(f.changed(), 0);
}

#[tokio::test]
async fn a10_a_terminal_whose_program_exits_stays_in_the_rail_without_a_status() {
    let f = Fixture::new();
    let node = f.spawn_terminal(None).await.unwrap();
    let terminal_id = node.terminal_id.clone().unwrap();

    f.terminals.kill(&terminal_id).await.unwrap();

    let tree = f
        .until(|_| f.terminals.list().iter().all(|t| !t.running))
        .await;
    assert_eq!(tree.len(), 1);
    assert_eq!(
        (tree[0].id.as_str(), tree[0].status.clone()),
        (node.id.as_str(), None)
    );
}

#[tokio::test]
async fn a12_a_terminal_node_comes_back_without_its_terminal_id() {
    let f = Fixture::new();
    let node = f.spawn_terminal(None).await.unwrap();

    let tree = f.reopen().tree().await;

    assert_eq!(tree[0].id, node.id);
    assert_eq!(tree[0].terminal_id, None);
}
