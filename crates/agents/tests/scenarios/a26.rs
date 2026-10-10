//! A26: a Workstream is one Door with Agents nested under it; Workstreams never nest.

use contracts::agent::{NodeKind, RailNode};
use rpc::code;
use serde_json::json;

use crate::common::Fixture;

fn shape(tree: &[RailNode]) -> Vec<(String, Option<String>, u32)> {
    tree.iter()
        .map(|n| (n.id.clone(), n.parent.clone(), n.order))
        .collect()
}

#[tokio::test]
async fn a26_create_workstream_with_a_parent_is_conflict_and_changes_nothing() {
    let mut f = Fixture::new();
    let w = f.workstream("w", None).await;
    let agent = f.spawn(Some(&w), None).await.unwrap().id;
    let before = shape(&f.tree().await);
    f.changed();
    for parent in [&w, &agent] {
        let err = f
            .call(
                "rail.createWorkstream",
                json!({"name": "x", "parent": parent}),
            )
            .await
            .unwrap_err();
        assert_eq!(err.code, code::CONFLICT);
    }
    assert_eq!(shape(&f.tree().await), before);
    assert_eq!(f.changed(), 0);
}

#[tokio::test]
async fn a26_move_a_workstream_under_a_workstream_or_agent_is_conflict() {
    let mut f = Fixture::new();
    let a = f.workstream("a", None).await;
    let b = f.workstream("b", None).await;
    let agent = f.spawn(Some(&b), None).await.unwrap().id;
    let before = shape(&f.tree().await);
    f.changed();
    for parent in [&b, &agent] {
        let err = f
            .call("rail.move", json!({"id": a, "parent": parent, "index": 0}))
            .await
            .unwrap_err();
        assert_eq!(err.code, code::CONFLICT);
    }
    assert_eq!(shape(&f.tree().await), before);
    assert_eq!(f.changed(), 0);
}

#[tokio::test]
async fn a26_a_new_workstream_holds_its_door_alone() {
    let f = Fixture::new();
    let w = f.workstream("w", None).await;
    let tree = f.tree().await;
    assert_eq!(tree.len(), 1);
    assert_eq!(
        (tree[0].id.as_str(), tree[0].kind),
        (w.as_str(), NodeKind::Workstream)
    );
}

#[tokio::test]
async fn a26_a_subagent_started_by_a_subagent_sits_three_deep_under_the_door() {
    let f = Fixture::new();
    let d = f.workstream("d", None).await;
    let a = f.spawn_as(&d, None).await.unwrap().id;
    let b = f.spawn_as_agent(&a).await;
    let tree = f.tree().await;
    let ids: Vec<_> = tree.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, [d.as_str(), a.as_str(), b.as_str()]);
    assert_eq!(tree[1].parent.as_deref(), Some(d.as_str()));
    assert_eq!(tree[2].parent.as_deref(), Some(a.as_str()));
}

#[tokio::test]
async fn a26_an_agent_parent_param_from_an_agent_is_ignored() {
    let f = Fixture::new();
    let d = f.workstream("d", None).await;
    let other = f.workstream("other", None).await;
    let a = f.spawn_as(&d, None).await.unwrap().id;
    let b = f.spawn_as(&a, Some(&other)).await.unwrap();
    assert_eq!(b.parent.as_deref(), Some(a.as_str()));
}

#[tokio::test]
async fn a26_move_an_agent_under_an_agent_is_conflict() {
    let mut f = Fixture::new();
    let w = f.workstream("w", None).await;
    let a = f.spawn(Some(&w), None).await.unwrap().id;
    let b = f.spawn(Some(&w), None).await.unwrap().id;
    let before = shape(&f.tree().await);
    f.changed();
    let err = f
        .call("rail.move", json!({"id": b, "parent": a, "index": 0}))
        .await
        .unwrap_err();
    assert_eq!(err.code, code::CONFLICT);
    assert_eq!(shape(&f.tree().await), before);
    assert_eq!(f.changed(), 0);
}

/// A Rail an earlier Daemon stored: `w2` (with an Agent) under `w1`, `w3` under `w2`, and the
/// roots `r0`, `r1`; `w1` holds a second child so a gap shows.
fn store_nested(f: &Fixture) {
    let db = rusqlite::Connection::open(f.dir.path().join("agents.db")).unwrap();
    db.execute_batch(
        "DELETE FROM nodes;
         INSERT INTO nodes (id, kind, name, parent, ord) VALUES
           (1, 'workstream', 'w1', NULL, 0),
           (2, 'workstream', 'w2', 1, 0),
           (3, 'agent', 'k', 1, 1),
           (4, 'workstream', 'w3', 2, 1),
           (5, 'agent', 'in2', 2, 0),
           (6, 'workstream', 'r1', NULL, 1);",
    )
    .unwrap();
}

fn stored(f: &Fixture) -> Vec<(i64, Option<i64>, i64, String)> {
    let db = rusqlite::Connection::open(f.dir.path().join("agents.db")).unwrap();
    let mut query = db
        .prepare("SELECT id, parent, ord, kind FROM nodes ORDER BY id")
        .unwrap();
    query
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[tokio::test]
async fn a26_a_stored_nested_workstream_loads_at_the_root_with_its_door_and_agents() {
    let f = Fixture::new();
    store_nested(&f);
    let f = f.reopen();
    let tree = f.tree().await;
    let at = |id: &str| tree.iter().find(|n| n.id == id).unwrap();
    // After the last root (`r1`, order 1): depth 1 `w2`, then depth 2 `w3`.
    assert_eq!((at("2").parent.clone(), at("2").order), (None, 2));
    assert_eq!((at("4").parent.clone(), at("4").order), (None, 3));
    assert_eq!(at("2").name, "w2");
    assert_eq!(at("5").parent.as_deref(), Some("2"));
    assert_eq!(at("3").parent.as_deref(), Some("1"));
}

#[tokio::test]
async fn a26_the_load_keeps_ids_orders_contiguous_and_starts_no_program() {
    let f = Fixture::new();
    store_nested(&f);
    let f = f.reopen();
    let tree = f.tree().await;
    assert_eq!(tree.len(), 6);
    assert!(tree.iter().all(|n| n.terminal_id.is_none()));
    // `k` was order 1 under `w1` and closes up to 0; `in2` keeps 0 under `w2`.
    let order = |id: &str| tree.iter().find(|n| n.id == id).unwrap().order;
    assert_eq!((order("3"), order("5")), (0, 0));
    let mut roots: Vec<_> = tree
        .iter()
        .filter(|n| n.parent.is_none())
        .map(|n| n.order)
        .collect();
    roots.sort_unstable();
    assert_eq!(roots, [0, 1, 2, 3]);
}

#[tokio::test]
async fn a26_the_next_write_stores_the_flat_tree() {
    let f = Fixture::new();
    store_nested(&f);
    let f = f.reopen();
    f.workstream("new", None).await;
    let rows = stored(&f);
    assert!(
        rows.iter()
            .filter(|(.., kind)| kind == "workstream")
            .all(|(_, parent, ..)| parent.is_none())
    );
}
