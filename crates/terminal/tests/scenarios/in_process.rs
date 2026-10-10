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

/// U174: a write reaches the pty, and `cat` echoes it back, within one frame at p95.
#[tokio::test]
async fn u174_a_write_reaches_the_pty_within_a_frame() {
    const FRAME: Duration = Duration::from_millis(16);
    const START: &[u8] = b"\x1b[200~";
    const END: &[u8] = b"\x1b[201~";
    for bracketed in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let (terminals, _) = open(&dir);
        let spawned = terminals.spawn(sh(dir.path(), "cat")).await.unwrap();
        let mut events = terminals.subscribe(&spawned.id).unwrap();
        let mut waits = Vec::new();
        for i in 0..100 {
            // A unique name per write, so output queued by earlier writes never satisfies this one.
            let needle = format!("file{i}.png");
            let mut bytes = Vec::new();
            if bracketed {
                bytes.extend_from_slice(START);
            }
            bytes.extend_from_slice(format!("'/p/it'\\''s a {needle}' ").as_bytes());
            if bracketed {
                bytes.extend_from_slice(END);
            }
            // A newline ends each canonical line, so `cat` consumes it and the tty never accumulates
            // the 4 KiB line limit across the 100 writes.
            bytes.push(b'\n');
            let sent = std::time::Instant::now();
            terminals.write(&spawned.id, &bytes).await.unwrap();
            // The tty echoes the input back as it is read by the line discipline; control bytes show as ^[.
            crate::common::until_printed(&mut events, &needle).await;
            waits.push(sent.elapsed());
        }
        waits.sort();
        let p95 = waits[94];
        assert!(
            p95 <= FRAME,
            "p95 {p95:?} over a frame (bracketed {bracketed})"
        );
        terminals.kill(&spawned.id).await.unwrap();
    }
}
