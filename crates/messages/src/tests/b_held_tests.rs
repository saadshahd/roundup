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

use contracts::agent::{NodeKind, StatusEvent};
use contracts::{Actor, Event, EventData, Kind, Status};
use rpc::code;
use serde_json::{Value, json};
use tokio::sync::broadcast::Receiver;

use super::*;
use super::{FakeRail, agent, agent_node, node};

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
        Self::over_with(dir, rail, bus, deliver, calls)
    }

    fn over_with(
        dir: &Path,
        rail: Arc<FakeRail>,
        bus: Bus,
        deliver: Deliver,
        calls: Calls,
    ) -> Self {
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

/// The Daemon is built before its runtime exists (a desktop test builds it, then `block_on`s).
#[test]
fn b2_a_messages_opened_before_any_runtime_still_types_on_idle() {
    let dir = tempfile::tempdir().unwrap();
    let h = Held2::new(dir.path(), vec![agent_node("b", Kind::Working)]);

    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(async {
            h.send_as(agent("a"), "b", "wait for me").await;
            h.becomes("b", Kind::Idle);
            assert_eq!(h.typed(1).await.len(), 1);
        });
}

/// A `deliver` that records the call, then waits for `release` before it answers `Ok`.
fn blocking() -> (Deliver, Calls, Arc<tokio::sync::Notify>) {
    let calls: Calls = Arc::default();
    let seen = Arc::clone(&calls);
    let release = Arc::new(tokio::sync::Notify::new());
    let gate = Arc::clone(&release);
    let deliver: Deliver = Arc::new(move |id: String, text: String| {
        seen.lock().unwrap().push((id, text));
        let gate = Arc::clone(&gate);
        let done: Pin<Box<dyn Future<Output = Result<(), Refusal>> + Send>> =
            Box::pin(async move {
                gate.notified().await;
                Ok(())
            });
        done
    });
    (deliver, calls, release)
}

/// A `deliver` that records the call and refuses it with `Busy`.
fn refusing() -> (Deliver, Calls) {
    let calls: Calls = Arc::default();
    let seen = Arc::clone(&calls);
    let deliver: Deliver = Arc::new(move |id: String, text: String| {
        seen.lock().unwrap().push((id, text));
        let done: Pin<Box<dyn Future<Output = Result<(), Refusal>> + Send>> =
            Box::pin(async { Err(Refusal::Busy) });
        done
    });
    (deliver, calls)
}

#[tokio::test]
async fn b6_a_message_recorded_delivered_before_begin_stays_delivered() {
    let dir = tempfile::tempdir().unwrap();
    let (deliver, calls, release) = blocking();
    let rail = FakeRail::new(vec![agent_node("b", Kind::Working)]);
    let h = Held2::over_with(dir.path(), rail, Bus::new(), deliver, calls);
    h.send_as(agent("a"), "b", "one").await;
    h.becomes("b", Kind::Idle);
    h.typed(1).await;

    let begin = tokio::time::timeout(
        Duration::from_secs(1),
        h.call("takeover.begin", json!({"agent": "b"})),
    )
    .await;
    release.notify_waiters();
    h.settle().await;
    h.call("takeover.end", json!({"agent": "b"})).await.unwrap();
    h.becomes("b", Kind::Working);
    h.becomes("b", Kind::Idle);
    h.settle().await;

    assert!(begin.is_ok(), "begin waited for a prompt being typed");
    assert_eq!(h.get(1).await["status"], "delivered");
    assert_eq!(h.typed(1).await.len(), 1, "typed twice");
}

#[tokio::test]
async fn b6_pending_messages_are_held_when_a_takeover_begins() {
    let (_dir, mut h) = project();
    h.send_as(agent("a"), "b", "one").await;
    h.send_as(Actor::user(), "b", "two").await;
    h.names();

    h.call("takeover.begin", json!({"agent": "b"}))
        .await
        .unwrap();

    let states = h.states().await;
    assert_eq!(
        (states[&1].clone(), states[&2].clone()),
        (held("takeover"), pending())
    );
    assert_eq!(h.names(), ["message.held", "takeover.changed"]);
}

#[tokio::test]
async fn b7_messages_released_by_an_end_go_behind_those_already_pending() {
    let (_dir, h) = project();
    h.call("takeover.begin", json!({"agent": "b"}))
        .await
        .unwrap();
    h.send_as(agent("a"), "b", "one").await;
    h.send_as(Actor::user(), "b", "two").await;
    h.call("takeover.end", json!({"agent": "b"})).await.unwrap();

    h.becomes("b", Kind::Idle);
    h.typed(1).await;
    h.becomes("b", Kind::Working);
    h.becomes("b", Kind::Idle);

    let typed = h.typed(2).await;
    assert!(
        typed[0].ends_with("two") && typed[1].ends_with("one"),
        "{typed:?}"
    );
}

#[tokio::test]
async fn b7_a_message_sent_after_a_release_goes_behind_it() {
    let (_dir, h) = project();
    h.route("b", "ask-first").await;
    h.send_as(agent("a"), "b", "one").await;
    h.call("message.deliver", json!({"id": 1})).await.unwrap();
    h.route("b", "auto").await;
    h.send_as(agent("a"), "b", "two").await;

    h.becomes("b", Kind::Idle);
    h.typed(1).await;
    h.becomes("b", Kind::Working);
    h.becomes("b", Kind::Idle);

    let typed = h.typed(2).await;
    assert!(
        typed[0].ends_with("one") && typed[1].ends_with("two"),
        "{typed:?}"
    );
}

#[tokio::test]
async fn b7_a_hold_released_on_reopen_goes_behind_the_pending_messages() {
    let (_dir, h) = project();
    h.send_as(agent("a"), "b", "one").await;
    h.call("takeover.begin", json!({"agent": "b"}))
        .await
        .unwrap();
    h.send_as(Actor::user(), "b", "two").await;

    let h = h.restarted();
    h.states().await;
    h.becomes("b", Kind::Idle);
    h.typed(1).await;
    h.becomes("b", Kind::Working);
    h.becomes("b", Kind::Idle);

    let typed = h.typed(2).await;
    assert!(
        typed[0].ends_with("two") && typed[1].ends_with("one"),
        "{typed:?}"
    );
}

#[tokio::test]
async fn b2_an_idle_records_delivered_and_emits_message_delivered() {
    let (_dir, mut h) = project();
    h.send_as(agent("a"), "b", "one").await;
    h.names();

    h.becomes("b", Kind::Idle);
    h.until(1, "delivered").await;

    assert_eq!(h.names(), ["agent.status", "message.delivered"]);
}

#[tokio::test]
async fn b2_a_refused_delivery_leaves_the_message_pending_and_emits_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (deliver, calls) = refusing();
    let rail = FakeRail::new(vec![agent_node("b", Kind::Working)]);
    let mut h = Held2::over_with(dir.path(), rail, Bus::new(), deliver, calls);
    h.send_as(agent("a"), "b", "one").await;
    h.names();

    h.becomes("b", Kind::Idle);
    h.typed(1).await;
    h.settle().await;

    assert_eq!(h.get(1).await["status"], "pending");
    assert_eq!(h.names(), ["agent.status"]);
}

