//! B13 to B17: a question bubbles up the Rail one Door at a time and lands with the user. Rooms
//! nest `top` > `mid` > `sub`; Agent `c` lives in `sub`, Agent `d` in `top`. The clock is fake.

use std::sync::atomic::{AtomicI64, Ordering};

use contracts::agent::{NodeKind, StatusEvent};
use contracts::{Actor, EventData, Kind, Status, Verb};
use serde_json::{Value, json};

use super::*;
use super::{FakeRail, agent, agent_node, node};

struct Hops {
    messages: Arc<Messages>,
    bus: Bus,
    rail: Arc<FakeRail>,
    clock: Arc<AtomicI64>,
    touches: Arc<provenance::Touches>,
    typed: Arc<StdMutex<Vec<(String, String)>>>,
    dir: std::path::PathBuf,
}

fn door(id: &str, parent: Option<&str>) -> RailNode {
    let mut room = node(
        id,
        NodeKind::Room,
        true,
        Some(Status {
            kind: Kind::Working,
            label: "x".into(),
            since: 0,
        }),
    );
    room.parent = parent.map(str::to_owned);
    room
}

fn child(id: &str, parent: &str) -> RailNode {
    let mut agent = agent_node(id, Kind::Working);
    agent.parent = Some(parent.to_owned());
    agent
}

fn tree() -> Vec<RailNode> {
    vec![
        door("top", None),
        door("mid", Some("top")),
        door("sub", Some("mid")),
        child("c", "sub"),
        child("d", "top"),
    ]
}

impl Hops {
    fn new(dir: &Path) -> Self {
        Self::over(dir, FakeRail::new(tree()), Arc::default())
    }

    fn over(dir: &Path, rail: Arc<FakeRail>, clock: Arc<AtomicI64>) -> Self {
        let bus = Bus::new();
        let touches = Arc::new(provenance::Touches::in_memory().unwrap());
        let typed: Arc<StdMutex<Vec<(String, String)>>> = Arc::default();
        let seen = Arc::clone(&typed);
        let deliver: Deliver = Arc::new(move |id, text| {
            seen.lock().unwrap().push((id, text));
            Box::pin(async { Ok(()) })
        });
        let reads = Arc::clone(&clock);
        let env = Env {
            touches: Arc::clone(&touches),
            clock: Arc::new(move || reads.load(Ordering::SeqCst)),
        };
        let agents: Arc<dyn Module> = rail.clone();
        let messages =
            Arc::new(Messages::open_with(dir, bus.clone(), agents, deliver, env).unwrap());
        Self {
            messages,
            bus,
            rail,
            clock,
            touches,
            typed,
            dir: dir.to_path_buf(),
        }
    }

    fn restarted(self) -> Self {
        let Self {
            rail, clock, dir, ..
        } = self;
        Self::over(&dir, rail, clock)
    }

    async fn call_as(&self, actor: Actor, method: &str, p: Value) -> Result<Value, RpcError> {
        let ctx = Ctx {
            actor,
            bus: self.bus.clone(),
            touches: Arc::clone(&self.touches),
        };
        self.messages.call(&ctx, method, p).await
    }

    async fn ask(&self, from: &str, to: &str) -> Value {
        let p = json!({"to": to, "kind": "question", "body": "which?"});
        self.call_as(agent(from), "message.send", p).await.unwrap()
    }

    async fn get(&self, id: u32) -> Value {
        let p = json!({"id": id});
        self.call_as(Actor::user(), "message.get", p).await.unwrap()
    }

    /// The Daemon's clock reads `ms`, and every bound that ran out by then is passed.
    async fn at(&self, ms: i64) {
        self.clock.store(ms, Ordering::SeqCst);
        settle(&self.messages.inner, None).await.unwrap();
    }

    /// `(to, status, reason, passedFrom)` of every Message, by id.
    async fn all(&self) -> Vec<(String, String, Value, Value)> {
        let all = self
            .call_as(Actor::user(), "message.list", json!({}))
            .await
            .unwrap();
        all.as_array()
            .unwrap()
            .iter()
            .map(|m| {
                (
                    m["to"].as_str().unwrap().to_owned(),
                    m["status"].as_str().unwrap().to_owned(),
                    m["reason"].clone(),
                    m["passedFrom"].clone(),
                )
            })
            .collect()
    }

