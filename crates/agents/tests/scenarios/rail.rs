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

#[tokio::test]
async fn a6_new_rooms_are_appended_with_contiguous_order() {
    let mut f = Fixture::new();
    let a = f.room("a", None).await;
    f.room("b", None).await;
    f.room("inner", Some(&a)).await;
    let tree = f.tree().await;
    assert_eq!(children(&tree, None), ["a:0", "b:1"]);
    assert_eq!(children(&tree, Some(&a)), ["inner:0"]);
    assert!(tree.iter().all(|n| n.kind == NodeKind::Room));
    assert_eq!(f.changed(), 3);
}

#[tokio::test]
async fn a6_move_reorders_and_keeps_orders_contiguous_from_zero() {
    let mut f = Fixture::new();
    let a = f.room("a", None).await;
    let b = f.room("b", None).await;
    let c = f.room("c", None).await;
    f.changed();

    assert_eq!(f.mv(&c, None, 0).await.unwrap(), Value::Null);
    assert_eq!(children(&f.tree().await, None), ["c:0", "a:1", "b:2"]);

    f.mv(&a, Some(&b), 5).await.unwrap();
    let tree = f.tree().await;
    assert_eq!(children(&tree, None), ["c:0", "b:1"]);
    assert_eq!(children(&tree, Some(&b)), ["a:0"]);
    assert_eq!(f.changed(), 2);

    f.mv(&a, None, 0).await.unwrap();
    assert_eq!(children(&f.tree().await, None), ["a:0", "c:1", "b:2"]);
}

#[tokio::test]
async fn a6_a_room_cannot_move_into_its_own_descendant() {
    let mut f = Fixture::new();
    let a = f.room("a", None).await;
    let b = f.room("b", Some(&a)).await;
    let c = f.room("c", Some(&b)).await;
    f.changed();
    for target in [&a, &b, &c] {
        let err = f.mv(&a, Some(target), 0).await.unwrap_err();
        assert_eq!(err.code, code::CONFLICT);
    }
    assert_eq!(f.changed(), 0);
}

#[tokio::test]
async fn a6_rename_changes_the_name_and_announces_it() {
    let mut f = Fixture::new();
    let a = f.room("a", None).await;
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
        .call("rail.createRoom", json!({"name": "x", "parent": "999"}))
        .await
        .unwrap_err();
    assert_eq!(err.code, code::NOT_FOUND);
}

#[tokio::test]
async fn a6_the_tree_lists_parents_before_their_children() {
    let f = Fixture::new();
    let a = f.room("a", None).await;
    let b = f.room("b", None).await;
    f.room("a1", Some(&a)).await;
    f.room("b1", Some(&b)).await;
    f.mv(&b, None, 0).await.unwrap();
    let names: Vec<_> = f.tree().await.into_iter().map(|n| n.name).collect();
    assert_eq!(names, ["b", "b1", "a", "a1"]);
}

#[tokio::test]
async fn a8_rooms_names_parents_and_order_survive_reopening() {
    let f = Fixture::new();
    let a = f.room("a", None).await;
    let b = f.room("b", None).await;
    f.room("inner", Some(&a)).await;
    f.mv(&b, None, 0).await.unwrap();
    let mut before = f.tree().await;
    let mut after = f.reopen().tree().await;
    for node in before.iter_mut().chain(after.iter_mut()) {
        node.status.as_mut().unwrap().since = 0;
    }
    assert_eq!(after, before);
}

#[tokio::test]
async fn a7_room_restart_preserves_membership_and_fences_old_signals() {
    let f = Fixture::running("exec sleep 30");
    let room = f
        .call("rail.createRoom", json!({"name":"room","parent":null}))
        .await
        .unwrap();
    let id = room["id"].as_str().unwrap();
    assert_eq!(room["kind"], "room");
    assert_eq!(room["attempt"], Value::Null);
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
    assert_eq!(tree.iter().find(|n| n.id == id).unwrap().name, "room");
    for target in [id, child.id.as_str()] {
        f.call("agent.stop", json!({"id":target})).await.unwrap();
    }
}

#[tokio::test]
async fn a8_a12_reopen_preserves_ordinal_with_reused_terminal_numbers() {
    let f = Fixture::running("exec sleep 30");
    let room = f
        .call("rail.createRoom", json!({"name":"room","parent":null}))
        .await
        .unwrap();
    let id = room["id"].as_str().unwrap().to_owned();
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
    let room = f
        .call("rail.createRoom", json!({"name":"room","parent":null}))
        .await
        .unwrap();
    let id = room["id"].as_str().unwrap();
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
