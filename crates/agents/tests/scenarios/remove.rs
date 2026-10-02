//! A16 `rail.remove`.

use std::path::Path;

use contracts::Kind;
use contracts::agent::{NodeKind, RailNode};
use rpc::code;
use serde_json::{Value, json};

use crate::common::{Fixture, status_of};

impl Fixture {
    async fn remove(&self, id: &str) -> Result<Value, rpc::RpcError> {
        self.call("rail.remove", json!({"id": id})).await
    }
}

async fn spawn_terminal(f: &Fixture, parent: Option<&str>) -> RailNode {
    let cwd = f.dir.path().to_string_lossy().into_owned();
    let node = f
        .call("rail.spawnTerminal", json!({"cwd": cwd, "parent": parent}))
        .await
        .unwrap();
    serde_json::from_value(node).unwrap()
}

async fn promote(f: &Fixture, id: &str) {
    f.call("rail.promote", json!({"id": id})).await.unwrap();
}

/// `name:order` of the children of `parent`, in order; an Agent shows as `agent` whatever its name.
fn names(tree: &[RailNode], parent: Option<&str>) -> Vec<String> {
    let mut kids: Vec<_> = tree
        .iter()
        .filter(|n| n.parent.as_deref() == parent)
        .collect();
    kids.sort_by_key(|n| n.order);
    kids.into_iter()
        .map(|n| match n.kind {
            NodeKind::Agent => format!("agent:{}", n.order),
            _ => format!("{}:{}", n.name, n.order),
        })
        .collect()
}

/// A second connection holding a write transaction makes a write through `Rail`'s own connection
/// fail with a real SQLite error once rusqlite's default 5 s `busy_timeout` elapses.
fn lock_db_for_writes(dir: &Path) -> rusqlite::Connection {
    let lock = rusqlite::Connection::open(dir.join("agents.db")).unwrap();
    lock.execute_batch("BEGIN IMMEDIATE;").unwrap();
    lock
}

#[tokio::test]
async fn a16_a_running_agent_is_stopped_and_removed() {
    let f = Fixture::running("sleep 30");
    let agent = f.spawn(None, None).await.unwrap();

    f.remove(&agent.id).await.unwrap();

    assert!(f.tree().await.is_empty());
    let terminal_id = agent.terminal_id.unwrap();
    let terminal = f.terminals.list().into_iter().find(|t| t.id == terminal_id);
    assert!(terminal.is_none_or(|t| !t.running));
}

#[tokio::test]
async fn a16_a_running_terminal_is_killed_and_removed() {
    let f = Fixture::new();
    let node = spawn_terminal(&f, None).await;

    f.remove(&node.id).await.unwrap();

    assert!(f.tree().await.is_empty());
    let terminal = f.terminals.list();
    assert_eq!(terminal.len(), 1);
    assert!(!terminal[0].running);
}

#[tokio::test]
async fn a16_a_plain_groups_children_move_to_its_parent_at_its_place() {
    let f = Fixture::new();
    f.group("before", None).await;
    let g = f.group("g", None).await;
    f.group("after", None).await;
    let child_a = f.group("child-a", Some(&g)).await;
    let child_b = f.group("child-b", Some(&g)).await;

    f.remove(&g).await.unwrap();

    let tree = f.tree().await;
    assert_eq!(
        names(&tree, None),
        ["before:0", "child-a:1", "child-b:2", "after:3"]
    );
    assert!(tree.iter().all(|n| n.id != g));
    assert!(tree.iter().any(|n| n.id == child_a));
    assert!(tree.iter().any(|n| n.id == child_b));
}

#[tokio::test]
async fn a16_removing_a_meta_agent_stops_it_lifts_its_children_then_deletes_it() {
    let f = Fixture::running("sleep 30");
    f.group("before", None).await;
    let team = f.group("team", None).await;
    f.group("after", None).await;
    let mut agents = vec![];
    for _ in 0..2 {
        agents.push(f.spawn(Some(&team), None).await.unwrap().id);
    }
    promote(&f, &team).await;

    f.remove(&team).await.unwrap();

    let tree = f.tree().await;
    assert_eq!(
        names(&tree, None),
        ["before:0", "agent:1", "agent:2", "after:3"]
    );
    assert!(tree.iter().all(|n| n.id != team));
    for id in &agents {
        let child = tree.iter().find(|n| &n.id == id).unwrap();
        assert_eq!(child.parent, None);
        assert_eq!(status_of(&tree, id).kind, Kind::Working);
    }
}

#[tokio::test]
async fn a16_a_terminal_whose_id_a_reopen_cleared_is_still_removed() {
    let f = Fixture::new();
    let node = spawn_terminal(&f, None).await;
    let f = f.reopen();
    assert_eq!(f.tree().await[0].terminal_id, None);

    f.remove(&node.id).await.unwrap();

    assert!(f.tree().await.is_empty());
}

#[tokio::test]
async fn a16_unknown_id_is_not_found() {
    let f = Fixture::new();
    let err = f.remove("999").await.unwrap_err();
    assert_eq!(err.code, code::NOT_FOUND);
}

#[tokio::test]
async fn a16_siblings_after_it_keep_their_order() {
    let f = Fixture::new();
    let a = f.group("a", None).await;
    let b = f.group("b", None).await;
    let c = f.group("c", None).await;

    f.remove(&b).await.unwrap();

    assert_eq!(names(&f.tree().await, None), ["a:0", "c:1"]);
    assert!(f.tree().await.iter().all(|n| n.id != b));
    assert!(f.tree().await.iter().any(|n| n.id == a));
    assert!(f.tree().await.iter().any(|n| n.id == c));
}

#[tokio::test]
async fn a16_remove_emits_rail_changed() {
    let mut f = Fixture::new();
    let group = f.group("g", None).await;
    f.changed();

    f.remove(&group).await.unwrap();

    assert_eq!(f.changed(), 1);
}

#[tokio::test]
async fn a16_a_delete_failure_after_a_successful_stop_leaves_the_node_with_its_exited_agent() {
    let f = Fixture::running("sleep 30");
    let agent = f.spawn(None, None).await.unwrap();
    let lock = lock_db_for_writes(f.dir.path());

    let err = f.remove(&agent.id).await.unwrap_err();

    assert_eq!(err.code, code::INTERNAL);
    let tree = f.tree().await;
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].id, agent.id);
    drop(lock);

    f.remove(&agent.id).await.unwrap();

    assert!(f.tree().await.is_empty());
}

#[tokio::test]
async fn a16_a_groups_failed_delete_leaves_its_children_under_it() {
    let f = Fixture::new();
    let group = f.group("g", None).await;
    let child = f.group("child", Some(&group)).await;
    let lock = lock_db_for_writes(f.dir.path());

    let err = f.remove(&group).await.unwrap_err();

    assert_eq!(err.code, code::INTERNAL);
    let tree = f.tree().await;
    let child_node = tree.iter().find(|n| n.id == child).unwrap();
    assert_eq!(child_node.parent.as_deref(), Some(group.as_str()));
    drop(lock);

    f.remove(&group).await.unwrap();

    assert_eq!(f.tree().await[0].id, child);
}
