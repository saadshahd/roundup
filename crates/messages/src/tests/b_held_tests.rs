//! B6 Takeover, B7 order, B9 receiver gone, B10 the user as receiver. Written by an architect
//! before slice 2; the Builder makes these pass and never edits this file (the one-line `mod` in
//! `lib.rs` and the `Messages::open` call in the older `Harness` are the Builder's to amend).
//!
//! Pinned seams, the only names these tests need that the contract does not give:
//! - `Messages::open(dir, bus, agents, deliver: Deliver)` with
//!   `pub type Deliver = Arc<dyn Fn(String, String) -> Pin<Box<dyn Future<Output = Result<(),
//!   Refusal>> + Send>> + Send + Sync>` and `pub enum Refusal { Busy, NotAccepted, NotFound }`;
//!   the arguments are the Agent id and the text. Slice 3 maps `Agents::prompt` onto it.
//! - `Messages` reads `agent.status` events on the `bus` it is opened with: that is how it learns
//!   a Kind changed. An `idle` event calls `deliver` once, for the oldest deliverable Message to
//!   that Agent (B2's "next time `a` becomes `idle`"; typing at the moment of a send, and the
//!   outcomes of a refusal, are slice 3). A `done` or `error` event is B9, and ends a Takeover.
//! - A Message's `reason` on the wire is a string: `ask-first`, `takeover`, `receiver gone`.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use contracts::agent::StatusEvent;
use contracts::{Actor, Event, EventData, Kind, Status};
use rpc::code;
use serde_json::{Value, json};
use tokio::sync::broadcast::Receiver;

use super::*;
use super::{FakeRail, agent, agent_node};

type Calls = Arc<StdMutex<Vec<(String, String)>>>;

struct Held2 {
    messages: Arc<Messages>,
    bus: Bus,
    rail: Arc<FakeRail>,
    calls: Calls,
    events: Receiver<Event>,
    touches: Arc<provenance::Touches>,
    dir: std::path::PathBuf,
}

fn recording() -> (Deliver, Calls) {
    let calls: Calls = Arc::default();
    let seen = Arc::clone(&calls);
    let deliver: Deliver = Arc::new(move |id: String, text: String| {
        seen.lock().unwrap().push((id, text));
        let done: Pin<Box<dyn Future<Output = Result<(), Refusal>> + Send>> =
            Box::pin(async { Ok(()) });
        done
    });
    (deliver, calls)
}

impl Held2 {
    fn new(dir: &Path, nodes: Vec<RailNode>) -> Self {
        let rail = FakeRail::new(nodes);
        Self::over(dir, rail, Bus::new())
    }

    fn over(dir: &Path, rail: Arc<FakeRail>, bus: Bus) -> Self {
        let (deliver, calls) = recording();
        let events = bus.subscribe();
        let agents: Arc<dyn Module> = rail.clone();
        let messages = Arc::new(Messages::open(dir, bus.clone(), agents, deliver).unwrap());
        Self {
            messages,
            bus,
            rail,
            calls,
            events,
            touches: Arc::new(provenance::Touches::in_memory().unwrap()),
            dir: dir.to_path_buf(),
        }
    }

    /// The same Project, a new Daemon: a fresh `Messages` over the same files and Rail.
    fn restarted(self) -> Self {
        let Self { rail, dir, .. } = self;
        Self::over(&dir, rail, Bus::new())
    }

    async fn call_as(&self, actor: Actor, method: &str, p: Value) -> Result<Value, RpcError> {
        let ctx = Ctx {
            actor,
            bus: self.bus.clone(),
            touches: Arc::clone(&self.touches),
        };
        self.messages.call(&ctx, method, p).await
    }

    async fn call(&self, method: &str, p: Value) -> Result<Value, RpcError> {
        self.call_as(Actor::user(), method, p).await
    }

    async fn send_as(&self, from: Actor, to: &str, body: &str) -> Value {
        let p = json!({"to": to, "kind": "note", "body": body});
        self.call_as(from, "message.send", p).await.unwrap()
    }

    async fn get(&self, id: u32) -> Value {
        self.call("message.get", json!({"id": id})).await.unwrap()
    }

    /// `(status, reason)` of each Message, by id.
    async fn states(&self) -> BTreeMap<u32, (String, Value)> {
        let all = self.call("message.list", json!({})).await.unwrap();
        all.as_array()
            .unwrap()
            .iter()
            .map(|m| {
                let state = (
                    m["status"].as_str().unwrap().to_owned(),
                    m["reason"].clone(),
                );
                (m["id"].as_u64().unwrap() as u32, state)
            })
            .collect()
    }

