//! H8: `rup permission <agent-id>` fails loud, never allows and never exits 2.

#[path = "common/served.rs"]
mod common;

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use common::Served;
use serde_json::json;

fn permission(socket: Option<&Path>, agent_id: &str, stdin: &str) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rup"));
    command.env_remove("RUPD_SOCKET");
    if let Some(socket) = socket {
        command.env("RUPD_SOCKET", socket);
    }
    let mut rup = command
        .args(["permission", agent_id])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _ = rup.stdin.take().unwrap().write_all(stdin.as_bytes());
    rup.wait_with_output().unwrap()
}

fn assert_fails_loud(out: &Output, cause: &str) {
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(out.stdout.is_empty(), "stdout must stay empty");
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.contains(cause), "{stderr}");
}

#[test]
fn h8_an_unset_socket_exits_1_naming_the_cause() {
    let out = permission(None, "1", r#"{"tool_name":"Bash"}"#);
    assert_fails_loud(&out, "RUPD_SOCKET");
}

#[test]
fn h8_no_daemon_listening_exits_1() {
    let nowhere = tempfile::tempdir().unwrap().path().join("missing.sock");
    let out = permission(Some(&nowhere), "1", r#"{"tool_name":"Bash"}"#);
    assert_fails_loud(&out, "");
}

#[test]
fn h8_a_payload_that_is_not_json_exits_1() {
    let nowhere = tempfile::tempdir().unwrap().path().join("missing.sock");
    let out = permission(Some(&nowhere), "1", "not json");
    assert_fails_loud(&out, "payload");
}

#[tokio::test(flavor = "multi_thread")]
async fn h8_an_unknown_agent_exits_1_naming_it() {
    let served = Served::start("#!/bin/sh\nsleep 30\n", &[]).await;
    let out = permission(Some(&served.socket()), "999", r#"{"tool_name":"Bash"}"#);
    assert_fails_loud(&out, "agent 999");
}

#[test]
fn h8_a_daemon_that_closes_without_answering_exits_1_rupd_did_not_answer() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("closing.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            // Read the request, then hang up without a reply.
            let mut line = String::new();
            let mut stream = std::io::BufReader::new(stream.unwrap());
            let _ = std::io::BufRead::read_line(&mut stream, &mut line);
        }
    });
    let out = permission(Some(&socket), "1", r#"{"tool_name":"Bash"}"#);
    assert_fails_loud(&out, "rupd did not answer");
}

#[test]
fn h8_a_wrong_number_of_arguments_exits_1_not_2() {
    let out = Command::new(env!("CARGO_BIN_EXE_rup"))
        .args(["permission"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn h8_an_unreadable_payload_opens_an_unanswerable_decision() {
    let served = Served::start("#!/bin/sh\nsleep 30\n", &[]).await;
    let client = rpc::Client::connect(&served.socket()).await.unwrap();
    let cwd = served.dir.path().to_string_lossy().into_owned();
    let agent = client
        .request(
            "agent.spawn",
            json!({"cwd": cwd, "prompt": null, "parent": null}),
        )
        .await
        .unwrap();
    let id = agent["id"].as_str().unwrap();

    let out = permission(Some(&served.socket()), id, "{}");

    assert!(out.status.success());
    assert!(out.stdout.is_empty());
    let open = client.request("decision.list", json!(null)).await.unwrap();
    assert_eq!(open[0]["answerable"], json!(false));
    assert_eq!(open[0]["agent"], json!(id));
}