#[tokio::test]
async fn b2_typed_text_is_from_name_kind_body() {
    let dir = tempfile::tempdir().unwrap();
    let h = Held2::new(
        dir.path(),
        vec![
            agent_node("a", Kind::Working),
            agent_node("b", Kind::Working),
        ],
    );
    let question = json!({"to": "b", "kind": "question", "body": "x"});
    h.call_as(agent("a"), "message.send", question)
        .await
        .unwrap();
    h.send_as(Actor::user(), "b", "y").await;

    h.becomes("b", Kind::Idle);
    h.typed(1).await;
    h.becomes("b", Kind::Working);
    h.becomes("b", Kind::Idle);

    assert_eq!(
        h.typed(2).await,
        ["[from a, question] x", "[from you, note] y"]
    );
}

#[tokio::test]
async fn b9_a_done_lost_to_lag_still_drops_pending() {
    let (_dir, h) = project();
    h.send_as(agent("a"), "b", "one").await;
    h.becomes("b", Kind::Done);
    for _ in 0..2000 {
        h.bus.emit(
            Actor::daemon(),
            EventData::AgentStatus(StatusEvent {
                id: "z".into(),
                status: Status {
                    kind: Kind::Working,
                    label: "x".into(),
                    since: 0,
                },
            }),
        );
    }

    h.until(1, "dropped").await;

    assert_eq!(h.states().await[&1], dropped("receiver gone"));
}