    /// Announce a Kind change of `id`, as the Agents module does.
    async fn becomes(&self, id: &str, kind: Kind) {
        let status = Status {
            kind,
            label: "x".into(),
            since: 0,
        };
        let mut revision = 1;
        for node in self
            .rail
            .0
            .lock()
            .unwrap()
            .iter_mut()
            .filter(|n| n.id == id)
        {
            node.status = Some(status.clone());
            revision = node
                .status_revision
                .as_deref()
                .unwrap()
                .parse::<i64>()
                .unwrap()
                + 1;
            node.status_revision = Some(revision.to_string());
        }
        self.bus.emit(
            Actor::daemon(),
            EventData::AgentStatus(StatusEvent {
                id: id.into(),
                attempt: "1".into(),
                status_revision: revision.to_string(),
                status,
            }),
        );
        // The listener reads the event on its own task.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    fn touchers(&self, id: u32) -> Vec<(Verb, String)> {
        self.touches
            .history(&item(id))
            .unwrap()
            .into_iter()
            .map(|t| (t.verb, t.actor.id))
            .collect()
    }
}

fn row(to: &str, status: &str, reason: Value, from: Value) -> (String, String, Value, Value) {
    (to.into(), status.into(), reason, from)
}

#[tokio::test]
async fn b13_a_question_goes_door_by_door_up_the_rail_and_lands_with_the_user() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());

    let first = h.ask("c", "sub").await;
    assert_eq!(first["passedFrom"], Value::Null);
    h.at(60_000).await;
    h.at(120_000).await;
    h.at(180_000).await;

    assert_eq!(
        h.all().await,
        [
            row("sub", "dropped", json!("passed"), Value::Null),
            row("mid", "dropped", json!("passed"), json!(1)),
            row("top", "dropped", json!("passed"), json!(2)),
            row("you", "held", json!("escalated"), json!(3)),
        ]
    );
    let landing = h.get(4).await;
    assert_eq!(landing["from"]["id"], "c");
    assert_eq!(landing["kind"], "question");
    assert_eq!(landing["body"], "which?");
    assert_eq!(landing["replyTo"], Value::Null);
}

#[tokio::test]
async fn b13_a_hop_made_by_a_bound_is_touched_by_rupd_and_the_first_by_the_sender() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    h.ask("c", "sub").await;
    h.at(60_000).await;

    assert_eq!(h.touchers(1), [(Verb::Wrote, "c".to_owned())]);
    assert_eq!(h.touchers(2), [(Verb::Wrote, "rupd".to_owned())]);
}

#[tokio::test]
async fn b13_the_sender_is_never_a_hop_and_the_chain_is_read_once() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    // `mid`'s Door asks `sub`: its own Room is above `sub`, so it is skipped.
    h.ask("mid", "sub").await;
    h.rail.0.lock().unwrap()[2].parent = None;
    h.at(60_000).await;
    h.at(120_000).await;

    assert_eq!(
        h.all().await,
        [
            row("sub", "dropped", json!("passed"), Value::Null),
            row("top", "dropped", json!("passed"), json!(1)),
            row("you", "held", json!("escalated"), json!(2)),
        ]
    );
}

#[tokio::test]
async fn b13_a_question_to_an_agent_or_the_user_has_one_hop() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    h.ask("c", "d").await;
    h.ask("c", "you").await;
    h.at(600_000).await;

    assert_eq!(
        h.all().await,
        [
            row("d", "pending", Value::Null, Value::Null),
            row("you", "delivered", Value::Null, Value::Null),
        ]
    );
}

#[tokio::test]
async fn b14_a_hop_has_not_passed_at_59999_ms_and_has_at_60000() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    h.clock.store(1_000, Ordering::SeqCst);
    h.ask("c", "sub").await;

    h.at(60_999).await;
    assert_eq!(h.all().await.len(), 1);
    h.at(61_000).await;
    assert_eq!(h.all().await.len(), 2);
}

