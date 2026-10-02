//! A15: a clean environment for every program the Daemon starts.
//!
//! The Daemon runs in a child process (this test binary re-run on `serve_for_the_parent`, as
//! `crates/rup/tests/signal.rs` does) because only a process of its own can be given the markers
//! and the fake `claude` these tests need in its environment.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use contracts::EventData;
use contracts::agent::{RailNode, SpawnParams as AgentSpawnParams};
use contracts::terminal::SpawnParams as TerminalSpawnParams;
use serde_json::json;

const SERVE_ENV: &str = "ROUNDUP_TEST_SERVE_DIR";

/// Env vars Claude Code sets when the program running it is itself inside a Claude Code run; A15
/// says a program the Daemon starts must never see any of them, however they reached the Daemon.
const MARKERS: [&str; 8] = [
    "CLAUDECODE",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
];

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

/// A Daemon child process that is killed when this drops, with every marker and the user's own
/// Claude Code configuration in its own environment, as A15's test calls for.
struct Served {
    dir: tempfile::TempDir,
    child: Child,
}

impl Served {
    /// `fake_claude` is the script `ROUNDUP_CLAUDE_BIN` points the Daemon at.
    async fn start(fake_claude: &str, extra_env: &[(&str, &str)]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("fake-claude");
        std::fs::write(&fake, fake_claude).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut env: Vec<(&str, &str)> = MARKERS.iter().map(|marker| (*marker, "1")).collect();
        env.push(("CLAUDE_CODE_OAUTH_TOKEN", "a-users-oauth-token"));
        env.push(("CLAUDE_CODE_USE_BEDROCK", "1"));
        env.extend_from_slice(extra_env);
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "serve_for_the_parent"])
            .env(SERVE_ENV, dir.path())
            .env("ROUNDUP_CLAUDE_BIN", &fake)
            .env("ROUNDUP_RUP_BIN", env!("CARGO_BIN_EXE_rup"))
            .env("CLAUDE_CONFIG_DIR", dir.path())
            .envs(env)
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
}

impl Drop for Served {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Parse `env`'s `NAME=VALUE` lines into a lookup.
fn parse_env(text: &str) -> BTreeMap<&str, &str> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .collect()
}

/// What A15 requires of a spawned program's environment: none of `MARKERS`, but the Claude Code
/// configuration `Served::start` set and `PATH`, which the Daemon inherits either way.
fn assert_clean(printed: &BTreeMap<&str, &str>) {
    for marker in MARKERS {
        assert!(
            !printed.contains_key(marker),
            "{marker} leaked: {printed:?}"
        );
    }
    assert_eq!(
        printed.get("CLAUDE_CODE_OAUTH_TOKEN"),
        Some(&"a-users-oauth-token"),
        "{printed:?}"
    );
    assert_eq!(
        printed.get("CLAUDE_CODE_USE_BEDROCK"),
        Some(&"1"),
        "{printed:?}"
    );
    assert!(printed.contains_key("PATH"), "{printed:?}");
}

/// The next `terminal.exited` for `id`, over `client`'s event stream (subscribed by the caller).
async fn wait_for_exit(client: &mut rpc::Client, id: &str) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let event = client.next_event().await.expect("events stay open");
            if let EventData::TerminalExited(exited) = event.data
                && exited.id == id
            {
                return;
            }
        }
    })
    .await
    .expect("the program exits in time");
}

#[tokio::test(flavor = "multi_thread")]
async fn a15_terminal_spawn_gives_the_program_a_clean_environment() {
    let served = Served::start("#!/bin/sh\nsleep 30\n", &[]).await;
    let mut client = rpc::Client::connect(&served.socket()).await.unwrap();
    client
        .request("events.subscribe", json!(null))
        .await
        .unwrap();

    let out = served.dir.path().join("env.out");
    let params = TerminalSpawnParams {
        cwd: served.dir.path().to_string_lossy().into_owned(),
        command: Some(vec![
            "/bin/sh".into(),
            "-c".into(),
            format!("env > {}", out.display()),
        ]),
        env: BTreeMap::new(),
        cols: 80,
        rows: 24,
    };
    let spawned = client.request("terminal.spawn", params).await.unwrap();
    let id = spawned["id"].as_str().unwrap().to_owned();

    wait_for_exit(&mut client, &id).await;

    let printed = std::fs::read_to_string(&out).unwrap();
    assert_clean(&parse_env(&printed));
}

#[tokio::test(flavor = "multi_thread")]
async fn a15_agent_spawn_gives_the_program_a_clean_environment() {
    let out_dir = tempfile::tempdir().unwrap();
    let out = out_dir.path().join("env.out");
    let out_var = out.to_string_lossy().into_owned();
    let served = Served::start(
        "#!/bin/sh\nenv > \"$ROUNDUP_TEST_ENV_OUT\"\n",
        &[("ROUNDUP_TEST_ENV_OUT", &out_var)],
    )
    .await;
    let mut client = rpc::Client::connect(&served.socket()).await.unwrap();
    client
        .request("events.subscribe", json!(null))
        .await
        .unwrap();

    let node = client
        .request(
            "agent.spawn",
            AgentSpawnParams {
                cwd: served.dir.path().to_string_lossy().into_owned(),
                prompt: None,
                parent: None,
            },
        )
        .await
        .unwrap();
    let node: RailNode = serde_json::from_value(node).unwrap();
    let terminal_id = node.terminal_id.expect("agent.spawn attaches a Terminal");

    wait_for_exit(&mut client, &terminal_id).await;

    let printed = std::fs::read_to_string(&out).unwrap();
    assert_clean(&parse_env(&printed));
}
