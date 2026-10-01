//! A4: `rup hook <agent-id>` turns one hook payload on stdin into `agent.signal` as that Agent.
//!
//! The Daemon runs in a child process (this test binary re-run on `serve_for_the_parent`) because
//! only a process of its own can be given a fake `claude` through the environment.

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::time::Duration;

use contracts::{ActorKind, EventData, Kind};
use serde_json::json;

const SERVE_ENV: &str = "ROUNDUP_TEST_SERVE_DIR";

/// Not a test: with `SERVE_ENV` set this process becomes a Daemon until it is killed.
#[tokio::test]
async fn serve_for_the_parent() {
    let Some(dir) = std::env::var_os(SERVE_ENV) else {
        return;
    };
    let dir = Path::new(&dir);
    let daemon = rupd::Daemon::open(&dir.join(".roundup")).unwrap();
    let listener = tokio::net::UnixListener::bind(dir.join("rupd.sock")).unwrap();
    rupd::serve(listener, std::sync::Arc::new(daemon))
        .await
        .unwrap();
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
            .env("CLAUDE_CONFIG_DIR", dir.path())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let served = Self { dir, child };
        for _ in 0..500 {
            if rpc::Client::connect(&served.socket()).await.is_ok() {
                return served;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("the Daemon never listened");
    }

    fn socket(&self) -> std::path::PathBuf {
        self.dir.path().join("rupd.sock")
    }

    /// Run `rup hook <agent_id>` with `stdin` on its standard input.
    fn hook(&self, agent_id: &str, stdin: &str) -> Output {
        let mut rup = Command::new(env!("CARGO_BIN_EXE_rup"))
            .args(["hook", agent_id])
            .env("RUPD_SOCKET", self.socket())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        rup.stdin
            .take()
            .unwrap()
            .write_all(stdin.as_bytes())
            .unwrap();
        rup.wait_with_output().unwrap()
    }
}

impl Drop for Served {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a4_hook_signals_the_agent_as_that_agent() {
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

    let out = served.hook(id, r#"{"hook_event_name":"Stop"}"#);

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
async fn a4_hook_for_an_unknown_agent_fails_loudly() {
    let served = Served::start().await;
    let out = served.hook("999", r#"{"hook_event_name":"Stop"}"#);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("agent 999"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a4_hook_rejects_a_payload_that_is_not_json() {
    let served = Served::start().await;
    let out = served.hook("1", "not json");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("payload"));
}
