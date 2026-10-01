//! A5 the signal path, and A4's prompt typed at the first idle.

use contracts::agent::StatusEvent;
use contracts::{EventData, Kind};
use rpc::code;
use serde_json::{Value, json};

use crate::common::{Fixture, status_of, until_file};

impl Fixture {
    async fn signal(&self, id: &str, event: &str) -> Result<Value, rpc::RpcError> {
        let payload = json!({"hook_event_name": event, "tool_name": "Bash"});
        self.call("agent.signal", json!({"id": id, "payload": payload}))
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

#[tokio::test]
async fn a5_a_payload_the_adapter_does_not_recognise_is_ignored() {
    let mut f = Fixture::running("sleep 30");
    let node = f.spawn(None, None).await.unwrap();
    f.statuses();

    f.signal(&node.id, "FutureEvent").await.unwrap();
    f.call("agent.signal", json!({"id": node.id, "payload": {}}))
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

    f.signal(&node.id, "UserPromptSubmit").await.unwrap();

    assert!(f.statuses().is_empty());
    assert_eq!(status_of(&f.tree().await, &node.id).kind, Kind::Done);
}

#[tokio::test]
async fn a4_the_prompt_is_typed_at_the_first_idle_and_only_then() {
    let f = Fixture::running(
        "read first; echo \"$first\" > \"$(dirname \"$0\")/typed\"; read second; echo \"$second\" >> \"$(dirname \"$0\")/typed\"; sleep 30",
    );
    let node = f.spawn(None, Some("hello there")).await.unwrap();
    let typed = f.dir.path().join("typed");

    f.signal(&node.id, "UserPromptSubmit").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    assert!(!typed.exists(), "typed before the first idle");

    f.signal(&node.id, "SessionStart").await.unwrap();
    assert_eq!(until_file(&typed).await.trim(), "hello there");

    // A later idle must not type it again: the second read stays unanswered.
    f.signal(&node.id, "UserPromptSubmit").await.unwrap();
    f.signal(&node.id, "Stop").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    assert_eq!(
        std::fs::read_to_string(&typed).unwrap().trim(),
        "hello there"
    );
}

#[tokio::test]
async fn a4_the_prompt_and_enter_arrive_as_separate_reads_a_pause_apart() {
    let script = r#"D=$(dirname "$0"); stty raw -echo
dd bs=100 count=1 of="$D/first" 2>/dev/null
t1=$(perl -MTime::HiRes=time -e 'print time')
dd bs=100 count=1 of="$D/second" 2>/dev/null
t2=$(perl -MTime::HiRes=time -e 'print time')
echo "$t1 $t2" > "$D/times"; sleep 30"#;
    let f = Fixture::running(script);
    let node = f.spawn(None, Some("hello there")).await.unwrap();
    f.signal(&node.id, "SessionStart").await.unwrap();

    let times = until_file(&f.dir.path().join("times")).await;
    let [t1, t2]: [f64; 2] = times
        .split_whitespace()
        .map(|t| t.parse().unwrap())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(f.dir.path().join("first")).unwrap(),
        "hello there"
    );
    assert_eq!(
        std::fs::read_to_string(f.dir.path().join("second")).unwrap(),
        "\r"
    );
    assert!(t2 - t1 >= 0.5, "Enter followed after only {}s", t2 - t1);
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
        json!({"hook_event_name": "StopFailure", "last_assistant_message": "SECRET-MESSAGE"}),
        json!({"prompt": "SECRET-PROMPT"}),
    ] {
        f.call("agent.signal", json!({"id": node.id, "payload": payload}))
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
    assert!(stderr.contains("StopFailure"), "{stderr}");
    assert!(stderr.contains("no event name"), "{stderr}");
    assert!(!stderr.contains("SECRET"), "{stderr}");
}
