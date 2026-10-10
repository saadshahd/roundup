//! B28: one failure fails one operation. `Env::fault` stands in for a Store that panics or errs
//! while an operation holds the lock.

use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

use contracts::agent::StatusEvent;
use contracts::{Actor, EventData, Kind, Status};
use serde_json::{Value, json};

use super::*;
use super::{FakeRail, agent_node};

/// What the next call of one operation does when it reaches the lock.
#[derive(Clone, Copy)]
enum Fail {
    Panic,
    Err,
}

struct Faulty {
    messages: Messages,
    bus: Bus,
    touches: Arc<provenance::Touches>,
    typed: Arc<StdMutex<Vec<(String, String)>>>,
    armed: Arc<StdMutex<Option<(&'static str, Fail)>>>,
    calls: Arc<AtomicUsize>,
    clock: Arc<AtomicI64>,
}

impl Faulty {
    fn new(dir: &Path) -> Self {
        Self::over(dir, vec![agent_node("a", Kind::Working)])
    }

    fn over(dir: &Path, nodes: Vec<RailNode>) -> Self {
        let bus = Bus::new();
        let touches = Arc::new(provenance::Touches::in_memory().unwrap());
        let typed: Arc<StdMutex<Vec<(String, String)>>> = Arc::default();
        let seen = Arc::clone(&typed);
        let deliver: Deliver = Arc::new(move |id, text| {
            seen.lock().unwrap().push((id, text));
            Box::pin(async { Ok(()) })
        });
        let armed: Arc<StdMutex<Option<(&'static str, Fail)>>> = Arc::default();
        let calls = Arc::new(AtomicUsize::new(0));
        let (trigger, counted) = (Arc::clone(&armed), Arc::clone(&calls));
        let clock: Arc<AtomicI64> = Arc::default();
        let reads = Arc::clone(&clock);
        let env = Env {
            touches: Arc::clone(&touches),
            clock: Arc::new(move || reads.load(Ordering::SeqCst)),
            fault: Arc::new(move |op| {
                if op == "clock" {
                    counted.fetch_add(1, Ordering::SeqCst);
                }
                let mut armed = lock(&trigger);
                match armed.filter(|(name, _)| *name == op) {
                    Some((_, Fail::Panic)) => {
                        *armed = None;
                        drop(armed);
                        panic!("injected panic in {op}");
                    }
                    Some((_, Fail::Err)) => {
                        *armed = None;
                        Err(format!("injected error in {op}"))
                    }
                    None => Ok(()),
                }
            }),
        };
        let rail = FakeRail::new(nodes);
        let messages = Messages::open_with(dir, bus.clone(), rail, deliver, env).unwrap();
        Self {
            messages,
            bus,
            touches,
            typed,
            armed,
            calls,
            clock,
        }
    }

    fn arm(&self, op: &'static str, fail: Fail) {
        *self.armed.lock().unwrap() = Some((op, fail));
    }

    async fn call(&self, method: &str, p: Value) -> Result<Value, RpcError> {
        let ctx = Ctx {
            actor: Actor::user(),
            bus: self.bus.clone(),
            touches: Arc::clone(&self.touches),
        };
        self.messages.call(&ctx, method, p).await
    }

    fn becomes_idle(&self, revision: &str) {
        let status = Status {
            kind: Kind::Idle,
            label: "x".into(),
            since: 0,
        };
        self.bus.emit(
            Actor::daemon(),
            EventData::AgentStatus(StatusEvent {
                id: "a".into(),
                attempt: "1".into(),
                status_revision: revision.into(),
                status,
            }),
        );
    }
}

fn send() -> Value {
    json!({"to": "a", "kind": "note", "body": "hi"})
}

#[tokio::test]
async fn b28_a_panic_in_a_locked_operation_leaves_the_next_calls_working() {
    let dir = tempfile::tempdir().unwrap();
    let h = Faulty::new(dir.path());

    h.arm("message.send", Fail::Panic);
    let err = h.call("message.send", send()).await.unwrap_err();
    assert_eq!(err.code, code::INTERNAL);
    assert!(err.message.contains("injected panic in message.send"));
    assert!(!err.message.contains("poisoned"));

    let sent = h.call("message.send", send()).await.unwrap();
    h.call("message.get", json!({"id": sent["id"]}))
        .await
        .unwrap();
    h.call(
        "route.set",
        json!({"from": "user", "to": "a", "delivery": "auto"}),
    )
    .await
    .unwrap();
    h.call("takeover.begin", json!({"agent": "a"}))
        .await
        .unwrap();
}

#[tokio::test]
async fn b28_a_failed_operation_changes_nothing_listed() {
    let dir = tempfile::tempdir().unwrap();
    let h = Faulty::new(dir.path());
    h.call("message.send", send()).await.unwrap();
    let mut events = h.bus.subscribe();
    let before = (
        h.call("message.list", json!({})).await.unwrap(),
        h.call("route.list", json!({})).await.unwrap(),
    );

    for (op, fail) in [("message.send", Fail::Panic), ("route.set", Fail::Err)] {
        h.arm(op, fail);
        let params = if op == "route.set" {
            json!({"from": "user", "to": "a", "delivery": "drop"})
        } else {
            send()
        };
        assert_eq!(h.call(op, params).await.unwrap_err().code, code::INTERNAL);
    }

    let after = (
        h.call("message.list", json!({})).await.unwrap(),
        h.call("route.list", json!({})).await.unwrap(),
    );
    assert_eq!(before, after);
    assert!(events.try_recv().is_err());
}

#[tokio::test]
async fn b28_a_transaction_open_at_a_panic_rolls_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&dir.path().join("messages.db")).unwrap();
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = store.atomically(|_| -> Result<(), RpcError> { panic!("mid-transaction") });
    }));
    assert!(panicked.is_err());
    store.atomically(|_| Ok(())).unwrap();
}