    async fn route(&self, to: &str, delivery: &str) {
        let p = json!({"from": "a", "to": to, "delivery": delivery});
        self.call("route.set", p).await.unwrap();
    }

    /// Tell `Messages` an Agent's Kind changed, the way the Agents module does: update the Rail,
    /// then announce it on the bus.
    fn becomes(&self, id: &str, kind: Kind) {
        let status = Status {
            kind,
            label: "x".into(),
            since: 0,
        };
        for node in self
            .rail
            .0
            .lock()
            .unwrap()
            .iter_mut()
            .filter(|n| n.id == id)
        {
            node.status = Some(status.clone());
        }
        let data = EventData::AgentStatus(StatusEvent {
            id: id.into(),
            status,
        });
        self.bus.emit(Actor::daemon(), data);
    }

    /// `name` plus payload of each event since the last call, oldest first.
    fn seen(&mut self) -> Vec<(String, Value)> {
        let mut seen = vec![];
        while let Ok(event) = self.events.try_recv() {
            let wire = serde_json::to_value(&event.data).unwrap();
            seen.push((
                wire["name"].as_str().unwrap().to_owned(),
                wire["data"].clone(),
            ));
        }
        seen
    }

    fn names(&mut self) -> Vec<String> {
        self.seen().into_iter().map(|(name, _)| name).collect()
    }

    /// Wait until `deliver` has been called `n` times; the bodies it was given, in order.
    async fn typed(&self, n: usize) -> Vec<String> {
        for _ in 0..500 {
            if self.calls.lock().unwrap().len() >= n {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let calls = self.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), n, "{calls:?}");
        calls.into_iter().map(|(_, text)| text).collect()
    }

