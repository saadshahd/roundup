//! A6 (rail tree) and A8 (persistence) through the module's RPC surface.

use contracts::agent::{NodeKind, RailNode};
use rpc::code;
use serde_json::{Value, json};

use crate::common::Fixture;

impl Fixture {
    async fn mv(&self, id: &str, parent: Option<&str>, index: u32) -> Result<Value, rpc::RpcError> {
        self.call(
            "rail.move",
            json!({"id": id, "parent": parent, "index": index}),
        )
        .await
    }
}

/// `name:order` of the children of `parent`, in tree order.
fn children(tree: &[RailNode], parent: Option<&str>) -> Vec<String> {
    tree.iter()
        .filter(|node| node.parent.as_deref() == parent)
        .map(|node| format!("{}:{}", node.name, node.order))
        .collect()
}

/// What the Rail keeps across a reopen: a running Agent's Status is not kept.
fn shape(tree: &[RailNode]) -> Vec<(String, NodeKind, String, Option<String>, u32)> {
    tree.iter()
        .map(|n| {
            (
                n.id.clone(),
                n.kind,
                n.name.clone(),
                n.parent.clone(),
                n.order,
            )
        })
        .collect()
}

#[tokio::test]
async fn a6_new_workstreams_are_appended_with_contiguous_order() {
    let mut f = Fixture::new();
    let a = f.workstream("a", None).await;
    f.workstream("b", None).await;
    f.spawn(Some(&a), None).await.unwrap();
    let tree = f.tree().await;
    assert_eq!(children(&tree, None), ["a:0", "b:1"]);
    assert_eq!(children(&tree, Some(&a)), ["new-agent:0"]);
    assert_eq!(f.changed(), 3);
}

#[tokio::test]
async fn a6_move_reorders_and_keeps_orders_contiguous_from_zero() {
    let mut f = Fixture::new();
    let a = f.workstream("a", None).await;
    let b = f.workstream("b", None).await;
    let c = f.workstream("c", None).await;
    f.changed();

    assert_eq!(f.mv(&c, None, 0).await.unwrap(), Value::Null);
    assert_eq!(children(&f.tree().await, None), ["c:0", "a:1", "b:2"]);

    let x = f.spawn(Some(&b), None).await.unwrap().id;
    f.changed();
    f.mv(&x, Some(&c), 5).await.unwrap();
    let tree = f.tree().await;
    assert_eq!(children(&tree, Some(&b)), Vec::<String>::new());
    assert_eq!(children(&tree, Some(&c)), ["new-agent:0"]);
    assert_eq!(f.changed(), 1);

    f.mv(&a, None, 2).await.unwrap();
    assert_eq!(children(&f.tree().await, None), ["c:0", "b:1", "a:2"]);
}

#[tokio::test]
async fn a6_an_agent_cannot_move_into_its_own_descendant() {
    let mut f = Fixture::new();
    let w = f.workstream("w", None).await;
    let a = f.spawn(Some(&w), None).await.unwrap().id;
    let b = f.spawn_as_agent(&a).await;
    f.changed();
    for target in [&a, &b] {
        let err = f.mv(&a, Some(target), 0).await.unwrap_err();
        assert_eq!(err.code, code::CONFLICT);
    }
    assert_eq!(f.changed(), 0);
}

#[tokio::test]
async fn a6_rename_changes_the_name_and_announces_it() {
    let mut f = Fixture::new();
    let a = f.workstream("a", None).await;
    f.changed();
    f.call("rail.rename", json!({"id": a, "name": "renamed"}))
        .await
        .unwrap();
    assert_eq!(children(&f.tree().await, None), ["renamed:0"]);
    assert_eq!(f.changed(), 1);
}

#[tokio::test]
async fn a6_unknown_nodes_are_not_found() {
    let f = Fixture::new();
    let err = f.mv("999", None, 0).await.unwrap_err();
    assert_eq!(err.code, code::NOT_FOUND);
    let err = f
        .call(
            "rail.createWorkstream",
            json!({"name": "x", "parent": "999"}),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, code::NOT_FOUND);
}

