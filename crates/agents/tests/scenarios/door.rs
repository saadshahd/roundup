//! A7 start_door and stop, and A8 for Doors.

use contracts::Kind;
use contracts::agent::{NodeKind, RailNode};
use rpc::code;
use serde_json::{Value, json};

use crate::common::{Fixture, hold_starts, release, status_of, until_file};

impl Fixture {
    async fn start_door(&self, id: &str) -> Result<RailNode, rpc::RpcError> {
        let node = self.call("rail.startDoor", json!({"id": id})).await?;
        Ok(serde_json::from_value(node).unwrap())
    }

    async fn stop(&self, id: &str) -> Result<Value, rpc::RpcError> {
        self.call("agent.stop", json!({"id": id})).await
    }
}

/// `name:order` of each node under `parent`, in order; an Agent shows as `agent` whatever its name.
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

/// A Room `team` between two other Rooms, holding two running Agents.
async fn team(f: &Fixture) -> (String, Vec<String>) {
    f.room("before", None).await;
    let team = f.room("team", None).await;
    f.room("after", None).await;
    let mut agents = vec![];
    for _ in 0..2 {
        agents.push(f.spawn(Some(&team), None).await.unwrap().id);
    }
    (team, agents)
}

#[tokio::test]
async fn a7_start_door_makes_a_room_a_door_with_a_live_agent() {
    let mut f = Fixture::running("sleep 30");
    let (team, agents) = team(&f).await;
    f.changed();

    let node = f.start_door(&team).await.unwrap();

    assert_eq!(node.kind, NodeKind::Room);
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
async fn a7_two_concurrent_start_doors_start_one_agent_and_one_loses() {
    let f = std::sync::Arc::new(Fixture::running(
        "echo started >> \"$(dirname \"$0\")/starts\"; sleep 30",
    ));
    let team = f.room("team", None).await;
    let start_door = || {
        let (f, team) = (std::sync::Arc::clone(&f), team.clone());
        tokio::spawn(async move { f.start_door(&team).await })
    };

    let (first, second) = (start_door(), start_door());

    let mut codes: Vec<_> = [first.await.unwrap(), second.await.unwrap()]
        .map(|r| r.map_err(|e| e.code))
        .into();
    codes.sort_by_key(|r| r.is_err());
    assert_eq!(codes.len(), 2);
    assert!(codes[0].is_ok());
    assert_eq!(codes[1].as_ref().unwrap_err(), &code::CONFLICT);
    let starts = until_file(&f.dir.path().join("starts")).await;
    assert_eq!(starts.lines().count(), 1);
}

#[tokio::test]
async fn a7_a_start_door_whose_agent_cannot_start_leaves_the_room_plain() {
    let f = Fixture::running("sleep 30");
    let team = f.room("team", None).await;
    let rup = f.dir.path().join("rup");
    std::fs::remove_file(&rup).unwrap();

    assert!(f.start_door(&team).await.is_err());
    assert!(f.tree().await[0].terminal_id.is_none());

    std::fs::write(&rup, "").unwrap();
    assert_eq!(f.start_door(&team).await.unwrap().kind, NodeKind::Room);
}

#[tokio::test]
async fn a7_a_start_door_whose_claude_is_missing_leaves_no_settings_file() {
    let f = Fixture::running("sleep 30");
    let team = f.room("team", None).await;
    std::fs::remove_file(f.dir.path().join("fake-claude")).unwrap();

    assert!(f.start_door(&team).await.is_err());

    let left: Vec<_> = std::fs::read_dir(f.dir.path().join("agents"))
        .map(|entries| entries.map(|e| e.unwrap().file_name()).collect())
        .unwrap_or_default();
    assert!(left.is_empty(), "left behind: {left:?}");
}

#[tokio::test]
async fn a7_only_a_stopped_room_can_start_its_door() {
    let f = Fixture::running("sleep 30");
    let (team, agents) = team(&f).await;
    f.start_door(&team).await.unwrap();

    let again = f.start_door(&team).await.unwrap_err();
    let agent = f.start_door(&agents[0]).await.unwrap_err();
    let unknown = f.start_door("999").await.unwrap_err();
    assert_eq!(
        (again.code, agent.code, unknown.code),
        (code::CONFLICT, code::CONFLICT, code::NOT_FOUND)
    );
}

#[tokio::test]
async fn a7_stopping_a_door_keeps_its_children_in_the_room_and_running() {
    let mut f = Fixture::running("sleep 30");
    let (team, agents) = team(&f).await;
    f.start_door(&team).await.unwrap();
    f.changed();

    f.stop(&team).await.unwrap();

    let tree = f.until(|t| status_of(t, &team).kind == Kind::Done).await;
    assert_eq!(names(&tree, None), ["before:0", "team:1", "after:2"]);
    let meta = tree.iter().find(|n| n.id == team).unwrap();
    assert_eq!(meta.kind, NodeKind::Room);
    for id in &agents {
        let child = tree.iter().find(|n| &n.id == id).unwrap();
        assert_eq!(child.parent.as_deref(), Some(team.as_str()));
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a7_stopping_a_door_that_is_still_starting_is_a_conflict_and_moves_nothing() {
    let f = std::sync::Arc::new(Fixture::running("sleep 30"));
    let (team, agents) = team(&f).await;
    let config = hold_starts(&f);
    let promoting = {
        let (f, team) = (std::sync::Arc::clone(&f), team.clone());
        tokio::spawn(async move { f.start_door(&team).await })
    };
    f.until(|t| t.iter().any(|n| n.id == team && n.incarnation.is_some()))
        .await;

    let stopped = f.stop(&team).await.unwrap_err();

    assert_eq!(stopped.code, code::CONFLICT);
    release(config).await;
    promoting.await.unwrap().unwrap();
    let tree = f.tree().await;
    assert_eq!(status_of(&tree, &team).kind, Kind::Working);
    assert!(agents.iter().all(|id| {
        tree.iter()
            .any(|n| &n.id == id && n.parent.as_deref() == Some(team.as_str()))
    }));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a7_a_room_whose_door_is_starting_reads_working_starting_not_done() {
    let f = std::sync::Arc::new(Fixture::running("sleep 30"));
    let team = f.room("team", None).await;
    let config = hold_starts(&f);
    let promoting = {
        let (f, team) = (std::sync::Arc::clone(&f), team.clone());
        tokio::spawn(async move { f.start_door(&team).await })
    };
    let tree = f
        .until(|t| t.iter().any(|n| n.id == team && n.incarnation.is_some()))
        .await;

    let status = status_of(&tree, &team);

    assert_eq!(
        (status.kind, status.label.as_str()),
        (Kind::Working, "starting")
    );
    release(config).await;
    promoting.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a7_an_agent_that_is_still_starting_reads_working_starting_not_done() {
    let f = std::sync::Arc::new(Fixture::running("sleep 30"));
    let config = hold_starts(&f);
    let spawning = {
        let f = std::sync::Arc::clone(&f);
        tokio::spawn(async move { f.spawn(None, None).await })
    };
    let tree = f.until(|t| !t.is_empty()).await;

    let status = status_of(&tree, &tree[0].id);

    assert_eq!(
        (status.kind, status.label.as_str()),
        (Kind::Working, "starting")
    );
    assert!(status.since > 0, "since is when it was marked");
    release(config).await;
    spawning.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a7_stopping_an_agent_that_is_still_starting_is_a_conflict() {
    let f = std::sync::Arc::new(Fixture::running("sleep 30"));
    let config = hold_starts(&f);
    let spawning = {
        let f = std::sync::Arc::clone(&f);
        tokio::spawn(async move { f.spawn(None, None).await })
    };
    let tree = f.until(|t| !t.is_empty()).await;

    let stopped = f.stop(&tree[0].id).await.unwrap_err();

    assert_eq!(stopped.code, code::CONFLICT);
    release(config).await;
    let agent = spawning.await.unwrap().unwrap();
    assert_eq!(status_of(&f.tree().await, &agent.id).kind, Kind::Working);
}

#[tokio::test]
async fn a7_stopping_an_agent_whose_program_already_failed_keeps_its_error() {
    let f = Fixture::running("exit 3");
    let agent = f.spawn(None, None).await.unwrap();
    f.until(|t| status_of(t, &agent.id).kind == Kind::Error)
        .await;

    f.stop(&agent.id).await.unwrap();

    let status = status_of(&f.tree().await, &agent.id).clone();
    assert_eq!(
        (status.kind, status.label.as_str()),
        (Kind::Error, "exited 3")
    );
}

#[tokio::test]
async fn a7_stopping_a_door_twice_announces_the_move_once() {
    let mut f = Fixture::running("sleep 30");
    let (team, _) = team(&f).await;
    f.start_door(&team).await.unwrap();
    f.stop(&team).await.unwrap();
    f.changed();

    f.stop(&team).await.unwrap();

    assert_eq!(f.changed(), 0);
}

#[tokio::test]
async fn a7_start_door_names_a_node_by_its_exact_id() {
    let f = Fixture::running("sleep 30");
    let team = f.room("team", None).await;

    let err = f.start_door(&format!("0{team}")).await.unwrap_err();

    assert_eq!(err.code, code::NOT_FOUND);
    assert!(f.tree().await[0].terminal_id.is_none());
    assert_eq!(f.start_door(&team).await.unwrap().kind, NodeKind::Room);
}

#[tokio::test]
async fn a7_stop_only_applies_to_agents_and_doors() {
    let f = Fixture::new();
    let room = f.room("plain", None).await;
    assert_eq!(f.stop(&room).await.unwrap(), Value::Null);
    assert_eq!(f.stop("999").await.unwrap_err().code, code::NOT_FOUND);
}

#[tokio::test]
async fn a8_a_door_keeps_its_flag_across_reopening_and_comes_back_done() {
    let f = Fixture::running("sleep 30");
    let (team, _) = team(&f).await;
    f.start_door(&team).await.unwrap();

    let tree = f.reopen().tree().await;

    let meta = tree.iter().find(|n| n.id == team).unwrap();
    assert_eq!(meta.kind, NodeKind::Room);
    assert_eq!(meta.terminal_id, None);
    assert_eq!(meta.status.as_ref().map(|s| s.kind), Some(Kind::Done));
}

#[tokio::test]
async fn a7_stopping_an_agent_an_earlier_daemon_ran_leaves_it_done() {
    let f = Fixture::running("sleep 30");
    let agent = f.spawn(None, None).await.unwrap();
    let f = f.reopen();

    f.stop(&agent.id).await.unwrap();

    assert_eq!(status_of(&f.tree().await, &agent.id).kind, Kind::Done);
}

#[tokio::test]
async fn a7_stopping_a_door_an_earlier_daemon_ran_keeps_its_children() {
    let f = Fixture::running("sleep 30");
    let (team, _) = team(&f).await;
    f.start_door(&team).await.unwrap();
    let f = f.reopen();

    f.stop(&team).await.unwrap();

    assert_eq!(
        names(&f.tree().await, None),
        ["before:0", "team:1", "after:2"]
    );
}