#[tokio::test]
async fn b13_a_delivered_hop_stays_delivered_when_it_passes() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    h.ask("c", "sub").await;
    h.becomes("sub", Kind::Idle).await;
    assert_eq!(h.typed.lock().unwrap().len(), 1);

    h.at(60_000).await;

    let all = h.all().await;
    assert_eq!(all[0].1, "delivered");
    assert_eq!(all[1], row("mid", "pending", Value::Null, json!(1)));
}

#[tokio::test]
async fn b14_a_held_hop_has_no_bound_until_it_is_released() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    let route = json!({"from": "c", "to": "sub", "delivery": "ask-first"});
    h.call_as(Actor::user(), "route.set", route).await.unwrap();
    h.ask("c", "sub").await;

    h.at(600_000).await;
    assert_eq!(h.all().await.len(), 1);
    h.call_as(Actor::user(), "message.deliver", json!({"id": 1}))
        .await
        .unwrap();
    h.at(659_999).await;
    assert_eq!(h.all().await.len(), 1);
    h.at(660_000).await;
    assert_eq!(h.all().await.len(), 2);
}

#[tokio::test]
async fn b16_a_hop_that_ends_without_an_answer_makes_the_next_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    let route = json!({"from": "c", "to": "sub", "delivery": "ask-first"});
    h.call_as(Actor::user(), "route.set", route).await.unwrap();
    h.ask("c", "sub").await;

    h.call_as(Actor::user(), "message.drop", json!({"id": 1}))
        .await
        .unwrap();

    let all = h.all().await;
    assert_eq!(all[0].1, "dropped");
    assert_eq!(all[1], row("mid", "pending", Value::Null, json!(1)));
}

#[tokio::test]
async fn b16_a_receiver_that_is_gone_or_a_drop_route_never_delays_the_landing() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    let route = json!({"from": "c", "to": "top", "delivery": "drop"});
    h.call_as(Actor::user(), "route.set", route).await.unwrap();
    h.ask("c", "sub").await;
    // `mid` is gone before the question passes: it is never made a hop.
    h.rail.0.lock().unwrap().retain(|n| n.id != "mid");

    h.at(60_000).await;

    assert_eq!(
        h.all().await,
        [
            row("sub", "dropped", json!("passed"), Value::Null),
            row("top", "dropped", Value::Null, json!(1)),
            row("you", "held", json!("escalated"), json!(2)),
        ]
    );
}

#[tokio::test]
async fn b16_a_hop_to_a_full_receiver_is_never_made() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    for _ in 0..OPEN_BOUND {
        let p = json!({"to": "mid", "kind": "note", "body": "x"});
        h.call_as(agent("d"), "message.send", p).await.unwrap();
    }
    h.ask("c", "sub").await;

    h.at(60_000).await;

    let all = h.all().await;
    assert_eq!(all.last().unwrap().0, "top");
    assert_eq!(all.last().unwrap().3, json!(33));
}

#[tokio::test]
async fn b16_a_delivered_hop_whose_agent_ends_passes_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    h.ask("c", "sub").await;
    h.becomes("sub", Kind::Idle).await;
    h.becomes("sub", Kind::Done).await;

    let all = h.all().await;
    assert_eq!(all[0].1, "delivered");
    assert_eq!(all[1], row("mid", "pending", Value::Null, json!(1)));
}

#[tokio::test]
async fn b13_message_pass_is_the_hops_receivers_call_on_an_open_hop() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    h.call_as(
        agent("mid"),
        "message.send",
        json!({"to": "d", "kind": "note", "body": "x"}),
    )
    .await
    .unwrap();
    h.ask("c", "sub").await;
    let pass = |who: Actor, id: u32| h.call_as(who, "message.pass", json!({"id": id}));

    assert_eq!(
        pass(Actor::user(), 2).await.unwrap_err().code,
        code::FORBIDDEN
    );
    assert_eq!(pass(agent("c"), 2).await.unwrap_err().code, code::FORBIDDEN);
    assert_eq!(pass(agent("d"), 1).await.unwrap_err().code, code::CONFLICT);
    assert_eq!(
        pass(agent("sub"), 9).await.unwrap_err().code,
        code::NOT_FOUND
    );

    let passed = pass(agent("sub"), 2).await.unwrap();
    assert_eq!(passed["status"], "dropped");
    assert_eq!(passed["reason"], "passed");
    assert_eq!(
        h.all().await[2],
        row("mid", "pending", Value::Null, json!(2))
    );
    assert_eq!(
        pass(agent("sub"), 2).await.unwrap_err().code,
        code::CONFLICT
    );
}