#[tokio::test]
async fn a6_the_tree_lists_parents_before_their_children() {
    let f = Fixture::new();
    let a = f.workstream("a", None).await;
    let b = f.workstream("b", None).await;
    f.spawn(Some(&a), None).await.unwrap();
    f.spawn(Some(&b), None).await.unwrap();
    f.mv(&b, None, 0).await.unwrap();
    let names: Vec<_> = f.tree().await.into_iter().map(|n| n.name).collect();
    assert_eq!(names, ["b", "new-agent", "a", "new-agent"]);
}

#[tokio::test]
async fn a8_workstreams_names_parents_and_order_survive_reopening() {
    let f = Fixture::new();
    let a = f.workstream("a", None).await;
    let b = f.workstream("b", None).await;
    f.spawn(Some(&a), None).await.unwrap();
    f.mv(&b, None, 0).await.unwrap();
    let before = shape(&f.tree().await);
    let after = shape(&f.reopen().tree().await);
    assert_eq!(after, before);
}

#[tokio::test]
async fn a7_workstream_restart_preserves_membership_and_fences_old_signals() {
    let f = Fixture::running("exec sleep 30");
    let workstream = f
        .call(
            "rail.createWorkstream",
            json!({"name":"workstream","parent":null}),
        )
        .await
        .unwrap();
    let id = workstream["id"].as_str().unwrap();
    assert_eq!(workstream["kind"], "workstream");
    assert_eq!(workstream["attempt"], Value::Null);
    let child = f.spawn(Some(id), None).await.unwrap();
    let first = f.call("rail.startDoor", json!({"id":id})).await.unwrap();
    assert_eq!(first["attempt"], "1");
    let live = f
        .call("rail.startDoor", json!({"id":id}))
        .await
        .unwrap_err();
    assert_eq!(live.code, code::CONFLICT);
    f.call("agent.stop", json!({"id":id})).await.unwrap();
    let second = f.call("rail.startDoor", json!({"id":id})).await.unwrap();
    assert_eq!(second["attempt"], "2");
    assert_ne!(first["terminal_id"], second["terminal_id"]);
    // A20: an earlier Attempt's Signal is ignored, never an error to the caller.
    let stale = f.call("agent.signal", json!({"id":id,"attempt":"1","payload":{"hook_event_name":"UserPromptSubmit","prompt":"wrong name"}})).await.unwrap();
    assert_eq!(stale, Value::Null);
    let tree = f.tree().await;
    assert_eq!(
        tree.iter()
            .find(|n| n.id == child.id)
            .unwrap()
            .parent
            .as_deref(),
        Some(id)
    );
    assert_eq!(tree.iter().find(|n| n.id == id).unwrap().name, "workstream");
    for target in [id, child.id.as_str()] {
        f.call("agent.stop", json!({"id":target})).await.unwrap();
    }
}

#[tokio::test]
async fn a8_a12_reopen_preserves_ordinal_with_reused_terminal_numbers() {
    let f = Fixture::running("exec sleep 30");
    let workstream = f
        .call(
            "rail.createWorkstream",
            json!({"name":"workstream","parent":null}),
        )
        .await
        .unwrap();
    let id = workstream["id"].as_str().unwrap().to_owned();
    let first = f.call("rail.startDoor", json!({"id":id})).await.unwrap();
    f.call("agent.stop", json!({"id":id})).await.unwrap();
    let f = f.reopen();
    assert!(f.terminals.list().is_empty());
    let node = &f.tree().await[0];
    assert_eq!(node.terminal_id, None);
    assert_eq!(node.attempt.as_deref(), Some("1"));
    assert_eq!(node.status_revision, None);
    let next = f.call("rail.startDoor", json!({"id":id})).await.unwrap();
    assert_eq!(first["terminal_id"], next["terminal_id"]);
    assert_eq!(next["attempt"], "2");
    // A20: the earlier Attempt is ignored; a malformed one is refused.
    let earlier = f
        .call("agent.signal", json!({"id":id,"attempt":"1","payload":{}}))
        .await
        .unwrap();
    assert_eq!(earlier, Value::Null);
    for (stamp, expected) in [
        ("", code::INVALID_PARAMS),
        ("01", code::INVALID_PARAMS),
        ("+2", code::INVALID_PARAMS),
        ("0", code::INVALID_PARAMS),
        ("9223372036854775808", code::INVALID_PARAMS),
    ] {
        let err = f
            .call(
                "agent.signal",
                json!({"id":id,"attempt":stamp,"payload":{}}),
            )
            .await
            .unwrap_err();
        assert_eq!(err.code, expected, "{stamp}");
    }
    let missing = f
        .call("agent.signal", json!({"id":id,"payload":{}}))
        .await
        .unwrap_err();
    assert_eq!(missing.code, code::INVALID_PARAMS);
    f.call(
        "agent.signal",
        json!({"id":id,"attempt":"2","payload":{"hook_event_name":"Stop"}}),
    )
    .await
    .unwrap();
    f.call("agent.stop", json!({"id":id})).await.unwrap();
}

