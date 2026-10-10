//! H13 the Interrupt, H17 its lost acknowledgement. The fake `claude` puts its Terminal in raw
//! mode, records every byte written to it and, for the acknowledging one, answers the first byte
//! with the star title. The bound is named by each test and the clock is injected and frozen.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use contracts::{Actor, ActorKind, Kind};
use rpc::{Ctx, Module, RpcError, code};
use serde_json::{Value, json};

use crate::common::{Fixture, status_of, until_file};

/// Raw before anything is written; the first byte is kept, then the star title is set, then
/// everything after is recorded.
const ACKS: &str = "stty raw -echo; echo 1 > \"$(dirname \"$0\")/ready\"; \
    dd bs=1 count=1 > \"$(dirname \"$0\")/typed\" 2>/dev/null; \
    printf '\\033]0;\\342\\234\\263 Claude Code\\007'; \
    cat >> \"$(dirname \"$0\")/typed\"";
/// The same Terminal, ignoring the Esc: no title follows it.
const IGNORES: &str = "stty raw -echo; echo 1 > \"$(dirname \"$0\")/ready\"; \
    cat > \"$(dirname \"$0\")/typed\"";
/// Ignores the Esc, and sets the star title once the test creates `go`.
const LATE: &str = "stty raw -echo; echo 1 > \"$(dirname \"$0\")/ready\"; \
    while [ ! -f \"$(dirname \"$0\")/go\" ]; do sleep 0.05; done; \
    printf '\\033]0;\\342\\234\\263 Claude Code\\007'; sleep 30";
const BOUND: Duration = Duration::from_millis(400);

fn typed(f: &Fixture) -> PathBuf {
    f.dir.path().join("typed")
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_default()
}

async fn signal(f: &Fixture, id: &str, event: &str) {
    let payload = json!({"hook_event_name": event, "tool_name": "Bash"});
    let attempt = f.attempt(id).await;
    f.call(
        "agent.signal",
        json!({"id": id, "attempt": attempt, "payload": payload}),
    )
    .await
    .unwrap();
}

/// A `working` Agent whose Terminal is raw, on a frozen clock.
async fn working(script: &str, bound: Duration) -> (Arc<Fixture>, String) {
    let mut f = Fixture::running(script);
    f.agents = f.agents.with_clock(|| 0).with_interrupt_bound(bound);
    let f = Arc::new(f);
    let node = f.spawn(None, None).await.unwrap();
    until_file(&f.dir.path().join("ready")).await;
    signal(&f, &node.id, "UserPromptSubmit").await;
    (f, node.id)
}

fn interrupt(f: &Arc<Fixture>, id: &str) -> tokio::task::JoinHandle<Result<Value, RpcError>> {
    let (f, id) = (Arc::clone(f), id.to_owned());
    tokio::spawn(async move { f.call("agent.interrupt", json!({"id": id})).await })
}

#[tokio::test]
async fn h13_an_interrupt_writes_one_esc_and_returns_when_the_title_shows_the_star() {
    let (f, id) = working(ACKS, Duration::from_secs(30)).await;
    assert_eq!(status_of(&f.tree().await, &id).kind, Kind::Working);

    let reply = interrupt(&f, &id).await.unwrap().unwrap();
    assert_eq!(reply, Value::Null);
    assert_eq!(status_of(&f.tree().await, &id).kind, Kind::Idle);
    assert_eq!(read(&typed(&f)), b"\x1b");
}

#[tokio::test]
async fn h13_an_agent_that_is_not_working_is_not_running_and_no_byte_is_written() {
    let (f, id) = working(ACKS, BOUND).await;
    signal(&f, &id, "Stop").await;
    let err = interrupt(&f, &id).await.unwrap().unwrap_err();
    assert_eq!(err.code, code::NOT_RUNNING);

    let ended = Arc::new(Fixture::running("exit 0"));
    let node = ended.spawn(None, None).await.unwrap();
    ended
        .until(|tree| status_of(tree, &node.id).kind == Kind::Done)
        .await;
    let err = interrupt(&ended, &node.id).await.unwrap().unwrap_err();
    assert!(
        matches!(err.code, code::NOT_RUNNING | code::NOT_FOUND),
        "{err}"
    );

    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(read(&typed(&f)), b"");
}

#[tokio::test]
async fn h13_only_the_user_or_a_doors_own_workstream_may_interrupt() {
    let (f, id) = working(ACKS, BOUND).await;
    let stranger = Ctx {
        actor: Actor {
            kind: ActorKind::Agent,
            id: "999".into(),
            parent: None,
        },
        ..f.ctx()
    };
    let err = f
        .agents
        .call(&stranger, "agent.interrupt", json!({"id": id}))
        .await
        .unwrap_err();
    assert_eq!(err.code, code::FORBIDDEN);
    assert_eq!(read(&typed(&f)), b"");
}

#[tokio::test]
async fn h13_an_interrupt_of_an_agent_with_an_open_decision_clears_it() {
    let (f, id) = working(ACKS, Duration::from_secs(30)).await;
    let hook = {
        let (f, id) = (Arc::clone(&f), id.clone());
        tokio::spawn(async move {
            f.call(
                "agent.permission",
                json!({"id": id, "payload": {"hook_event_name": "PermissionRequest",
                    "tool_name": "Bash", "tool_input": {"command": "ls"}}}),
            )
            .await
        })
    };
    f.until(|tree| status_of(tree, &id).kind == Kind::NeedsYou)
        .await;

    // Esc on the dialog closes the hook's connection: H7(b) folds `Dismissed`, the
    // acknowledgement of this Interrupt.
    let call = interrupt(&f, &id);
    for _ in 0..500 {
        if read(&typed(&f)) == b"\x1b" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    hook.abort();
    let _ = hook.await;
    assert_eq!(call.await.unwrap().unwrap(), Value::Null);
    assert_eq!(status_of(&f.tree().await, &id).kind, Kind::Idle);
    let open = f.call("decision.list", Value::Null).await.unwrap();
    assert_eq!(open, json!([]));
}

#[tokio::test]
async fn h17_a_lost_acknowledgement_is_not_acked_and_nothing_else_is_written() {
    let (f, id) = working(IGNORES, BOUND).await;
    let err = interrupt(&f, &id).await.unwrap().unwrap_err();

    assert_eq!(err.code, code::NOT_ACKED);
    assert!(err.message.contains(&id), "{}", err.message);
    assert_eq!(status_of(&f.tree().await, &id).kind, Kind::Working);
    tokio::time::sleep(BOUND).await;
    assert_eq!(read(&typed(&f)), b"\x1b", "a second byte was written");
}

#[tokio::test]
async fn h17_a_later_star_is_still_folded_as_a2_says() {
    let (f, id) = working(LATE, BOUND).await;
    interrupt(&f, &id).await.unwrap().unwrap_err();
    assert_eq!(status_of(&f.tree().await, &id).kind, Kind::Working);

    std::fs::write(f.dir.path().join("go"), "").unwrap();
    f.until(|tree| status_of(tree, &id).kind == Kind::Idle)
        .await;
}
