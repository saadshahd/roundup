//! X1 output, X3 exit, X7 cwd and env.

use crate::common::{PATIENCE, ctx, decode, open, sh, until_exit};
use contracts::EventData;
use contracts::terminal::{SpawnParams, TerminalId, TerminalInfo};
use rpc::Module;

#[tokio::test]
async fn x1_output_reaches_a_subscribed_client() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, bus) = open(&dir);
    let mut events = bus.subscribe();
    let ctx = ctx(&dir, &bus);
    let params = serde_json::to_value(sh(dir.path(), "echo hi")).unwrap();
    let spawned = terminals
        .call(&ctx, "terminal.spawn", params)
        .await
        .unwrap();
    let id = serde_json::from_value::<TerminalId>(spawned).unwrap().id;
    let mut printed = String::new();
    while !printed.contains("hi") {
        let event = tokio::time::timeout(PATIENCE, events.recv())
            .await
            .expect("output in time")
            .unwrap();
        if let EventData::TerminalOutput(out) = event.data {
            assert_eq!(out.id, id);
            printed.push_str(&decode(&out.data));
        }
    }
}

#[tokio::test]
async fn x3_exit_code_is_reported_and_listed() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut spawned = terminals.spawn(sh(dir.path(), "exit 3")).await.unwrap();
    let (_, code) = until_exit(&mut spawned.events).await;
    assert_eq!(code, Some(3));
    let listed = terminals.list();
    let info: &TerminalInfo = listed.iter().find(|t| t.id == spawned.id).unwrap();
    assert!(!info.running);
    assert_eq!(info.exit_code, Some(3));
}

#[tokio::test]
async fn x3_terminal_list_over_rpc_shows_the_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, bus) = open(&dir);
    let ctx = ctx(&dir, &bus);
    let mut spawned = terminals.spawn(sh(dir.path(), "exit 3")).await.unwrap();
    until_exit(&mut spawned.events).await;
    let listed = terminals
        .call(&ctx, "terminal.list", serde_json::Value::Null)
        .await
        .unwrap();
    let listed: Vec<TerminalInfo> = serde_json::from_value(listed).unwrap();
    assert_eq!((listed[0].running, listed[0].exit_code), (false, Some(3)));
}

#[tokio::test]
async fn x3_a_program_killed_by_a_signal_has_no_code() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut spawned = terminals.spawn(sh(dir.path(), "kill -9 $$")).await.unwrap();
    let (_, code) = until_exit(&mut spawned.events).await;
    assert_eq!(code, None);
}

#[tokio::test]
async fn x7_program_runs_in_cwd_with_env_and_inherits_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut params: SpawnParams = sh(
        dir.path(),
        "pwd -P; printf 'v=%s\\n' \"$ROUNDUP_X7\"; printf 'p=%s\\n' \"$PATH\"",
    );
    params.env.insert("ROUNDUP_X7".into(), "set".into());
    let mut spawned = terminals.spawn(params).await.unwrap();
    let (printed, _) = until_exit(&mut spawned.events).await;
    let cwd = dir.path().canonicalize().unwrap();
    assert!(printed.contains(cwd.to_str().unwrap()), "{printed}");
    assert!(printed.contains("v=set"), "{printed}");
    assert!(
        printed.contains(&format!("p={}", std::env::var("PATH").unwrap())),
        "{printed}"
    );
}

#[tokio::test]
async fn x7_a_missing_cwd_is_the_callers_error() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let params = sh(&dir.path().join("absent"), "true");
    let err = terminals.spawn(params).await.err().expect("spawn fails");
    assert_eq!(err.code, rpc::code::INVALID_PARAMS);
}

#[tokio::test]
async fn x7_an_argv_or_env_the_os_cannot_take_is_the_callers_error() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mutations: [fn(&mut SpawnParams); 5] = [
        |p| p.command = Some(vec![String::new()]),
        |p| p.command = Some(vec!["/bin/sh".into(), "a\0b".into()]),
        |p| _ = p.env.insert("A\0B".into(), "v".into()),
        |p| _ = p.env.insert("A=B".into(), "v".into()),
        |p| _ = p.env.insert("A".into(), "v\0".into()),
    ];
    for mutate in mutations {
        let mut params = sh(dir.path(), "true");
        mutate(&mut params);
        let err = terminals.spawn(params).await.err().expect("spawn fails");
        assert_eq!(err.code, rpc::code::INVALID_PARAMS, "{}", err.message);
    }
}
