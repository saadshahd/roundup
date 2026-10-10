//! X8 in-process API: subscribe to one Terminal, in order, without a socket.

use std::time::Duration;

use crate::common::{open, sh, until_exit};
use contracts::EventData;
use rpc::code;
use tokio::sync::broadcast::error::RecvError;

/// How long a Terminal may go without any event before its reader counts as blocked.
const STALL: Duration = Duration::from_secs(5);

#[tokio::test]
async fn x8_a_late_subscriber_sees_output_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let spawned = terminals
        .spawn(sh(dir.path(), "read go; seq 1 300"))
        .await
        .unwrap();
    let mut events = terminals.subscribe(&spawned.id).unwrap();
    terminals.write(&spawned.id, b"\n").await.unwrap();
    let (printed, _) = until_exit(&mut events).await;
    let numbers: Vec<u32> = printed
        .lines()
        .filter_map(|line| line.trim().parse().ok())
        .collect();
    assert_eq!(numbers, (1..=300).collect::<Vec<_>>());
}

#[tokio::test]
async fn x8_resize_is_usable_without_a_socket() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut spawned = terminals
        .spawn(sh(dir.path(), "read line; stty size"))
        .await
        .unwrap();
    terminals.resize(&spawned.id, 100, 30).await.unwrap();
    terminals.write(&spawned.id, b"\n").await.unwrap();
    let (printed, _) = until_exit(&mut spawned.events).await;
    assert!(printed.contains("30 100"), "{printed}");
}

#[tokio::test]
async fn x8_subscribing_to_an_unknown_terminal_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let err = terminals.subscribe("999").unwrap_err();
    assert_eq!(err.code, code::NOT_FOUND);
}

#[tokio::test]
async fn x8_subscribing_to_an_exited_terminal_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut spawned = terminals.spawn(sh(dir.path(), "true")).await.unwrap();
    until_exit(&mut spawned.events).await;
    let err = terminals.subscribe(&spawned.id).unwrap_err();
    assert_eq!(err.code, code::NOT_FOUND);
}

#[tokio::test]
async fn x8_a_slow_subscriber_never_blocks_the_reader() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut slow = terminals
        .spawn(sh(dir.path(), "yes x | head -c 8500000"))
        .await
        .unwrap();
    let mut draining = terminals.subscribe(&slow.id).unwrap();
    // Each event carries at most 8192 bytes and the backlog is 1024 events, so 8.5 MB overflows it
    // however the kernel chunks the reads (Linux hands out about 4 KiB, macOS about 700 bytes).
    // `slow` is not read until the program has exited. A blocked reader stops the draining
    // subscriber too, so "no event for STALL" proves the block however slow the machine is.
    loop {
        match tokio::time::timeout(STALL, draining.recv()).await {
            Err(_) => panic!("the reader was blocked: no event for {STALL:?}"),
            Ok(Ok(EventData::TerminalExited(_))) => break,
            Ok(Ok(_) | Err(RecvError::Lagged(_))) => {}
            Ok(Err(RecvError::Closed)) => panic!("events closed before the exit"),
        }
    }
    assert!(terminals.list().iter().all(|t| !t.running));
    assert!(matches!(
        slow.events.recv().await,
        Err(RecvError::Lagged(_))
    ));
    let last = std::iter::from_fn(|| slow.events.try_recv().ok()).last();
    assert!(matches!(last, Some(EventData::TerminalExited(_))));
}