    /// Wait for Message `id` to reach `status`.
    async fn until(&self, id: u32, status: &str) {
        for _ in 0..500 {
            if self.get(id).await["status"] == status {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("Message {id} never became {status}: {}", self.get(id).await);
    }

    async fn settle(&self) {
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
}

fn held(reason: &str) -> (String, Value) {
    ("held".into(), json!(reason))
}

fn pending() -> (String, Value) {
    ("pending".into(), Value::Null)
}

fn dropped(reason: &str) -> (String, Value) {
    ("dropped".into(), json!(reason))
}

fn project() -> (tempfile::TempDir, Held2) {
    let dir = tempfile::tempdir().unwrap();
    let h = Held2::new(dir.path(), vec![agent_node("b", Kind::Working)]);
    (dir, h)
}

#[tokio::test]
async fn b6_only_the_user_begins_and_ends_a_takeover() {
    let (_dir, h) = project();

    for method in ["takeover.begin", "takeover.end"] {
        let err = h
            .call_as(agent("a"), method, json!({"agent": "b"}))
            .await
            .unwrap_err();
        assert_eq!(err.code, code::FORBIDDEN, "{method}");
    }
}

#[tokio::test]
async fn b6_an_id_that_names_no_agent_is_not_found_and_an_ended_agent_is_a_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let h = Held2::new(
        dir.path(),
        vec![
            agent_node("done", Kind::Done),
            agent_node("bad", Kind::Error),
        ],
    );

    let ghost = h
        .call("takeover.begin", json!({"agent": "ghost"}))
        .await
        .unwrap_err();
    let done = h
        .call("takeover.begin", json!({"agent": "done"}))
        .await
        .unwrap_err();
    let bad = h
        .call("takeover.begin", json!({"agent": "bad"}))
        .await
        .unwrap_err();

    assert_eq!(
        (ghost.code, done.code, bad.code),
        (code::NOT_FOUND, code::CONFLICT, code::CONFLICT)
    );
}

#[tokio::test]
async fn b6_each_change_emits_once_and_a_repeat_changes_nothing() {
    let (_dir, mut h) = project();

    h.call("takeover.end", json!({"agent": "b"})).await.unwrap();
    h.call("takeover.begin", json!({"agent": "b"}))
        .await
        .unwrap();
    h.call("takeover.begin", json!({"agent": "b"}))
        .await
        .unwrap();
    h.call("takeover.end", json!({"agent": "b"})).await.unwrap();
    h.call("takeover.end", json!({"agent": "b"})).await.unwrap();

    let changes: Vec<Value> = h
        .seen()
        .into_iter()
        .filter(|(name, _)| name == "takeover.changed")
        .map(|(_, data)| data)
        .collect();
    assert_eq!(
        changes,
        [
            json!({"agent": "b", "on": true}),
            json!({"agent": "b", "on": false})
        ]
    );
}

#[tokio::test]
async fn b6_during_a_takeover_a_message_from_anyone_but_the_user_is_held_for_it() {
    let (_dir, mut h) = project();
    h.route("b", "auto").await;
    h.call("takeover.begin", json!({"agent": "b"}))
        .await
        .unwrap();
    h.names();

    let auto = h.send_as(agent("a"), "b", "auto").await;
    let other = h.send_as(agent("c"), "b", "no route set").await;
    h.route("b", "ask-first").await;
    let ask = h.send_as(agent("a"), "b", "ask").await;
    h.route("b", "drop").await;
    let drop = h.send_as(agent("a"), "b", "drop").await;
    let user = h.send_as(Actor::user(), "b", "from the user").await;

    let status = |m: &Value| {
        (
            m["status"].as_str().unwrap().to_owned(),
            m["reason"].clone(),
        )
    };
    assert_eq!(status(&auto), held("takeover"));
    assert_eq!(status(&other), held("takeover"));
    assert_eq!(status(&ask), held("ask-first"));
    assert_eq!(status(&drop).0, "dropped");
    assert_eq!(status(&user), pending());
    assert_eq!(
        h.names(),
        [
            "message.sent",
            "message.held",
            "message.sent",
            "message.held",
            "route.changed",
            "message.sent",
            "message.held",
            "route.changed",
            "message.sent",
            "message.dropped",
            "message.sent"
        ]
    );
}

#[tokio::test]
async fn b6_the_end_releases_takeover_holds_in_id_order_and_keeps_ask_first_held() {
    let (_dir, h) = project();
    h.call("takeover.begin", json!({"agent": "b"}))
        .await
        .unwrap();
    h.send_as(agent("a"), "b", "one").await;
    h.route("b", "ask-first").await;
    h.send_as(agent("a"), "b", "two").await;
    h.route("b", "auto").await;
    h.send_as(agent("a"), "b", "three").await;

    h.call("takeover.end", json!({"agent": "b"})).await.unwrap();
    h.becomes("b", Kind::Idle);
    h.becomes("b", Kind::Idle);

    let states = h.states().await;
    assert_eq!(states[&2], held("ask-first"));
    let typed = h.typed(2).await;
    assert!(
        typed[0].contains("one") && typed[1].contains("three"),
        "{typed:?}"
    );
    h.until(1, "delivered").await;
    h.until(3, "delivered").await;
}

#[tokio::test]
async fn b6_deliver_on_a_takeover_hold_releases_it_though_the_takeover_goes_on() {
    let (_dir, h) = project();
    h.call("takeover.begin", json!({"agent": "b"}))
        .await
        .unwrap();
    h.send_as(agent("a"), "b", "one").await;
    h.send_as(agent("a"), "b", "two").await;

    h.call("message.deliver", json!({"id": 2})).await.unwrap();

    let states = h.states().await;
    assert_eq!(
        (states[&1].clone(), states[&2].clone()),
        (held("takeover"), pending())
    );
    h.becomes("b", Kind::Idle);
    assert!(h.typed(1).await[0].contains("two"));
}

#[tokio::test]
async fn b6_a_takeover_ends_by_itself_when_the_agent_exits_and_its_holds_are_then_dropped() {
    let (_dir, mut h) = project();
    h.call("takeover.begin", json!({"agent": "b"}))
        .await
        .unwrap();
    h.send_as(agent("a"), "b", "one").await;
    h.seen();

    h.becomes("b", Kind::Done);

    h.until(1, "dropped").await;
    assert_eq!(h.states().await[&1], dropped("receiver gone"));
    let order: Vec<(String, Value)> = h.seen();
    let names: Vec<&str> = order
        .iter()
        .map(|(n, _)| n.as_str())
        .filter(|n| n.starts_with("takeover") || n.starts_with("message"))
        .collect();
    assert_eq!(names, ["takeover.changed", "message.dropped"]);
    let changed = order.iter().find(|(n, _)| n == "takeover.changed").unwrap();
    assert_eq!(changed.1, json!({"agent": "b", "on": false}));
}

#[tokio::test]
async fn b6_a_new_daemon_has_no_takeover_and_its_holds_are_pending() {
    let (_dir, h) = project();
    h.call("takeover.begin", json!({"agent": "b"}))
        .await
        .unwrap();
    h.send_as(agent("a"), "b", "one").await;
    h.route("b", "ask-first").await;
    h.send_as(agent("a"), "b", "two").await;

    let h = h.restarted();

    let states = h.states().await;
    assert_eq!(
        (states[&1].clone(), states[&2].clone()),
        (pending(), held("ask-first"))
    );
    let later = h.send_as(agent("c"), "b", "three").await;
    assert_eq!(later["status"], "pending");
}

#[tokio::test]
async fn b7_messages_are_typed_in_the_order_they_became_deliverable() {
    let (_dir, h) = project();
    h.call("takeover.begin", json!({"agent": "b"}))
        .await
        .unwrap();
    h.send_as(agent("a"), "b", "held-first").await;
    h.send_as(Actor::user(), "b", "pending-a").await;
    h.send_as(Actor::user(), "b", "pending-b").await;
    h.call("message.deliver", json!({"id": 1})).await.unwrap();

    for _ in 0..3 {
        h.becomes("b", Kind::Idle);
        h.settle().await;
    }

    let typed = h.typed(3).await;
    let bodies: Vec<&str> = ["pending-a", "pending-b", "held-first"].into();
    for (text, body) in typed.iter().zip(bodies) {
        assert!(text.contains(body), "{typed:?}");
    }
}

#[tokio::test]
async fn b7_exactly_one_message_is_typed_per_idle() {
    let (_dir, h) = project();
    h.send_as(agent("a"), "b", "one").await;
    h.send_as(agent("a"), "b", "two").await;

    h.becomes("b", Kind::Idle);
    h.settle().await;

    assert_eq!(h.typed(1).await.len(), 1);
}

#[tokio::test]
async fn b7_a_held_message_never_blocks_a_later_one() {
    let (_dir, h) = project();
    h.route("b", "ask-first").await;
    h.send_as(agent("a"), "b", "held").await;
    h.route("b", "auto").await;
    h.send_as(agent("a"), "b", "later").await;

    h.becomes("b", Kind::Idle);

    assert!(h.typed(1).await[0].contains("later"));
    assert_eq!(h.states().await[&1], held("ask-first"));
}

#[tokio::test]
async fn b9_an_agent_that_ends_drops_its_pending_messages_in_id_order() {
    for kind in [Kind::Done, Kind::Error] {
        let (_dir, mut h) = project();
        h.send_as(agent("a"), "b", "one").await;
        h.route("b", "ask-first").await;
        h.send_as(agent("a"), "b", "kept").await;
        h.route("b", "auto").await;
        h.send_as(agent("a"), "b", "three").await;
        h.seen();

        h.becomes("b", kind);

        h.until(3, "dropped").await;
        let states = h.states().await;
        assert_eq!(
            (states[&1].clone(), states[&2].clone(), states[&3].clone()),
            (
                dropped("receiver gone"),
                held("ask-first"),
                dropped("receiver gone")
            ),
            "{kind:?}"
        );
        let dropped_ids: Vec<u64> = h
            .seen()
            .into_iter()
            .filter(|(name, _)| name == "message.dropped")
            .map(|(_, data)| data["id"].as_u64().unwrap())
            .collect();
        assert_eq!(dropped_ids, [1, 3], "{kind:?}");
    }
}

#[tokio::test]
async fn b9_a_later_send_or_a_deliver_to_an_ended_agent_is_a_conflict_and_changes_nothing() {
    let (_dir, h) = project();
    h.route("b", "ask-first").await;
    h.send_as(agent("a"), "b", "kept").await;
    h.becomes("b", Kind::Done);
    h.settle().await;

    let send = h
        .call_as(
            agent("a"),
            "message.send",
            json!({"to": "b", "kind": "note", "body": "late"}),
        )
        .await
        .unwrap_err();
    let deliver = h
        .call("message.deliver", json!({"id": 1}))
        .await
        .unwrap_err();

    assert_eq!((send.code, deliver.code), (code::CONFLICT, code::CONFLICT));
    assert_eq!(h.states().await[&1], held("ask-first"));
}

#[tokio::test]
async fn b10_a_message_to_the_user_is_delivered_at_once_with_no_status_event() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Held2::new(dir.path(), vec![agent_node("b", Kind::Working)]);

    let sent = h.send_as(agent("b"), "you", "hello").await;

    assert_eq!(sent["status"], "delivered");
    assert_eq!(h.names(), ["message.sent", "message.delivered"]);
    assert!(h.calls.lock().unwrap().is_empty());
}