#[test]
fn b2_a_reopened_store_opened_before_any_runtime_types_on_an_idle_before_the_first_call() {
    let dir = tempfile::tempdir().unwrap();
    let first = Held2::new(dir.path(), vec![agent_node("b", Kind::Working)]);
    let runtime = || {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
    };
    runtime().block_on(first.send_as(agent("a"), "b", "one"));

    let h = first.restarted();
    h.becomes("b", Kind::Idle);

    runtime().block_on(async {
        h.states().await;
        assert_eq!(h.typed(1).await.len(), 1);
    });
}

#[tokio::test]
async fn b6_the_user_a_terminal_and_a_group_cannot_be_taken_over() {
    let dir = tempfile::tempdir().unwrap();
    let h = Held2::new(
        dir.path(),
        vec![
            node("t", NodeKind::Terminal, false, None),
            node("g", NodeKind::Group, false, None),
        ],
    );

    for id in ["you", "t", "g", "ghost"] {
        for method in ["takeover.begin", "takeover.end"] {
            let err = h.call(method, json!({"agent": id})).await.unwrap_err();
            assert_eq!(err.code, code::NOT_FOUND, "{method} {id}");
        }
    }
}

/// A `deliver` that records the call, waits for `release`, then refuses with `Busy`.
fn blocking_then_busy() -> (Deliver, Calls, Arc<tokio::sync::Notify>) {
    let calls: Calls = Arc::default();
    let seen = Arc::clone(&calls);
    let release = Arc::new(tokio::sync::Notify::new());
    let gate = Arc::clone(&release);
    let deliver: Deliver = Arc::new(move |id: String, text: String| {
        seen.lock().unwrap().push((id, text));
        let gate = Arc::clone(&gate);
        let done: Pin<Box<dyn Future<Output = Result<(), Refusal>> + Send>> =
            Box::pin(async move {
                gate.notified().await;
                Err(Refusal::Busy)
            });
        done
    });
    (deliver, calls, release)
}

#[tokio::test]
async fn b6_a_busy_refusal_during_a_takeover_holds_the_message_for_it() {
    let dir = tempfile::tempdir().unwrap();
    let (deliver, calls, release) = blocking_then_busy();
    let rail = FakeRail::new(vec![agent_node("b", Kind::Working)]);
    let mut h = Held2::over_with(dir.path(), rail, Bus::new(), deliver, calls);
    h.send_as(agent("a"), "b", "one").await;
    h.becomes("b", Kind::Idle);
    h.typed(1).await;
    h.call("takeover.begin", json!({"agent": "b"}))
        .await
        .unwrap();
    h.names();

    release.notify_waiters();
    h.until(1, "held").await;
    h.becomes("b", Kind::Working);
    h.becomes("b", Kind::Idle);
    h.settle().await;

    assert_eq!(h.states().await[&1], held("takeover"));
    assert_eq!(h.typed(1).await.len(), 1, "typed during the Takeover");
    assert!(h.names().contains(&"message.held".to_owned()));
}

#[tokio::test]
async fn b9_a_lag_drops_nothing_for_a_live_agent() {
    let (_dir, h) = project();
    h.send_as(agent("a"), "b", "one").await;
    for _ in 0..2000 {
        h.bus.emit(
            Actor::daemon(),
            EventData::AgentStatus(StatusEvent {
                id: "z".into(),
                status: Status {
                    kind: Kind::Working,
                    label: "x".into(),
                    since: 0,
                },
            }),
        );
    }
    h.settle().await;

    assert_eq!(h.states().await[&1], pending());
}

#[tokio::test]
async fn b2_a_reopened_store_on_a_runtime_types_on_an_idle_before_any_call() {
    let (_dir, h) = project();
    h.send_as(agent("a"), "b", "one").await;

    let h = h.restarted();
    h.becomes("b", Kind::Idle);

    assert_eq!(h.typed(1).await.len(), 1);
}

#[tokio::test]
async fn b6_drop_of_a_takeover_held_message() {
    let (_dir, mut h) = project();
    h.call("takeover.begin", json!({"agent": "b"}))
        .await
        .unwrap();
    h.send_as(agent("a"), "b", "one").await;
    h.names();

    h.call("message.drop", json!({"id": 1})).await.unwrap();
    let later = h.send_as(agent("a"), "b", "two").await;
    h.call("takeover.end", json!({"agent": "b"})).await.unwrap();

    assert_eq!(h.states().await[&1], ("dropped".into(), Value::Null));
    assert_eq!(
        later["status"], "held",
        "the Takeover went on after the drop"
    );
    assert_eq!(
        h.names(),
        [
            "message.dropped",
            "message.sent",
            "message.held",
            "takeover.changed"
        ]
    );
}

