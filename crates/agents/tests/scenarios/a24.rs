//! A24: starting a closed Workstream's Door returns or fails, never hangs.

use std::sync::Arc;
use std::time::Duration;

use contracts::agent::RailNode;
use rpc::code;
use serde_json::json;
use tokio::time::Instant;

use crate::common::Fixture;

const CONVERSATION: &str = "11111111-1111-1111-1111-111111111111";
const BOUND: Duration = Duration::from_millis(10_000);

async fn closed_door(f: &Fixture) -> RailNode {
    let workstream = f.workstream("team", None).await;
    f.call("rail.startDoor", json!({"id": workstream}))
        .await
        .unwrap();
    let attempt = f.attempt(&workstream).await;
    f.call(
        "agent.signal",
        json!({"id": workstream, "attempt": attempt, "payload": {
            "hook_event_name": "SessionStart", "session_id": CONVERSATION, "source": "startup"}}),
    )
    .await
    .unwrap();
    f.call("agent.stop", json!({"id": workstream}))
        .await
        .unwrap();
    f.tree()
        .await
        .into_iter()
        .find(|n| n.id == workstream)
        .unwrap()
}

async fn start(f: &Fixture, id: &str) -> Result<serde_json::Value, rpc::RpcError> {
    f.call("rail.startDoor", json!({"id": id})).await
}

/// While `starting` is pending: `rail.tree` and `agent.stop` for another id are answered.
async fn others_are_answered(f: &Fixture) {
    let other = f.spawn(None, None).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        f.tree().await;
        f.call("agent.stop", json!({"id": other.id})).await.unwrap();
    })
    .await
    .expect("the Daemon answers other ids while a start is pending");
}

async fn acknowledge(f: &Fixture, id: &str, old_attempt: &str) {
    f.until_attempt_changed(id, old_attempt).await;
    let attempt = f.attempt(id).await;
    f.call(
        "agent.signal",
        json!({"id": id, "attempt": attempt, "payload": {
            "hook_event_name": "SessionStart", "session_id": CONVERSATION, "source": "resume"}}),
    )
    .await
    .unwrap();
}

async fn settles_acknowledged(f: Arc<Fixture>, door: &RailNode) {
    let begun = Instant::now();
    let starting = {
        let (f, id) = (Arc::clone(&f), door.id.clone());
        tokio::spawn(async move { start(&f, &id).await })
    };
    f.until_attempt_changed(&door.id, door.attempt.as_deref().unwrap())
        .await;
    others_are_answered(&f).await;
    acknowledge(&f, &door.id, door.attempt.as_deref().unwrap()).await;
    let node: RailNode = serde_json::from_value(starting.await.unwrap().unwrap()).unwrap();
    assert!(node.terminal_id.is_some());
    assert!(begun.elapsed() < BOUND);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a24_start_after_close_in_this_run_settles_within_the_bound() {
    let f = Fixture::running("sleep 30");
    let door = closed_door(&f).await;
    settles_acknowledged(Arc::new(f), &door).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a24_start_after_reopen_settles_within_the_bound() {
    let f = Fixture::running("sleep 30");
    closed_door(&f).await;
    let f = f.reopen();
    let door = f.tree().await.into_iter().find(|n| n.can_resume).unwrap();
    settles_acknowledged(Arc::new(f), &door).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a24_a_start_never_acknowledged_fails_within_the_bound() {
    let f = Arc::new(Fixture::running("sleep 30"));
    let door = closed_door(&f).await;
    let begun = Instant::now();
    let starting = {
        let (f, id) = (Arc::clone(&f), door.id.clone());
        tokio::spawn(async move { start(&f, &id).await })
    };
    f.until_attempt_changed(&door.id, door.attempt.as_deref().unwrap())
        .await;
    others_are_answered(&f).await;

    let err = starting.await.unwrap().unwrap_err();

    assert!(begun.elapsed() < BOUND, "took {:?}", begun.elapsed());
    assert_eq!(err.code, code::CONFLICT);
    assert!(err.message.contains("acknowledg"), "{}", err.message);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a24_an_unresumable_conversation_fails_with_a22s_code() {
    let f = Fixture::running("sleep 30");
    let door = closed_door(&f).await;
    std::fs::remove_file(f.dir.path().join("fake-claude")).unwrap();
    let begun = Instant::now();

    let err = start(&f, &door.id).await.unwrap_err();

    assert!(begun.elapsed() < BOUND);
    assert_ne!(err.code, code::NOT_FOUND);
    assert!(!err.message.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a24_a_failed_start_leaves_no_slot() {
    let f = Fixture::running("sleep 30");
    let door = closed_door(&f).await;
    let bin = f.dir.path().join("fake-claude");
    let saved = std::fs::read(&bin).unwrap();
    std::fs::remove_file(&bin).unwrap();
    start(&f, &door.id).await.unwrap_err();

    let second = start(&f, &door.id).await.unwrap_err();

    assert_ne!(second.code, code::CONFLICT, "{}", second.message);
    std::fs::write(&bin, saved).unwrap();
}