#[tokio::test]
async fn a7_failed_door_attempt_consumes_attempt_and_retry_keeps_children() {
    let f = Fixture::running("exec sleep 30");
    let workstream = f
        .call(
            "rail.createWorkstream",
            json!({"name":"workstream","parent":null}),
        )
        .await
        .unwrap();
    let id = workstream["id"].as_str().unwrap();
    let child = f.spawn(Some(id), None).await.unwrap();
    std::fs::remove_file(f.dir.path().join("rup")).unwrap();
    assert!(f.call("rail.startDoor", json!({"id":id})).await.is_err());
    let tree = f.tree().await;
    let stopped = tree.iter().find(|n| n.id == id).unwrap();
    assert_eq!(stopped.attempt.as_deref(), Some("1"));
    assert_eq!(stopped.terminal_id, None);
    assert_eq!(
        tree.iter()
            .find(|n| n.id == child.id)
            .unwrap()
            .parent
            .as_deref(),
        Some(id)
    );
    std::fs::write(f.dir.path().join("rup"), "").unwrap();
    let next = f.call("rail.startDoor", json!({"id":id})).await.unwrap();
    assert_eq!(next["attempt"], "2");
    f.call("agent.stop", json!({"id":id})).await.unwrap();
    f.call("agent.stop", json!({"id":child.id})).await.unwrap();
}

#[tokio::test]
async fn a25_create_workstream_returns_a_node_of_kind_workstream() {
    let f = Fixture::new();
    let node = f
        .call(
            "rail.createWorkstream",
            json!({"name":"workstream","parent":null}),
        )
        .await
        .unwrap();
    assert_eq!(node["kind"], "workstream");
    assert!(
        f.tree()
            .await
            .iter()
            .all(|n| n.kind == NodeKind::Workstream)
    );
}

#[tokio::test]
async fn a25_the_old_method_and_kind_do_not_exist() {
    let f = Fixture::new();
    let err = f
        .call("rail.createRoom", json!({"name":"room","parent":null}))
        .await
        .unwrap_err();
    assert_eq!(err.code, code::METHOD_NOT_FOUND);
    f.workstream("w", None).await;
    let tree = serde_json::to_string(&f.tree().await).unwrap();
    assert!(!tree.contains("\"room\""));
}

/// Writes `kind = 'room'` over every Workstream row, as an earlier Daemon stored it.
fn store_as_room(f: &Fixture) {
    rusqlite::Connection::open(f.dir.path().join("agents.db"))
        .unwrap()
        .execute(
            "UPDATE nodes SET kind = 'room' WHERE kind = 'workstream'",
            [],
        )
        .unwrap();
}

#[tokio::test]
async fn a25_a_stored_rail_with_kind_room_opens_as_workstreams_with_ids_and_children_intact() {
    let f = Fixture::new();
    let a = f.workstream("a", None).await;
    f.spawn(Some(&a), None).await.unwrap();
    f.workstream("b", None).await;
    let before = shape(&f.tree().await);
    store_as_room(&f);
    let after = f.reopen().tree().await;
    assert_eq!(shape(&after), before);
    assert!(
        after
            .iter()
            .filter(|n| n.kind != NodeKind::Agent)
            .all(|n| n.kind == NodeKind::Workstream)
    );
}

#[tokio::test]
async fn a25_the_next_write_stores_workstream() {
    let f = Fixture::new();
    f.workstream("a", None).await;
    store_as_room(&f);
    let f = f.reopen();
    f.workstream("b", None).await;
    let kinds: Vec<String> = rusqlite::Connection::open(f.dir.path().join("agents.db"))
        .unwrap()
        .prepare("SELECT kind FROM nodes")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(kinds, ["workstream", "workstream"]);
}
