//! H11 the Steer, H12 the first prompt through it, H16 no other keystroke. The fake `claude` puts
//! its Terminal in raw mode and records every byte written to it; the test plays its hooks with
//! `agent.signal`. The clock is injected and never advances: nothing here waits on it.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use contracts::{Actor, ActorKind, Kind};
use rpc::{Ctx, Module, RpcError, code};
use serde_json::{Value, json};

use crate::common::{Fixture, status_of, until_file};

/// Raw before anything is written, or the Terminal would turn `\r` into `\n`: `ready` says so.
const RECORD: &str =
    "stty raw -echo; echo 1 > \"$(dirname \"$0\")/ready\"; cat > \"$(dirname \"$0\")/typed\"";
const BOUND: Duration = Duration::from_millis(400);

fn paste(text: &str) -> String {
    format!("\x1b[200~{text}\x1b[201~\r")
}

fn frozen(f: Fixture, bound: Duration) -> Fixture {
    let mut f = f;
    f.agents = f.agents.with_clock(|| 0).with_steer_bound(bound);
    f
}

async fn signal(f: &Fixture, id: &str, event: &str) {
    let payload = json!({"hook_event_name": event, "prompt": "typed by the user"});
    let attempt = f.attempt(id).await;
    f.call(
        "agent.signal",
        json!({"id": id, "attempt": attempt, "payload": payload}),
    )
    .await
    .unwrap();
}

fn typed(f: &Fixture) -> PathBuf {
    f.dir.path().join("typed")
}

fn read(path: &Path) -> String {
    std::fs::read(path)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default()
}