#[tokio::test]
async fn b17_a_note_that_answers_the_current_hop_stops_the_bubble() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    h.ask("c", "sub").await;
    h.becomes("sub", Kind::Idle).await;

    let answer = json!({"to": "c", "kind": "note", "body": "this one", "replyTo": 1});
    h.call_as(agent("sub"), "message.send", answer)
        .await
        .unwrap();
    h.at(600_000).await;

    let all = h.all().await;
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].1, "delivered");
    assert_eq!(all[1].0, "c");
}

#[tokio::test]
async fn b17_a_note_from_an_earlier_hop_or_after_the_landing_stops_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    h.ask("c", "sub").await;
    h.becomes("sub", Kind::Idle).await;
    h.at(60_000).await;

    let late = json!({"to": "c", "kind": "note", "body": "late", "replyTo": 1});
    h.call_as(agent("sub"), "message.send", late).await.unwrap();
    h.at(120_000).await;
    h.at(180_000).await;

    let all = h.all().await;
    assert_eq!(all[4], row("you", "held", json!("escalated"), json!(4)));
}

#[tokio::test]
async fn b15_the_users_reply_delivers_the_landing_and_a_drop_ends_it() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    h.ask("c", "top").await;
    h.at(60_000).await;
    assert_eq!(h.get(2).await["status"], "held");
    let mut events = h.bus.subscribe();

    let deliver = h
        .call_as(Actor::user(), "message.deliver", json!({"id": 2}))
        .await;
    assert_eq!(deliver.unwrap_err().code, code::CONFLICT);
    let reply = json!({"to": "c", "kind": "note", "body": "go left", "replyTo": 2});
    h.call_as(Actor::user(), "message.send", reply)
        .await
        .unwrap();

    assert_eq!(h.get(2).await["status"], "delivered");
    assert_eq!(h.get(2).await["reason"], Value::Null);
    let mut names = vec![];
    while let Ok(event) = events.try_recv() {
        names.push(serde_json::to_value(&event.data).unwrap()["name"].clone());
    }
    assert!(names.contains(&json!("message.delivered")));
}

#[tokio::test]
async fn b15_a_dropped_landing_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    h.ask("c", "top").await;
    h.at(60_000).await;

    h.call_as(Actor::user(), "message.drop", json!({"id": 2}))
        .await
        .unwrap();

    assert_eq!(h.get(2).await["status"], "dropped");
}

#[tokio::test]
async fn b14_after_a_restart_a_hop_whose_bound_ran_out_passes_once() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    h.ask("c", "sub").await;
    let h = h.restarted();

    h.at(60_000).await;
    h.at(60_001).await;

    let all = h.all().await;
    assert_eq!(all.len(), 2);
    assert_eq!(all[1], row("mid", "pending", Value::Null, json!(1)));
}

#[tokio::test]
async fn b14_a_restart_reads_the_bound_from_the_stored_time() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    h.clock.store(10_000, Ordering::SeqCst);
    h.ask("c", "sub").await;
    let h = h.restarted();

    h.at(69_999).await;
    assert_eq!(h.all().await.len(), 1);
    h.at(70_000).await;
    assert_eq!(h.all().await.len(), 2);
}

#[tokio::test]
async fn b17_a_note_to_anyone_but_the_asker_stops_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    h.ask("c", "sub").await;
    h.becomes("sub", Kind::Idle).await;

    let elsewhere = json!({"to": "d", "kind": "note", "body": "fyi", "replyTo": 1});
    h.call_as(agent("sub"), "message.send", elsewhere)
        .await
        .unwrap();
    h.at(60_000).await;

    let all = h.all().await;
    assert!(all.iter().any(|m| m.3 == json!(1)), "{all:?}");
}

#[tokio::test]
async fn b15_a_reply_addressed_elsewhere_does_not_deliver_the_landing() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    h.ask("c", "top").await;
    h.at(60_000).await;
    assert_eq!(h.get(2).await["status"], "held");

    let elsewhere = json!({"to": "d", "kind": "note", "body": "go left", "replyTo": 2});
    h.call_as(Actor::user(), "message.send", elsewhere)
        .await
        .unwrap();

    assert_eq!(h.get(2).await["status"], "held");
}

