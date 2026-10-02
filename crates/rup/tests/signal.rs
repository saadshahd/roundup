//! A4: `rup signal <agent-id>` turns one hook payload on stdin into `agent.signal` as that Agent.
//! It never exits 2, which Claude Code reads as "block": every failure is exit 1 and a message.
//!
//! The Daemon runs in a child process (this test binary re-run on `serve_for_the_parent`) because
//! only a process of its own can be given a fake `claude` through the environment.

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use contracts::{ActorKind, EventData, Kind};
use serde_json::json;

#[path = "support/ready.rs"]
mod ready;
mod support;

const SERVE_ENV: &str = "ROUNDUP_TEST_SERVE_DIR";
/// Claude Code kills a command hook after this (the `timeout` roundup gives it in the settings).
const HOOK_TIMEOUT: Duration = Duration::from_secs(5);

/// Not a test: with `SERVE_ENV` set this process becomes a Daemon until it is killed.
#[tokio::test]
async fn serve_for_the_parent() {
    let Some(dir) = std::env::var_os(SERVE_ENV) else {
        return;
    };
    let dir = Path::new(&dir);
    let (daemon, listener) = support::open_and_bind(dir, &dir.join("rupd.sock"));
    rupd::serve(listener, daemon).await.unwrap();
}

/// A Daemon child process that is killed when this drops.
struct Served {
    dir: tempfile::TempDir,
    child: Child,
}

impl Served {
    async fn start() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("fake-claude");
        std::fs::write(&fake, "#!/bin/sh\nsleep 30\n").unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "serve_for_the_parent"])
            .env(SERVE_ENV, dir.path())
            .env("ROUNDUP_CLAUDE_BIN", &fake)
            .env("ROUNDUP_RUP_BIN", env!("CARGO_BIN_EXE_rup"))
            .env("CLAUDE_CONFIG_DIR", dir.path())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let served = Self { dir, child };
        ready::wait_for_ping(&served.socket()).await;
        served
    }

    fn socket(&self) -> std::path::PathBuf {
        self.dir.path().join("rupd.sock")
    }

    /// Run `rup signal <agent_id>` against this Daemon.
    fn signal(&self, agent_id: &str, stdin: &str) -> Output {
        rup(&self.socket(), &["signal", agent_id], stdin)
    }
}

/// Run `rup <args>` with `stdin` on its standard input, pointed at `socket`. A run that outlasts
/// Claude Code's hook timeout fails the test.
fn rup(socket: &Path, args: &[&str], stdin: &str) -> Output {
    let mut rup = Command::new(env!("CARGO_BIN_EXE_rup"))
        .args(args)
        .env("RUPD_SOCKET", socket)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // A rup that exits before reading (bad arguments) closes the pipe; that is not a test error.
    let _ = rup.stdin.take().unwrap().write_all(stdin.as_bytes());
    let deadline = Instant::now() + HOOK_TIMEOUT;
    while rup.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            let _ = rup.kill();
            panic!("rup {args:?} outlasted Claude Code's hook timeout");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    rup.wait_with_output().unwrap()
}

impl Drop for Served {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a4_signal_signals_the_agent_as_that_agent() {
    let served = Served::start().await;
    let mut client = rpc::Client::connect(&served.socket()).await.unwrap();
    client
        .request("events.subscribe", json!(null))
        .await
        .unwrap();
    let cwd = served.dir.path().to_string_lossy().into_owned();
    let agent = client
        .request(
            "agent.spawn",
            json!({"cwd": cwd, "prompt": null, "parent": null}),
        )
        .await
        .unwrap();
    let id = agent["id"].as_str().unwrap();

    let out = served.signal(id, r#"{"hook_event_name":"Stop"}"#);

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let event = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let event = client.next_event().await.expect("events stay open");
            if let EventData::AgentStatus(status) = event.data {
                return (event.actor, status);
            }
        }
    })
    .await
    .expect("agent.status arrives");
    let (actor, status) = event;
    assert_eq!((actor.kind, actor.id.as_str()), (ActorKind::Agent, id));
    assert_eq!((status.id.as_str(), status.status.kind), (id, Kind::Idle));
}

#[tokio::test(flavor = "multi_thread")]
async fn a4_signal_for_an_unknown_agent_fails_loudly() {
    let served = Served::start().await;
    let out = served.signal("999", r#"{"hook_event_name":"Stop"}"#);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("agent 999"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a4_signal_rejects_a_payload_that_is_not_json() {
    let served = Served::start().await;
    let out = served.signal("1", "not json");
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("payload"));
}

#[test]
fn a4_signal_with_no_daemon_exits_1_with_a_message() {
    let nowhere = tempfile::tempdir().unwrap().path().join("missing.sock");
    let out = rup(&nowhere, &["signal", "1"], r#"{"hook_event_name":"Stop"}"#);
    assert_eq!(out.status.code(), Some(1));
    assert!(!out.stderr.is_empty());
}

#[test]
fn a4_signal_to_a_daemon_that_never_answers_exits_1_in_time() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("silent.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    std::thread::spawn(move || {
        // Accepted and kept open, never answered.
        let mut held = vec![];
        for stream in listener.incoming() {
            held.push(stream);
        }
    });

    let started = Instant::now();
    let out = rup(&socket, &["signal", "1"], r#"{"hook_event_name":"Stop"}"#);

    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("may or may not"), "{stderr}");
    // Well inside the hook timeout, so a slow Daemon never costs Claude Code its hook.
    assert!(
        started.elapsed() < HOOK_TIMEOUT / 2,
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn a4_signal_to_a_daemon_that_closes_mid_call_says_the_signal_may_not_have_arrived() {
    use std::io::{BufRead, BufReader};
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("closing.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut lines = BufReader::new(&stream).lines();
        let identify: serde_json::Value =
            serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
        let reply = json!({"jsonrpc": "2.0", "id": identify["id"], "result": null});
        (&stream)
            .write_all(format!("{reply}\n").as_bytes())
            .unwrap();
        // The agent.signal request is read, then the connection drops unanswered.
        lines.next().unwrap().unwrap();
    });

    let out = rup(&socket, &["signal", "1"], r#"{"hook_event_name":"Stop"}"#);

    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("may or may not have arrived"), "{stderr}");
}

#[test]
fn a4_signal_with_the_wrong_number_of_arguments_exits_1_not_2() {
    let nowhere = tempfile::tempdir().unwrap().path().join("missing.sock");
    for args in [&["signal"][..], &["signal", "1", "extra"]] {
        let out = rup(&nowhere, args, "{}");
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("usage"),
            "{args:?}"
        );
    }
}