#[tokio::test]
async fn b1_held_for_takeover_messages_count_toward_the_bound_of_32() {
    let (_dir, h) = project();
    h.call("takeover.begin", json!({"agent": "b"}))
        .await
        .unwrap();
    for _ in 0..32 {
        h.send_as(agent("a"), "b", "x").await;
    }

    let err = h
        .call_as(
            agent("a"),
            "message.send",
            json!({"to": "b", "kind": "note", "body": "x"}),
        )
        .await
        .unwrap_err();

    assert_eq!(err.code, code::CONFLICT);
}

#[tokio::test]
async fn b6_a_meta_agent_can_be_taken_over() {
    let dir = tempfile::tempdir().unwrap();
    let working = Status {
        kind: Kind::Working,
        label: "x".into(),
        since: 0,
    };
    let h = Held2::new(
        dir.path(),
        vec![node("m", NodeKind::Group, true, Some(working))],
    );

    h.call("takeover.begin", json!({"agent": "m"}))
        .await
        .unwrap();
    let sent = h.send_as(agent("a"), "m", "one").await;

    assert_eq!(
        (sent["status"].clone(), sent["reason"].clone()),
        ("held".into(), json!("takeover"))
    );
}

#[tokio::test]
async fn b2_typed_text_names_an_agent_by_its_rail_name() {
    let dir = tempfile::tempdir().unwrap();
    let mut renamed = agent_node("a", Kind::Working);
    renamed.name = "builder".into();
    let h = Held2::new(dir.path(), vec![renamed, agent_node("b", Kind::Working)]);
    h.send_as(agent("a"), "b", "x").await;

    h.becomes("b", Kind::Idle);

    assert_eq!(h.typed(1).await, ["[from builder, note] x"]);
}

#[tokio::test]
async fn b6_a_busy_refusal_of_a_users_message_during_a_takeover_leaves_it_pending() {
    let dir = tempfile::tempdir().unwrap();
    let (deliver, calls, release) = blocking_then_busy();
    let rail = FakeRail::new(vec![agent_node("b", Kind::Working)]);
    let mut h = Held2::over_with(dir.path(), rail, Bus::new(), deliver, calls);
    h.send_as(Actor::user(), "b", "one").await;
    h.becomes("b", Kind::Idle);
    h.typed(1).await;
    h.call("takeover.begin", json!({"agent": "b"}))
        .await
        .unwrap();
    h.names();

    release.notify_waiters();
    h.settle().await;

    assert_eq!(h.states().await[&1], pending());
    assert!(!h.names().contains(&"message.held".to_owned()));
}

#[tokio::test]
async fn b2_a_rail_that_cannot_answer_leaves_the_message_pending_for_the_next_idle() {
    let (_dir, h) = project();
    h.send_as(agent("a"), "b", "one").await;

    h.rail.fail(true);
    h.becomes("b", Kind::Idle);
    h.settle().await;
    assert_eq!(h.get(1).await["status"], "pending");
    assert!(h.calls.lock().unwrap().is_empty());

    h.rail.fail(false);
    h.becomes("b", Kind::Working);
    h.becomes("b", Kind::Idle);
    h.until(1, "delivered").await;
    assert_eq!(h.typed(1).await.len(), 1);
}

fn status_flood(h: &Held2) {
    for _ in 0..2000 {
        h.bus.emit(
            Actor::daemon(),
            EventData::AgentStatus(StatusEvent {
                id: "z".into(),
                status: Status {
                    kind: Kind::Working,
                    label: "x".into(),
                    since: 0,
                },
            }),
        );
    }
}

#[tokio::test]
async fn b9_a_lag_while_the_rail_cannot_answer_still_drops_pending_once_it_can() {
    let (_dir, h) = project();
    h.send_as(agent("a"), "b", "one").await;

    h.rail.fail(true);
    h.becomes("b", Kind::Done);
    status_flood(&h);
    h.settle().await;
    h.rail.fail(false);
    status_flood(&h);

    h.until(1, "dropped").await;
    assert_eq!(h.states().await[&1], dropped("receiver gone"));
}

#[tokio::test]
async fn b2_a_sender_no_longer_on_the_rail_is_named_by_its_id() {
    let (_dir, h) = project();
    h.send_as(agent("a"), "b", "one").await;

    h.becomes("b", Kind::Idle);

    assert_eq!(h.typed(1).await, ["[from a, note] one"]);
}
