use std::sync::Arc;

use contracts::{Actor, ActorKind, EventData, Verb};
use pads::Pads;
use provenance::Touches;
use rpc::{Bus, Ctx, Module, code};
use serde_json::{Value, json};
use tempfile::TempDir;

fn agent(id: &str) -> Actor {
    Actor {
        kind: ActorKind::Agent,
        id: id.into(),
        parent: None,
    }
}

struct Rig {
    pads: Pads,
    bus: Bus,
    touches: Arc<Touches>,
    _dir: TempDir,
}

impl Rig {
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let bus = Bus::new();
        Self {
            pads: Pads::open(dir.path(), bus.clone()).unwrap(),
            bus,
            touches: Arc::new(Touches::in_memory().unwrap()),
            _dir: dir,
        }
    }

    async fn call(
        &self,
        actor: &Actor,
        method: &str,
        params: Value,
    ) -> Result<Value, rpc::RpcError> {
        let ctx = Ctx {
            actor: actor.clone(),
            bus: self.bus.clone(),
            touches: Arc::clone(&self.touches),
        };
        self.pads.call(&ctx, method, params).await
    }

    async fn ok(&self, actor: &Actor, method: &str, params: Value) -> Value {
        self.call(actor, method, params).await.unwrap()
    }

    async fn fail(&self, actor: &Actor, method: &str, params: Value) -> rpc::RpcError {
        self.call(actor, method, params).await.unwrap_err()
    }
}

#[tokio::test]
async fn p1_create_makes_the_caller_owner_and_logs_a_write() {
    let rig = Rig::new();
    let mut events = rig.bus.subscribe();
    let a = agent("a");

    let pad = rig
        .ok(&a, "pad.create", json!({"name": "notes", "text": "hi"}))
        .await;

    assert_eq!(pad["owner"]["id"], "a");
    assert_eq!(pad["text"], "hi");
    let duplicate = rig
        .ok(&a, "pad.create", json!({"name": "notes"}))
        .await
        .unwrap_err();
    assert_eq!(duplicate.code, code::CONFLICT);
    let EventData::PadChanged(changed) = events.try_recv().unwrap().data else {
        panic!("expected pad.changed");
    };
    assert_eq!(changed.name, "notes");
    let history = rig.touches.history("pad:notes").unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].verb, Verb::Wrote);
}

#[tokio::test]
async fn p2_owner_rewrites_and_others_only_append() {
    let rig = Rig::new();
    let (a, b) = (agent("a"), agent("b"));
    rig.fail(&a, "pad.create", json!({"name": "notes", "text": "one"}))
        .await;

    rig.ok(&a, "pad.write", json!({"name": "notes", "text": "two"}))
        .await;
    let mut events = rig.bus.subscribe();
    let denied = rig
        .ok(&b, "pad.write", json!({"name": "notes", "text": "x"}))
        .await;
    assert_eq!(denied.code, code::FORBIDDEN);
    assert!(events.try_recv().is_err());
    rig.fail(&b, "pad.append", json!({"name": "notes", "text": "+b"}))
        .await;

    let pad = rig.ok(&a, "pad.read", json!({"name": "notes"})).await;
    assert_eq!(pad["text"], "two+b");
    assert!(matches!(
        events.try_recv().unwrap().data,
        EventData::PadChanged(_)
    ));
    let by_b: Vec<_> = rig.touches.touched("b").unwrap();
    assert_eq!(by_b.len(), 1);
    assert_eq!(by_b[0].verb, Verb::Wrote);
}

#[tokio::test]
async fn p3_list_is_ordered_and_read_is_logged() {
    let rig = Rig::new();
    let a = agent("a");
    for name in ["c", "a", "b"] {
        rig.ok(&a, "pad.create", json!({"name": name, "text": name}))
            .await;
    }

    let listed = rig.ok(&a, "pad.list", Value::Null).await;
    let names: Vec<_> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["a", "b", "c"]);
    let read = rig.ok(&a, "pad.read", json!({"name": "b"})).await;
    assert_eq!(
        (read["name"].as_str(), read["text"].as_str()),
        (Some("b"), Some("b"))
    );
    rig.ok(&a, "pad.read", json!({"name": "a"})).await;
    let verbs: Vec<_> = rig
        .touches
        .history("pad:a")
        .unwrap()
        .iter()
        .map(|t| t.verb)
        .collect();
    assert_eq!(verbs, [Verb::Wrote, Verb::Read]);
    let missing = rig.call(&a, "pad.read", json!({"name": "zzz"})).await;
    assert_eq!(missing.code, code::NOT_FOUND);
}

#[tokio::test]
async fn p8_bad_names_are_invalid_params_for_every_method() {
    let rig = Rig::new();
    let a = agent("a");
    for method in ["pad.create", "pad.read", "pad.write", "pad.append"] {
        for name in ["", "a/b", "a\\b", "..", ".hidden", "a..b"] {
            let err = rig
                .fail(&a, method, json!({"name": name, "text": "x"}))
                .await;
            assert_eq!(err.code, code::INVALID_PARAMS, "{method} {name:?}");
        }
    }
}
