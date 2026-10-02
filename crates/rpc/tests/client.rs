//! C1: a Client whose connection drops reports a third outcome, `UNKNOWN_OUTCOME`, distinct from
//! a request that never reached the Daemon at all (`INTERNAL`). Fake server on a temp socket, no
//! Daemon, no network beyond the local Unix socket.

use std::io::BufRead;
use std::os::unix::net::UnixListener;
use std::time::Duration;

use rpc::{Client, code};
use serde_json::Value;

fn socket_in(dir: &tempfile::TempDir) -> std::path::PathBuf {
    dir.path().join("rupd.sock")
}

#[tokio::test(flavor = "multi_thread")]
async fn c1_a_connection_that_drops_after_the_request_is_written_is_unknown_outcome() {
    let dir = tempfile::tempdir().unwrap();
    let socket = socket_in(&dir);
    let listener = UnixListener::bind(&socket).unwrap();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        // Reading the line proves the request was written; dropping `stream` here (no reply)
        // is the connection closing before the reply the scenario describes.
        std::io::BufReader::new(&stream)
            .lines()
            .next()
            .unwrap()
            .unwrap();
    });

    let client = Client::connect(&socket).await.unwrap();
    let err = tokio::time::timeout(
        Duration::from_secs(5),
        client.request("daemon.ping", Value::Null),
    )
    .await
    .expect("a dropped connection must not hang the caller")
    .unwrap_err();

    assert_eq!(err.code, code::UNKNOWN_OUTCOME);
    assert!(err.message.contains("may have run"), "{}", err.message);
}

#[tokio::test(flavor = "multi_thread")]
async fn c1_every_other_pending_call_fails_the_same_way_and_none_hangs() {
    let dir = tempfile::tempdir().unwrap();
    let socket = socket_in(&dir);
    let listener = UnixListener::bind(&socket).unwrap();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut lines = std::io::BufReader::new(&stream).lines();
        // Both request lines are read (both were written), then the connection drops
        // before either gets a reply.
        lines.next().unwrap().unwrap();
        lines.next().unwrap().unwrap();
    });

    let client = Client::connect(&socket).await.unwrap();
    let (a, b) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(
            client.request("daemon.ping", Value::Null),
            client.request("daemon.ping", Value::Null),
        )
    })
    .await
    .expect("no pending call should hang after the connection drops");

    assert_eq!(a.unwrap_err().code, code::UNKNOWN_OUTCOME);
    assert_eq!(b.unwrap_err().code, code::UNKNOWN_OUTCOME);
}

#[tokio::test(flavor = "multi_thread")]
async fn c1_every_other_pending_call_gets_the_same_message_too() {
    let dir = tempfile::tempdir().unwrap();
    let socket = socket_in(&dir);
    let listener = UnixListener::bind(&socket).unwrap();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut lines = std::io::BufReader::new(&stream).lines();
        // Both request lines are read (both were written), then the connection drops
        // before either gets a reply.
        lines.next().unwrap().unwrap();
        lines.next().unwrap().unwrap();
    });

    let client = Client::connect(&socket).await.unwrap();
    let (a, b) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(
            client.request("daemon.ping", Value::Null),
            client.request("daemon.ping", Value::Null),
        )
    })
    .await
    .expect("no pending call should hang after the connection drops");

    assert!(a.unwrap_err().message.contains("may have run"));
    assert!(b.unwrap_err().message.contains("may have run"));
}

#[tokio::test(flavor = "multi_thread")]
async fn c1_a_request_made_after_the_connection_is_known_closed_fails_at_once_and_says_not_sent() {
    let dir = tempfile::tempdir().unwrap();
    let socket = socket_in(&dir);
    let listener = UnixListener::bind(&socket).unwrap();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        std::io::BufReader::new(&stream)
            .lines()
            .next()
            .unwrap()
            .unwrap();
    });

    let client = Client::connect(&socket).await.unwrap();
    // Arrange: this first call only resolves once the read side has observed the close and
    // marked the connection closed, which is the bound the second call's claim needs.
    client
        .request("daemon.ping", Value::Null)
        .await
        .unwrap_err();

    let second = client
        .request("daemon.ping", Value::Null)
        .await
        .unwrap_err();

    assert_eq!(second.code, code::INTERNAL);
    assert!(second.message.contains("not sent"), "{}", second.message);
}

#[tokio::test(flavor = "multi_thread")]
async fn c1_a_request_after_the_connection_is_known_closed_does_not_hang_even_if_the_write_would_land()
 {
    let dir = tempfile::tempdir().unwrap();
    let socket = socket_in(&dir);
    let listener = UnixListener::bind(&socket).unwrap();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        std::io::BufReader::new(&stream)
            .lines()
            .next()
            .unwrap()
            .unwrap();
        // Half-close: the client's read side sees EOF (so `closed` is set), but the write side
        // stays open, so a write the Client still attempts would land in the kernel buffer
        // instead of failing outright, and only the closed-connection check stops it from
        // waiting forever for a reply that read_loop has already stopped listening for.
        stream.shutdown(std::net::Shutdown::Write).unwrap();
        let _ = std::io::BufReader::new(&stream).lines().next();
    });

    let client = Client::connect(&socket).await.unwrap();
    client
        .request("daemon.ping", Value::Null)
        .await
        .unwrap_err();

    let second = tokio::time::timeout(
        Duration::from_secs(2),
        client.request("daemon.ping", Value::Null),
    )
    .await
    .expect("a request after the connection is known closed must not hang")
    .unwrap_err();

    assert_eq!(second.code, code::INTERNAL);
    assert!(second.message.contains("not sent"), "{}", second.message);
}

#[tokio::test(flavor = "current_thread")]
async fn c1_a_request_that_cannot_be_written_at_all_fails_with_internal_and_says_not_sent() {
    let dir = tempfile::tempdir().unwrap();
    let socket = socket_in(&dir);
    let listener = UnixListener::bind(&socket).unwrap();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        drop(stream);
        ready_tx.send(()).unwrap();
    });

    let client = Client::connect(&socket).await.unwrap();
    // Arrange: wait for the peer's full close to actually land at the OS level before writing,
    // so the write syscall itself fails instead of racing the read side's own EOF detection.
    ready_rx.recv().unwrap();

    let err = client
        .request("daemon.ping", Value::Null)
        .await
        .unwrap_err();

    assert_eq!(err.code, code::INTERNAL);
    assert!(err.message.contains("not sent"), "{}", err.message);
}
