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
async fn a6_new_groups_are_appended_with_contiguous_order() {
    let mut f = Fixture::new();
    let a = f.group("a", None).await;
    f.group("b", None).await;
    f.group("inner", Some(&a)).await;
    let tree = f.tree().await;
    assert_eq!(children(&tree, None), ["a:0", "b:1"]);
    assert_eq!(children(&tree, Some(&a)), ["inner:0"]);
    assert!(tree.iter().all(|n| n.kind == NodeKind::Group && !n.meta));
    assert_eq!(f.changed(), 3);
}

#[tokio::test]
async fn a6_move_reorders_and_keeps_orders_contiguous_from_zero() {
    let mut f = Fixture::new();
    let a = f.group("a", None).await;
    let b = f.group("b", None).await;
    let c = f.group("c", None).await;
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
async fn a6_a_group_cannot_move_into_its_own_descendant() {
    let mut f = Fixture::new();
    let a = f.group("a", None).await;
    let b = f.group("b", Some(&a)).await;
    let c = f.group("c", Some(&b)).await;
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
    let a = f.group("a", None).await;
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
        .call("rail.createGroup", json!({"name": "x", "parent": "999"}))
        .await
        .unwrap_err();
    assert_eq!(err.code, code::NOT_FOUND);
}

#[tokio::test]
async fn a6_the_tree_lists_parents_before_their_children() {
    let f = Fixture::new();
    let a = f.group("a", None).await;
    let b = f.group("b", None).await;
    f.group("a1", Some(&a)).await;
    f.group("b1", Some(&b)).await;
    f.mv(&b, None, 0).await.unwrap();
    let names: Vec<_> = f.tree().await.into_iter().map(|n| n.name).collect();
    assert_eq!(names, ["b", "b1", "a", "a1"]);
}

#[tokio::test]
async fn a8_groups_names_parents_and_order_survive_reopening() {
    let f = Fixture::new();
    let a = f.group("a", None).await;
    let b = f.group("b", None).await;
    f.group("inner", Some(&a)).await;
    f.mv(&b, None, 0).await.unwrap();
    let before = f.tree().await;
    assert_eq!(f.reopen().tree().await, before);
}
