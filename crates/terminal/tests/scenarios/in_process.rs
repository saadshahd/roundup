//! X8 in-process API: subscribe to one Terminal, in order, without a socket.

use crate::common::{open, sh, until_exit};
use contracts::EventData;
use rpc::code;
use tokio::sync::broadcast::error::RecvError;

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
async fn x8_subscribing_to_an_unknown_terminal_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let err = terminals.subscribe("999").unwrap_err();
    assert_eq!(err.code, code::NOT_FOUND);
}

#[tokio::test]
async fn x8_a_slow_subscriber_never_blocks_the_reader() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut slow = terminals
        .spawn(sh(dir.path(), "yes x | head -c 6000000"))
        .await
        .unwrap();
    // `slow` is not read until the program has finished and been listed as exited.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while terminals.list().iter().any(|t| t.running) {
        assert!(
            std::time::Instant::now() < deadline,
            "the reader was blocked"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(matches!(
        slow.events.recv().await,
        Err(RecvError::Lagged(_))
    ));
    let last = std::iter::from_fn(|| slow.events.try_recv().ok()).last();
    assert!(matches!(last, Some(EventData::TerminalExited(_))));
}
