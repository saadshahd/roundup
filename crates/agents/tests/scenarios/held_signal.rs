//! A14 a Signal that arrives early is held and applied.

use std::sync::Arc;
use std::time::Duration;

use contracts::{Event, EventData, Kind};
use rpc::{RpcError, code};
use serde_json::{Value, json};
use tokio::sync::broadcast::Receiver;

use crate::common::{Fixture, hold_starts, release, until_file};

const TIMEOUT: Duration = Duration::from_secs(10);

async fn signal(f: &Fixture, id: &str, event: &str) -> Result<Value, RpcError> {
    let payload = json!({"hook_event_name": event, "tool_name": "Bash"});
    f.call(
        "agent.signal",
        json!({"id": id, "incarnation": f.incarnation(id).await, "payload": payload}),
    )
    .await
}

/// The next `n` `agent.status` Kinds announced on `events`.
async fn next_kinds(events: &mut Receiver<Event>, n: usize) -> Vec<Kind> {
    let mut kinds = Vec::new();
    while kinds.len() < n {
        let event = tokio::time::timeout(TIMEOUT, events.recv())
            .await
            .expect("an agent.status in time")
            .unwrap();
        if let EventData::AgentStatus(status) = event.data {
            kinds.push(status.status.kind);
        }
    }
    kinds
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a14_signals_held_while_starting_are_applied_in_arrival_order() {
    let f = Arc::new(Fixture::running("sleep 30"));
    let mut events = f.bus.subscribe();
    let config = hold_starts(&f);
    let spawning = {
        let f = Arc::clone(&f);
        tokio::spawn(async move { f.spawn(None, None).await })
    };
    let tree = f.until(|t| !t.is_empty()).await;
    let id = tree[0].id.clone();

    // Arrival order: working, then needs-you, then idle. The replies are captured, not unwrapped,
    // so a `NOT_FOUND` reply — what a Signal gets if it is rejected instead of held while starting
    // — fails the assertion below instead of leaving this Signal's `spawn_blocking` read stuck on
    // `config` forever (never released) when the test panics.
    let replies = [
        signal(&f, &id, "PreToolUse").await,
        signal(&f, &id, "PermissionRequest").await,
        signal(&f, &id, "Stop").await,
    ];

    release(config).await;
    let node = spawning.await.unwrap().unwrap();
    assert_eq!(node.id, id);

    for reply in &replies {
        assert!(reply.is_ok(), "{reply:?}");
    }
    let kinds = next_kinds(&mut events, 3).await;
    assert_eq!(kinds, [Kind::Working, Kind::NeedsYou, Kind::Idle]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a14_a_held_user_prompt_submit_still_names_the_agent() {
    let f = Arc::new(Fixture::running("sleep 30"));
    let config = hold_starts(&f);
    let spawning = {
        let f = Arc::clone(&f);
        tokio::spawn(async move { f.spawn(None, None).await })
    };
    let tree = f.until(|t| !t.is_empty()).await;
    let id = tree[0].id.clone();

    let payload = json!({"hook_event_name": "UserPromptSubmit", "prompt": "fix the build"});
    let held = f
        .call(
            "agent.signal",
            json!({"id": id, "incarnation": f.incarnation(&id).await, "payload": payload}),
        )
        .await;

    release(config).await;
    let node = spawning.await.unwrap().unwrap();

    assert!(held.is_ok(), "{held:?}");
    let tree = f.tree().await;
    assert_eq!(
        tree.iter().find(|n| n.id == node.id).unwrap().name,
        "fix-build"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a14_a_held_signal_that_leaves_the_agent_idle_still_types_its_spawn_prompt() {
    let f = Arc::new(Fixture::running(
        "read p; echo \"$p\" > \"$(dirname \"$0\")/typed\"; sleep 30",
    ));
    let config = hold_starts(&f);
    let typed = f.dir.path().join("typed");
    let spawning = {
        let f = Arc::clone(&f);
        tokio::spawn(async move { f.spawn(None, Some("hello there")).await })
    };
    let tree = f.until(|t| !t.is_empty()).await;
    let id = tree[0].id.clone();

    // "Stop" leaves the Agent idle (A5), the only Kind that types the spawn prompt (A4).
    let held = signal(&f, &id, "Stop").await;

    release(config).await;
    spawning.await.unwrap().unwrap();

    assert!(held.is_ok(), "{held:?}");
    assert_eq!(until_file(&typed).await.trim(), "hello there");
}

#[tokio::test]
async fn a14_a_signal_for_an_id_that_was_never_spawned_is_still_not_found() {
    let f = Fixture::new();
    let err = signal(&f, "999", "Stop").await.unwrap_err();
    assert_eq!(err.code, code::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a14_a_failed_spawn_discards_the_signals_it_held() {
    let f = Arc::new(Fixture::running("sleep 30"));
    let mut events = f.bus.subscribe();
    let config = hold_starts(&f);
    let spawning = {
        let f = Arc::clone(&f);
        tokio::spawn(async move { f.spawn(None, None).await })
    };
    let tree = f.until(|t| !t.is_empty()).await;
    let id = tree[0].id.clone();

    // Captured, not unwrapped: see the comment in the ordering test above.
    let held = signal(&f, &id, "Stop").await;
    // Invalid JSON makes `trust` fail once it reads this: the spawn fails after the Signal was
    // held.
    tokio::task::spawn_blocking({
        let config = config.clone();
        move || std::fs::write(config, "{ not json")
    })
    .await
    .unwrap()
    .unwrap();

    let err = spawning.await.unwrap().unwrap_err();

    assert_eq!(err.code, code::INTERNAL);
    assert!(f.tree().await.is_empty(), "a failed spawn leaves no Agent");
    assert!(held.is_ok(), "{held:?}");
    let err = signal(&f, &id, "Stop").await.unwrap_err();
    assert_eq!(
        err.code,
        code::NOT_FOUND,
        "nothing was held for a later Agent to inherit at this id"
    );
    assert!(
        std::iter::from_fn(|| events.try_recv().ok())
            .all(|event| !matches!(event.data, EventData::AgentStatus(_))),
        "the discarded Signal's Status is never announced"
    );
}

const NOISY_OVERFLOW: &str = "ROUNDUP_TEST_NOISY_OVERFLOW";
/// Spelled out, not the production bound, so a changed bound fails this test.
const BOUND: usize = 8;
const CYCLE: [&str; 3] = ["PreToolUse", "PermissionRequest", "Stop"];

/// Not a test: with `NOISY_OVERFLOW` set, holds a spawn, sends `BOUND + 2` Signals for it, lets it
/// finish, then prints the Kinds the surviving held Signals produced, so the parent process can
/// read both this (real) stdout and this (real) stderr without the test harness's own capturing
/// (which a held Signal's background task can escape) getting in the way.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn child_overflows_the_held_signals() {
    if std::env::var_os(NOISY_OVERFLOW).is_none() {
        return;
    }
    let f = Arc::new(Fixture::running("sleep 30"));
    let mut events = f.bus.subscribe();
    let config = hold_starts(&f);
    let spawning = {
        let f = Arc::clone(&f);
        tokio::spawn(async move { f.spawn(None, None).await })
    };
    let tree = f.until(|t| !t.is_empty()).await;
    let id = tree[0].id.clone();

    // Captured, not unwrapped immediately (see the comment in the ordering test above): the
    // scenario says every call returns success, including the ninth and later ones that only get
    // dropped from the held queue, so that is checked below rather than here, where a failure
    // would panic while this Signal's `spawn_blocking` read of `config` is still stuck, never
    // released.
    let mut replies = Vec::with_capacity(BOUND + 2);
    for i in 0..BOUND + 2 {
        replies.push(signal(&f, &id, CYCLE[i % CYCLE.len()]).await);
    }

    release(config).await;
    spawning.await.unwrap().unwrap();

    for reply in &replies {
        assert!(reply.is_ok(), "{reply:?}");
    }

    // A short timeout, not `TIMEOUT`: `next_kinds` bounds each `recv` by `TIMEOUT` (10s) and
    // panics past it, so a missing event would otherwise take up to `BOUND * TIMEOUT` to surface
    // on this output pipe; this fails the subprocess in 2s instead.
    let kinds = tokio::time::timeout(Duration::from_secs(2), next_kinds(&mut events, BOUND))
        .await
        .unwrap_or_default();
    println!(
        "KINDS={}",
        kinds
            .iter()
            .map(|kind| format!("{kind:?}"))
            .collect::<Vec<_>>()
            .join(",")
    );
}

#[tokio::test]
async fn a14_past_the_bound_the_oldest_held_signal_is_dropped_and_logged() {
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "held_signal::child_overflows_the_held_signals",
            "--nocapture",
        ])
        .env(NOISY_OVERFLOW, "1")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");

    // The two oldest (PreToolUse=working, PermissionRequest=needs-you) are dropped; the surviving
    // 8 start at the third Signal sent (Stop=idle) and keep cycling.
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout
        .lines()
        .find_map(|line| line.strip_prefix("KINDS="))
        .unwrap_or_else(|| panic!("no KINDS= line in: {stdout}"));
    let kinds: Vec<&str> = line.split(',').collect();
    assert_eq!(
        kinds,
        [
            "Idle", "Working", "NeedsYou", "Idle", "Working", "NeedsYou", "Idle", "Working"
        ]
    );
    assert_eq!(
        stderr.matches("agents: dropping the oldest").count(),
        2,
        "{stderr}"
    );
}
