//! A6 (rail tree) and A8 (persistence) through the module's RPC surface.

use std::path::Path;
use std::sync::Arc;

use agents::Agents;
use contracts::agent::{NodeKind, RailNode};
use contracts::{Actor, EventData};
use provenance::Touches;
use rpc::{Bus, Ctx, Module, code};
use serde_json::{Value, json};
use terminal::Terminals;
use tokio::sync::broadcast::Receiver;

struct Fixture {
    dir: tempfile::TempDir,
    bus: Bus,
    events: Receiver<contracts::Event>,
    agents: Agents,
}

fn open_in(dir: &Path, bus: &Bus) -> Agents {
    let terminals = Arc::new(Terminals::open(dir, bus.clone()).unwrap());
    Agents::open(dir, bus.clone(), terminals).unwrap()
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let bus = Bus::new();
    let events = bus.subscribe();
    let agents = open_in(dir.path(), &bus);
    Fixture {
        dir,
        bus,
        events,
        agents,
    }
}

fn ctx(bus: &Bus) -> Ctx {
    Ctx {
        actor: Actor::user(),
        bus: bus.clone(),
        touches: Arc::new(Touches::in_memory().unwrap()),
    }
}

impl Fixture {
    async fn call(&self, method: &str, params: Value) -> Result<Value, rpc::RpcError> {
        self.agents.call(&ctx(&self.bus), method, params).await
    }

    async fn group(&self, name: &str, parent: Option<&str>) -> String {
        let node = self
            .call("rail.createGroup", json!({"name": name, "parent": parent}))
            .await
            .unwrap();
        node["id"].as_str().unwrap().to_owned()
    }

    async fn tree(&self) -> Vec<RailNode> {
        let tree = self.call("rail.tree", Value::Null).await.unwrap();
        serde_json::from_value(tree).unwrap()
    }

    async fn mv(&self, id: &str, parent: Option<&str>, index: u32) -> Result<Value, rpc::RpcError> {
        self.call(
            "rail.move",
            json!({"id": id, "parent": parent, "index": index}),
        )
        .await
    }

    fn changed(&mut self) -> usize {
        std::iter::from_fn(|| self.events.try_recv().ok())
            .filter(|event| matches!(event.data, EventData::RailChanged))
            .count()
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
    let mut f = fixture();
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
    let mut f = fixture();
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
}

#[tokio::test]
async fn a6_move_to_the_root_with_parent_none() {
    let f = fixture();
    let a = f.group("a", None).await;
    let inner = f.group("inner", Some(&a)).await;
    f.mv(&inner, None, 0).await.unwrap();
    assert_eq!(children(&f.tree().await, None), ["inner:0", "a:1"]);
}

#[tokio::test]
async fn a6_a_group_cannot_move_into_its_own_descendant() {
    let mut f = fixture();
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
    let mut f = fixture();
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
    let f = fixture();
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
    let f = fixture();
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
    let f = fixture();
    let a = f.group("a", None).await;
    let b = f.group("b", None).await;
    f.group("inner", Some(&a)).await;
    f.mv(&b, None, 0).await.unwrap();
    let before = f.tree().await;
    let Fixture {
        dir, bus, agents, ..
    } = f;
    drop(agents);

    let again = open_in(dir.path(), &bus);
    let tree = again
        .call(&ctx(&bus), "rail.tree", Value::Null)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_value::<Vec<RailNode>>(tree).unwrap(),
        before
    );
}
