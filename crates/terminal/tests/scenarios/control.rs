//! X2 input, X5 resize, X6 kill.

use crate::common::{ctx, open, sh, until_exit, until_printed};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use contracts::terminal::{ResizeParams, TerminalId, WriteParams};
use rpc::{Module, code};
use serde_json::json;

#[tokio::test]
async fn x2_input_is_echoed_back_by_cat() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, bus) = open(&dir);
    let ctx = ctx(&dir, &bus);
    let mut spawned = terminals.spawn(sh(dir.path(), "cat")).await.unwrap();
    let write = WriteParams {
        id: spawned.id.clone(),
        data: STANDARD.encode("ping\n"),
    };
    terminals
        .call(&ctx, "terminal.write", serde_json::to_value(write).unwrap())
        .await
        .unwrap();
    until_printed(&mut spawned.events, "ping").await;
    terminals.kill(&spawned.id).await.unwrap();
}

#[tokio::test]
async fn x2_input_that_is_not_base64_is_the_callers_error() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, bus) = open(&dir);
    let spawned = terminals.spawn(sh(dir.path(), "cat")).await.unwrap();
    let write = json!({"id": spawned.id, "data": "***"});
    let err = terminals
        .call(&ctx(&dir, &bus), "terminal.write", write)
        .await
        .unwrap_err();
    assert_eq!(err.code, code::INVALID_PARAMS);
    terminals.kill(&spawned.id).await.unwrap();
}

#[tokio::test]
async fn x5_resize_changes_what_the_program_sees() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, bus) = open(&dir);
    let mut spawned = terminals
        .spawn(sh(dir.path(), "read line; stty size"))
        .await
        .unwrap();
    let resize = ResizeParams {
        id: spawned.id.clone(),
        cols: 100,
        rows: 30,
    };
    let ctx = ctx(&dir, &bus);
    terminals
        .call(
            &ctx,
            "terminal.resize",
            serde_json::to_value(resize).unwrap(),
        )
        .await
        .unwrap();
    terminals.write(&spawned.id, b"\n").await.unwrap();
    let (printed, _) = until_exit(&mut spawned.events).await;
    assert!(printed.contains("30 100"), "{printed}");
}

#[tokio::test]
async fn x6_kill_stops_the_program_and_keeps_it_listed() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, bus) = open(&dir);
    let ctx = ctx(&dir, &bus);
    let mut spawned = terminals.spawn(sh(dir.path(), "cat")).await.unwrap();
    let id = serde_json::to_value(TerminalId {
        id: spawned.id.clone(),
    })
    .unwrap();
    terminals
        .call(&ctx, "terminal.kill", id.clone())
        .await
        .unwrap();
    let (_, exit) = until_exit(&mut spawned.events).await;
    assert_eq!(exit, None);
    let listed = terminals.list();
    assert!(listed.iter().any(|t| t.id == spawned.id && !t.running));
    let err = terminals.call(&ctx, "terminal.kill", id).await.unwrap_err();
    assert_eq!(err.code, code::NOT_FOUND);
}

#[tokio::test]
async fn x6_kill_stops_a_program_that_ignores_hangup() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut spawned = terminals
        .spawn(sh(dir.path(), "trap '' HUP; echo ready; exec cat"))
        .await
        .unwrap();
    until_printed(&mut spawned.events, "ready").await;
    terminals.kill(&spawned.id).await.unwrap();
    let (_, exit) = until_exit(&mut spawned.events).await;
    assert_eq!(exit, None);
}

#[tokio::test]
async fn x5_a_zero_sized_window_is_the_callers_error() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    for (cols, rows) in [(0, 24), (80, 0)] {
        let mut params = sh(dir.path(), "cat");
        (params.cols, params.rows) = (cols, rows);
        let err = terminals.spawn(params).await.err().expect("spawn fails");
        assert_eq!(err.code, code::INVALID_PARAMS);
    }
    let running = terminals.spawn(sh(dir.path(), "cat")).await.unwrap();
    let err = terminals.resize(&running.id, 0, 30).await.unwrap_err();
    assert_eq!(err.code, code::INVALID_PARAMS);
    terminals.kill(&running.id).await.unwrap();
}

#[tokio::test]
async fn x6_an_unknown_terminal_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    for id in ["999"] {
        assert_eq!(terminals.kill(id).await.unwrap_err().code, code::NOT_FOUND);
        assert_eq!(
            terminals.write(id, b"x").await.unwrap_err().code,
            code::NOT_FOUND
        );
        assert_eq!(
            terminals.resize(id, 80, 24).await.unwrap_err().code,
            code::NOT_FOUND
        );
    }
}
