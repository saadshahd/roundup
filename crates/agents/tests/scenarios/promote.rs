//! A7 promote and stop, and A8 for Meta-agents.

use contracts::Kind;
use contracts::agent::{NodeKind, RailNode};
use rpc::code;
use serde_json::{Value, json};

use crate::common::{Fixture, status_of};

impl Fixture {
    async fn promote(&self, id: &str) -> Result<RailNode, rpc::RpcError> {
        let node = self.call("rail.promote", json!({"id": id})).await?;
        Ok(serde_json::from_value(node).unwrap())
    }

    async fn stop(&self, id: &str) -> Result<Value, rpc::RpcError> {
        self.call("agent.stop", json!({"id": id})).await
    }
}

/// `name:order` of each node under `parent`, in order; Agents are named after their folder, so `agent`.
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

/// A Group `team` between two other Groups, holding two running Agents.
async fn team(f: &Fixture) -> (String, Vec<String>) {
    f.group("before", None).await;
    let team = f.group("team", None).await;
    f.group("after", None).await;
    let mut agents = vec![];
    for _ in 0..2 {
        agents.push(f.spawn(Some(&team), None).await.unwrap().id);
    }
    (team, agents)
}

#[tokio::test]
async fn a7_promote_makes_a_group_a_meta_agent_with_a_live_agent() {
    let mut f = Fixture::running("sleep 30");
    let (team, agents) = team(&f).await;
    f.changed();

    let node = f.promote(&team).await.unwrap();

    assert_eq!((node.kind, node.meta), (NodeKind::Group, true));
    assert!(node.terminal_id.is_some());
    assert_eq!(node.status.as_ref().map(|s| s.kind), Some(Kind::Working));
    let tree = f.tree().await;
    assert_eq!(tree.iter().find(|n| n.id == team), Some(&node));
    assert!(agents.iter().all(|id| {
        tree.iter()
            .any(|n| &n.id == id && n.parent.as_deref() == Some(team.as_str()))
    }));
    assert_eq!(f.changed(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a7_two_concurrent_promotes_start_one_agent_and_one_loses() {
    let f = std::sync::Arc::new(Fixture::running(
        "echo started >> \"$(dirname \"$0\")/starts\"; sleep 30",
    ));
    let team = f.group("team", None).await;
    let promote = || {
        let (f, team) = (std::sync::Arc::clone(&f), team.clone());
        tokio::spawn(async move { f.promote(&team).await })
    };

    let (first, second) = (promote(), promote());

    let mut codes: Vec<_> = [first.await.unwrap(), second.await.unwrap()]
        .map(|r| r.map_err(|e| e.code))
        .into();
    codes.sort_by_key(|r| r.is_err());
    assert_eq!(codes.len(), 2);
    assert!(codes[0].is_ok());
    assert_eq!(codes[1].as_ref().unwrap_err(), &code::CONFLICT);
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let starts = std::fs::read_to_string(f.dir.path().join("starts")).unwrap();
    assert_eq!(starts.lines().count(), 1);
}

#[tokio::test]
async fn a7_a_promote_whose_agent_cannot_start_leaves_the_group_plain() {
    let f = Fixture::running("sleep 30");
    let team = f.group("team", None).await;
    let rup = f.dir.path().join("rup");
    std::fs::remove_file(&rup).unwrap();

    assert!(f.promote(&team).await.is_err());
    assert!(!f.tree().await[0].meta);

    std::fs::write(&rup, "").unwrap();
    assert!(f.promote(&team).await.unwrap().meta);
}

#[tokio::test]
async fn a7_only_a_plain_group_can_be_promoted() {
    let f = Fixture::running("sleep 30");
    let (team, agents) = team(&f).await;
    f.promote(&team).await.unwrap();

    let again = f.promote(&team).await.unwrap_err();
    let agent = f.promote(&agents[0]).await.unwrap_err();
    let unknown = f.promote("999").await.unwrap_err();
    assert_eq!(
        (again.code, agent.code, unknown.code),
        (code::CONFLICT, code::CONFLICT, code::NOT_FOUND)
    );
}

#[tokio::test]
async fn a7_stopping_a_meta_agent_lifts_its_children_where_it_was_and_they_keep_running() {
    let mut f = Fixture::running("sleep 30");
    let (team, agents) = team(&f).await;
    f.promote(&team).await.unwrap();
    f.changed();

    f.stop(&team).await.unwrap();

    let tree = f.until(|t| status_of(t, &team).kind == Kind::Done).await;
    assert_eq!(
        names(&tree, None),
        ["before:0", "agent:1", "agent:2", "team:3", "after:4"]
    );
    let meta = tree.iter().find(|n| n.id == team).unwrap();
    assert!(meta.meta);
    for id in &agents {
        let child = tree.iter().find(|n| &n.id == id).unwrap();
        assert_eq!(child.parent, None);
        assert_eq!(status_of(&tree, id).kind, Kind::Working);
    }
    assert_eq!(f.changed(), 1);
    for id in &agents {
        let terminal = tree
            .iter()
            .find(|n| &n.id == id)
            .unwrap()
            .terminal_id
            .clone();
        let alive = f
            .terminals
            .list()
            .into_iter()
            .any(|t| Some(t.id) == terminal && t.running);
        assert!(alive, "the kill reached a child's Terminal");
    }
}

#[tokio::test]
async fn a7_a_stopped_agent_is_done_not_an_error() {
    let f = Fixture::running("sleep 30");
    let agent = f.spawn(None, None).await.unwrap();

    f.stop(&agent.id).await.unwrap();

    let tree = f
        .until(|t| status_of(t, &agent.id).kind != Kind::Working)
        .await;
    assert_eq!(status_of(&tree, &agent.id).kind, Kind::Done);
    // The kill's exit must not turn it into an error afterwards.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert_eq!(status_of(&f.tree().await, &agent.id).kind, Kind::Done);
}

#[tokio::test]
async fn a7_stop_only_applies_to_agents_and_meta_agents() {
    let f = Fixture::new();
    let group = f.group("plain", None).await;
    assert_eq!(f.stop(&group).await.unwrap_err().code, code::CONFLICT);
    assert_eq!(f.stop("999").await.unwrap_err().code, code::NOT_FOUND);
}

#[tokio::test]
async fn a8_a_meta_agent_keeps_its_flag_across_reopening_and_comes_back_done() {
    let f = Fixture::running("sleep 30");
    let (team, _) = team(&f).await;
    let promoted = f.promote(&team).await.unwrap();

    let tree = f.reopen().tree().await;

    let meta = tree.iter().find(|n| n.id == team).unwrap();
    assert!(meta.meta);
    assert_eq!(meta.terminal_id, promoted.terminal_id);
    assert_eq!(meta.status.as_ref().map(|s| s.kind), Some(Kind::Done));
}
