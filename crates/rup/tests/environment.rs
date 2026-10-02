//! A15: a clean environment for every program the Daemon starts.

#[path = "common/served.rs"]
mod common;

use std::collections::BTreeMap;
use std::time::Duration;

use common::Served;
use contracts::EventData;
use contracts::agent::{RailNode, SpawnParams as AgentSpawnParams};
use contracts::terminal::SpawnParams as TerminalSpawnParams;
use serde_json::json;

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

/// A Daemon with every marker and the user's own Claude Code configuration in its own
/// environment, as A15's test calls for; `extra_env` is added on top for a case's own needs.
async fn served_with_markers(fake_claude: &str, extra_env: &[(&str, &str)]) -> Served {
    let mut env: Vec<(&str, &str)> = MARKERS.iter().map(|marker| (*marker, "1")).collect();
    env.push(("CLAUDE_CODE_OAUTH_TOKEN", "a-users-oauth-token"));
    env.push(("CLAUDE_CODE_USE_BEDROCK", "1"));
    env.extend_from_slice(extra_env);
    Served::start(fake_claude, &env).await
}

fn parse_env(text: &str) -> BTreeMap<&str, &str> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .collect()
}

/// What A15 requires of a spawned program's environment: none of `MARKERS`, but the Claude Code
/// configuration `served_with_markers` set and `PATH`, which the Daemon inherits either way.
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

/// Spawn a Terminal with `env` that writes its environment to a file, wait for it to exit, and
/// return what it saw there.
async fn terminal_env(served: &Served, env: BTreeMap<String, String>) -> String {
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
        env,
        cols: 80,
        rows: 24,
    };
    let spawned = client.request("terminal.spawn", params).await.unwrap();
    let id = spawned["id"].as_str().unwrap().to_owned();

    wait_for_exit(&mut client, &id).await;

    std::fs::read_to_string(&out).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn a15_terminal_spawn_gives_the_program_a_clean_environment() {
    let served = served_with_markers("#!/bin/sh\nsleep 30\n", &[]).await;
    let printed = terminal_env(&served, BTreeMap::new()).await;
    assert_clean(&parse_env(&printed));
}

/// A15 says a marker is stripped "however it reached the Daemon": a caller can also hand one to
/// `terminal.spawn` through `params.env` directly, with none of it in the Daemon's own environment.
#[tokio::test(flavor = "multi_thread")]
async fn a15_terminal_spawn_removes_a_marker_passed_in_its_own_env_param() {
    let served = Served::start("#!/bin/sh\nsleep 30\n", &[]).await;
    let env = MARKERS
        .iter()
        .map(|marker| (marker.to_string(), "1".to_string()))
        .collect();

    let printed = terminal_env(&served, env).await;

    let printed = parse_env(&printed);
    for marker in MARKERS {
        assert!(
            !printed.contains_key(marker),
            "{marker} leaked: {printed:?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a15_agent_spawn_gives_the_program_a_clean_environment() {
    let out_dir = tempfile::tempdir().unwrap();
    let out = out_dir.path().join("env.out");
    let out_var = out.to_string_lossy().into_owned();
    let served = served_with_markers(
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