/// The recorded bytes, once there are `len` of them.
async fn until_typed(f: &Fixture, len: usize) -> String {
    for _ in 0..500 {
        let text = read(&typed(f));
        if text.len() >= len {
            return text;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("{len} bytes were never typed: {:?}", read(&typed(f)));
}

/// An Agent whose `SessionStart` has arrived, so it is `idle`.
async fn idle(bound: Duration) -> (Arc<Fixture>, String) {
    let f = Arc::new(frozen(Fixture::running(RECORD), bound));
    let node = f.spawn(None, None).await.unwrap();
    until_file(&f.dir.path().join("ready")).await;
    signal(&f, &node.id, "SessionStart").await;
    (f, node.id)
}

fn steer(
    f: &Arc<Fixture>,
    id: &str,
    text: &str,
) -> tokio::task::JoinHandle<Result<Value, RpcError>> {
    let (f, id, text) = (Arc::clone(f), id.to_owned(), text.to_owned());
    tokio::spawn(async move {
        f.call("agent.prompt", json!({"id": id, "text": text}))
            .await
    })
}

#[tokio::test]
async fn h11_a_steer_is_one_bracketed_paste_and_returns_when_the_agent_submits_it() {
    let (f, id) = idle(Duration::from_secs(30)).await;
    let call = steer(&f, &id, "run the tests");

    let text = paste("run the tests");
    assert_eq!(until_typed(&f, text.len()).await, text);
    assert!(!call.is_finished(), "returned before UserPromptSubmit");
    signal(&f, &id, "UserPromptSubmit").await;
    assert_eq!(call.await.unwrap().unwrap(), Value::Null);
    assert_eq!(read(&typed(&f)), text);
}

#[tokio::test]
async fn h11_the_in_process_path_is_the_one_the_rpc_method_uses() {
    let (f, id) = idle(Duration::from_secs(30)).await;
    let waiting = {
        let (f, id) = (Arc::clone(&f), id.clone());
        tokio::spawn(async move { f.agents.prompt(&id, "hi").await })
    };
    until_typed(&f, paste("hi").len()).await;
    signal(&f, &id, "UserPromptSubmit").await;
    assert_eq!(waiting.await.unwrap(), Ok(()));
}

#[tokio::test]
async fn h11_text_cannot_close_the_paste_early() {
    let (f, id) = idle(Duration::from_secs(30)).await;
    let call = steer(&f, &id, "a\x1b[201~rm -rf b");
    let text = paste("a[201~rm -rf b");
    assert_eq!(until_typed(&f, text.len()).await, text);
    signal(&f, &id, "UserPromptSubmit").await;
    call.await.unwrap().unwrap();
}

#[tokio::test]
async fn h11_a_lost_submit_is_not_accepted_and_nothing_is_sent_again() {
    let (f, id) = idle(BOUND).await;
    let err = steer(&f, &id, "hello").await.unwrap().unwrap_err();

    assert_eq!(err.code, code::NOT_ACCEPTED);
    assert!(err.message.contains(&id), "{}", err.message);
    tokio::time::sleep(BOUND).await;
    assert_eq!(read(&typed(&f)), paste("hello"));
    assert_eq!(status_of(&f.tree().await, &id).kind, Kind::Idle);
}

#[tokio::test]
async fn h11_an_agent_that_is_not_idle_is_busy_and_nothing_is_written() {
    let f = Arc::new(frozen(Fixture::running(RECORD), BOUND));
    let node = f.spawn(None, None).await.unwrap();

    let err = steer(&f, &node.id, "early").await.unwrap().unwrap_err();
    assert_eq!(err.code, code::BUSY);
    assert!(err.message.contains("Working"), "{}", err.message);

    signal(&f, &node.id, "UserPromptSubmit").await;
    let err = steer(&f, &node.id, "mid-turn").await.unwrap().unwrap_err();
    assert_eq!(err.code, code::BUSY);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(read(&typed(&f)), "");
}

#[tokio::test]
async fn h11_a_second_steer_while_one_waits_is_busy() {
    let (f, id) = idle(Duration::from_secs(30)).await;
    let first = steer(&f, &id, "one");
    until_typed(&f, paste("one").len()).await;

    let err = steer(&f, &id, "two").await.unwrap().unwrap_err();
    assert_eq!(err.code, code::BUSY);
    signal(&f, &id, "UserPromptSubmit").await;
    first.await.unwrap().unwrap();
    assert_eq!(read(&typed(&f)), paste("one"));
}

#[tokio::test]
async fn h11_an_agent_that_is_gone_or_never_was_is_not_found() {
    let f = Arc::new(frozen(Fixture::running("exit 0"), BOUND));
    let node = f.spawn(None, None).await.unwrap();
    f.until(|tree| status_of(tree, &node.id).kind == Kind::Done)
        .await;

    for id in [node.id.as_str(), "999"] {
        let err = steer(&f, id, "hello").await.unwrap().unwrap_err();
        assert_eq!(err.code, code::NOT_FOUND, "{id}");
    }
}

#[tokio::test]
async fn h11_an_agent_that_ends_while_a_steer_waits_is_not_found() {
    let (f, id) = idle(Duration::from_secs(30)).await;
    let call = steer(&f, &id, "hello");
    until_typed(&f, paste("hello").len()).await;
    f.call("agent.stop", json!({"id": id})).await.unwrap();
    assert_eq!(call.await.unwrap().unwrap_err().code, code::NOT_FOUND);
}

#[tokio::test]
async fn h11_only_the_user_or_a_doors_own_room_may_steer() {
    let f = Arc::new(frozen(Fixture::running(RECORD), Duration::from_secs(30)));
    let room = f.room("team", None).await;
    let child = f.spawn(Some(&room), None).await.unwrap();
    until_file(&f.dir.path().join("ready")).await;
    signal(&f, &child.id, "SessionStart").await;

    let from = |kind, id: &str| Ctx {
        actor: Actor {
            kind,
            id: id.into(),
            parent: None,
        },
        ..f.ctx()
    };
    let params = json!({"id": child.id, "text": "go"});
    for stranger in [from(ActorKind::Agent, "999"), from(ActorKind::Ext, &room)] {
        let err = f
            .agents
            .call(&stranger, "agent.prompt", params.clone())
            .await
            .unwrap_err();
        assert_eq!(err.code, code::FORBIDDEN);
    }
    assert_eq!(read(&typed(&f)), "");

    let door = from(ActorKind::Agent, &room);
    let call = {
        let f = Arc::clone(&f);
        tokio::spawn(async move { f.agents.call(&door, "agent.prompt", params).await })
    };
    until_typed(&f, paste("go").len()).await;
    signal(&f, &child.id, "UserPromptSubmit").await;
    assert_eq!(call.await.unwrap().unwrap(), Value::Null);
}

#[tokio::test]
async fn h12_the_first_prompt_is_steered_at_the_first_session_start_and_not_before() {
    let f = frozen(Fixture::running(RECORD), Duration::from_secs(30));
    let node = f.spawn(None, Some("build it")).await.unwrap();
    until_file(&f.dir.path().join("ready")).await;

    signal(&f, &node.id, "UserPromptSubmit").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        read(&typed(&f)),
        "",
        "written before the first SessionStart"
    );

    signal(&f, &node.id, "SessionStart").await;
    let text = paste("build it");
    assert_eq!(until_typed(&f, text.len()).await, text);
    signal(&f, &node.id, "UserPromptSubmit").await;
    signal(&f, &node.id, "Stop").await;
    signal(&f, &node.id, "SessionStart").await;

    // Once: a later idle does not send it again.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(read(&typed(&f)), text);
    assert_eq!(status_of(&f.tree().await, &node.id).kind, Kind::Idle);
}

#[tokio::test]
async fn h12_a_first_prompt_that_is_not_accepted_leaves_the_agent_in_error() {
    let f = frozen(Fixture::running(RECORD), BOUND);
    let node = f.spawn(None, Some("build it")).await.unwrap();
    until_file(&f.dir.path().join("ready")).await;
    signal(&f, &node.id, "SessionStart").await;

    let tree = f
        .until(|tree| status_of(tree, &node.id).kind == Kind::Error)
        .await;
    assert_eq!(status_of(&tree, &node.id).label, "prompt not accepted");
    assert_eq!(read(&typed(&f)), paste("build it"));
}

#[tokio::test]
async fn h16_no_signal_text_reaches_the_terminal_and_only_one_function_writes_to_it() {
    let (f, id) = idle(BOUND).await;
    for event in ["UserPromptSubmit", "PermissionRequest", "Stop"] {
        signal(&f, &id, event).await;
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(read(&typed(&f)), "", "a Signal became input");

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut callers = vec![];
    for dir in ["agents/src", "rupd/src"] {
        collect(&root.join(dir), &mut callers);
    }
    // H11's write and H13's; the Takeover path joins this list in its own series.
    assert_eq!(callers, ["agents/src/lib.rs: 2"]);
}

/// `terminals.write(` in the code under `dir`, tests excluded, per file.
fn collect(dir: &Path, found: &mut Vec<String>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::path);
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, found);
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        // Everything after `#[cfg(test)]` is test code; rustfmt may split a call over lines.
        let code = text.split("#[cfg(test)]").next().unwrap_or_default();
        let squeezed: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        let count = squeezed.matches("terminals.write(").count();
        if count > 0 {
            let name = path.strip_prefix(dir.parent().unwrap().parent().unwrap());
            found.push(format!("{}: {count}", name.unwrap().display()));
        }
    }
}
