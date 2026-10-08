//! A5 the signal path.

use contracts::agent::StatusEvent;
use contracts::{EventData, Kind};
use rpc::{Module, code};
use serde_json::{Value, json};

use crate::common::{Fixture, status_of};

impl Fixture {
    async fn signal(&self, id: &str, event: &str) -> Result<Value, rpc::RpcError> {
        let payload = json!({"hook_event_name": event, "tool_name": "Bash"});
        self.call(
            "agent.signal",
            json!({"id": id, "attempt": self.attempt(id).await, "payload": payload}),
        )
        .await
    }

    fn statuses(&mut self) -> Vec<StatusEvent> {
        std::iter::from_fn(|| self.events.try_recv().ok())
            .filter_map(|event| match event.data {
                EventData::AgentStatus(status) => Some(status),
                _ => None,
            })
            .collect()
    }
}

#[tokio::test]
async fn a5_a_signal_updates_the_status_and_announces_it_once() {
    let mut f = Fixture::running("sleep 30");
    let node = f.spawn(None, None).await.unwrap();
    f.statuses();

    f.signal(&node.id, "Stop").await.unwrap();
    f.signal(&node.id, "Stop").await.unwrap();

    let announced = f.statuses();
    assert_eq!(announced.len(), 1);
    assert_eq!(
        (announced[0].id.as_str(), announced[0].status.kind),
        (node.id.as_str(), Kind::Idle)
    );
    assert_eq!(status_of(&f.tree().await, &node.id).kind, Kind::Idle);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a5_status_events_arrive_in_the_order_the_status_changed() {
    let f = std::sync::Arc::new(Fixture::running("sleep 30"));
    let node = f.spawn(None, None).await.unwrap();
    let mut events = f.bus.subscribe();
    // A thread of its own: a signalling task never yields, so a reader task could starve and lag.
    let reader = std::thread::spawn(move || {
        let mut kinds = vec![];
        loop {
            match events.blocking_recv().expect("the reader keeps up").data {
                EventData::AgentStatus(status) => kinds.push(status.status.kind),
                EventData::RailChanged => return kinds,
                _ => {}
            }
        }
    });
    let signallers: Vec<_> = (0..4)
        .map(|_| {
            let (f, id) = (std::sync::Arc::clone(&f), node.id.clone());
            tokio::spawn(async move {
                // One call context per task keeps each call short, so the tasks contend.
                let ctx = f.ctx();
                for event in ["Stop", "UserPromptSubmit"].repeat(SIGNALS / 2) {
                    let payload = json!({"hook_event_name": event});
                    let params =
                        json!({"id": id, "attempt": f.attempt(&id).await, "payload": payload});
                    f.agents.call(&ctx, "agent.signal", params).await.unwrap();
                }
            })
        })
        .collect();
    for signaller in signallers {
        signaller.await.unwrap();
    }
    f.bus.emit(contracts::Actor::user(), EventData::RailChanged);

    let kinds = reader.join().unwrap();
    // Every announced change alternates idle and working, so two equal neighbours mean two
    // announcements crossed.
    let crossed = kinds.windows(2).filter(|pair| pair[0] == pair[1]).count();
    assert_eq!(
        crossed,
        0,
        "{crossed} crossed pairs in {} events",
        kinds.len()
    );
    assert_eq!(
        kinds.last(),
        Some(&status_of(&f.tree().await, &node.id).kind)
    );
}

/// Per signalling task in the ordering test. Four tasks announce at most 4 x 250 = 1000 changes,
/// fewer than the Bus holds (1024), so the reader cannot lag however late it is scheduled.
const SIGNALS: usize = 250;

#[tokio::test]
async fn a5_a_payload_the_adapter_does_not_recognise_is_ignored() {
    let mut f = Fixture::running("sleep 30");
    let node = f.spawn(None, None).await.unwrap();
    f.statuses();

    f.signal(&node.id, "FutureEvent").await.unwrap();
    f.call(
        "agent.signal",
        json!({"id": node.id, "attempt": node.attempt, "payload": {}}),
    )
    .await
    .unwrap();

    assert!(f.statuses().is_empty());
    assert_eq!(status_of(&f.tree().await, &node.id).kind, Kind::Working);
}

#[tokio::test]
async fn a5_a_signal_for_an_unknown_agent_is_not_found() {
    let f = Fixture::new();
    let err = f.signal("999", "Stop").await.unwrap_err();
    assert_eq!(err.code, code::NOT_FOUND);
}

#[tokio::test]
async fn a3_a_signal_after_exit_is_ignored() {
    let mut f = Fixture::running("exit 0");
    let node = f.spawn(None, None).await.unwrap();
    f.until(|t| status_of(t, &node.id).kind == Kind::Done).await;
    f.statuses();

    assert_eq!(
        f.signal(&node.id, "UserPromptSubmit")
            .await
            .unwrap_err()
            .code,
        code::NOT_FOUND
    );

    assert!(f.statuses().is_empty());
    assert_eq!(status_of(&f.tree().await, &node.id).kind, Kind::Done);
}

const NOISY_CHILD: &str = "ROUNDUP_TEST_NOISY_CHILD";

/// Not a test: with `NOISY_CHILD` set, sends payloads an adapter must refuse, so the parent can
/// read what this process wrote to stderr.
#[tokio::test]
async fn child_signals_payloads_the_adapter_refuses() {
    if std::env::var_os(NOISY_CHILD).is_none() {
        return;
    }
    let f = Fixture::running("sleep 30");
    let node = f.spawn(None, None).await.unwrap();
    for payload in [
        json!({"hook_event_name": "FutureEvent", "prompt": "SECRET-PROMPT"}),
        json!({"prompt": "SECRET-PROMPT"}),
    ] {
        f.call(
            "agent.signal",
            json!({"id": node.id, "attempt": node.attempt, "payload": payload}),
        )
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn a5_refused_payloads_are_logged_to_stderr_by_event_name_never_by_content() {
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "signal::child_signals_payloads_the_adapter_refuses",
            "--nocapture",
        ])
        .env(NOISY_CHILD, "1")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    assert!(stderr.contains("FutureEvent"), "{stderr}");
    assert!(stderr.contains("no event name"), "{stderr}");
    assert!(!stderr.contains("SECRET"), "{stderr}");
}