#[tokio::test]
async fn b28_a_failure_in_on_status_or_settle_does_not_stop_delivery_or_the_clock() {
    let dir = tempfile::tempdir().unwrap();
    let h = Faulty::new(dir.path());
    h.call("message.send", send()).await.unwrap();

    h.arm("on_status", Fail::Panic);
    h.becomes_idle("2");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(h.typed.lock().unwrap().is_empty());

    h.arm("settle", Fail::Panic);
    h.becomes_idle("3");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert_eq!(h.typed.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn b28_a_failure_in_the_clock_does_not_stop_it() {
    let dir = tempfile::tempdir().unwrap();
    let h = Faulty::new(dir.path());
    h.call("route.list", json!({})).await.unwrap();

    h.arm("clock", Fail::Panic);
    let before = h.calls.load(Ordering::SeqCst);
    tokio::time::sleep(std::time::Duration::from_millis(1_600)).await;
    assert!(h.calls.load(Ordering::SeqCst) >= before + 3);

    // The pass after the failed one still works: a hop passes at 60 000 ms, not at 59 999.
    let dir = tempfile::tempdir().unwrap();
    let h = Faulty::over(dir.path(), super::b_hop_tests::tree());
    let ask = json!({"to": "sub", "kind": "question", "body": "which?"});
    let ctx = Ctx {
        actor: Actor::user(),
        bus: h.bus.clone(),
        touches: Arc::clone(&h.touches),
    };
    let actor = super::agent("c");
    let ctx = Ctx { actor, ..ctx };
    h.messages.call(&ctx, "message.send", ask).await.unwrap();
    h.arm("clock", Fail::Panic);
    h.clock.store(59_999, Ordering::SeqCst);
    tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;
    assert_eq!(
        h.call("message.list", json!({}))
            .await
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    h.clock.store(60_000, Ordering::SeqCst);
    tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;
    assert_eq!(
        h.call("message.list", json!({}))
            .await
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
}