/// Marks `ids` `idle` on the Rail, with no status event.
fn idle(h: &Hops, ids: &[&str]) {
    for node in h.rail.0.lock().unwrap().iter_mut() {
        if ids.contains(&node.id.as_str()) {
            node.status = Some(Status {
                kind: Kind::Idle,
                label: "x".into(),
                since: 0,
            });
        }
    }
}

/// Waits for the delivery callback to have typed `n` prompts, then lets a stray one show.
async fn typed_after(h: &Hops, n: usize) -> usize {
    for _ in 0..100 {
        if h.typed.lock().unwrap().len() >= n {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    h.typed.lock().unwrap().len()
}

#[tokio::test]
async fn b2_a_message_to_an_already_idle_door_is_typed_without_a_new_status() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    idle(&h, &["sub"]);
    h.ask("c", "sub").await;
    assert_eq!(typed_after(&h, 1).await, 1);
    assert_eq!(h.get(1).await["status"], "delivered");
}

#[tokio::test]
async fn b3_a_released_message_to_an_already_idle_agent_is_typed_without_a_new_status() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    idle(&h, &["d"]);
    let route = json!({"from": "c", "to": "d", "delivery": "ask-first"});
    h.call_as(Actor::user(), "route.set", route).await.unwrap();
    let note = json!({"to": "d", "kind": "note", "body": "hi"});
    h.call_as(agent("c"), "message.send", note).await.unwrap();
    assert_eq!(h.get(1).await["status"], "held");
    h.call_as(Actor::user(), "message.deliver", json!({"id": 1}))
        .await
        .unwrap();
    assert_eq!(typed_after(&h, 1).await, 1);
    assert_eq!(h.get(1).await["status"], "delivered");
}

#[tokio::test]
async fn b6_ending_a_takeover_of_an_already_idle_agent_types_what_it_held() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    idle(&h, &["d"]);
    let take = json!({"agent": "d"});
    h.call_as(Actor::user(), "takeover.begin", take.clone())
        .await
        .unwrap();
    let note = json!({"to": "d", "kind": "note", "body": "hi"});
    h.call_as(agent("c"), "message.send", note).await.unwrap();
    assert_eq!(typed_after(&h, 1).await, 0);
    h.call_as(Actor::user(), "takeover.end", take)
        .await
        .unwrap();
    assert_eq!(typed_after(&h, 1).await, 1);
    assert_eq!(h.get(1).await["status"], "delivered");
}

#[tokio::test]
async fn b13_a_hop_to_an_already_idle_door_is_typed_without_a_new_status() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    idle(&h, &["mid"]);
    h.ask("c", "sub").await;
    h.call_as(agent("sub"), "message.pass", json!({"id": 1}))
        .await
        .unwrap();
    assert_eq!(typed_after(&h, 1).await, 1);
    assert_eq!(h.get(2).await["to"], "mid");
    assert_eq!(h.get(2).await["status"], "delivered");
}

#[tokio::test]
async fn b17_a_kill_between_the_landing_and_the_chain_move_leaves_neither() {
    let dir = tempfile::tempdir().unwrap();
    let h = Hops::new(dir.path());
    h.ask("c", "top").await;
    // The chain move fails, as a kill between the two writes would.
    let db = rusqlite::Connection::open(dir.path().join("messages.db")).unwrap();
    db.execute_batch(
        "CREATE TRIGGER kill BEFORE UPDATE ON chains BEGIN SELECT RAISE(ABORT, 'killed'); END",
    )
    .unwrap();
    h.clock.store(60_000, Ordering::SeqCst);
    assert!(settle(&h.messages.inner, None).await.is_err());
    assert_eq!(h.all().await.len(), 1);
    db.execute_batch("DROP TRIGGER kill").unwrap();
    let h = h.restarted();
    h.at(60_000).await;
    h.at(120_000).await;
    let rows = h.all().await;
    assert_eq!(rows.iter().filter(|r| r.0 == Actor::user().id).count(), 1);
}
