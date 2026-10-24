//! X14: output belongs to one Terminal, is selected by its subscriber and reconciles with snapshots.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use contracts::EventData;
use rpc::code;
use terminal::{Output, OutputStream};
use tokio::sync::broadcast::error::TryRecvError;

use crate::common::{PATIENCE, drain, open, sh, until_exit};

/// `stty -opost` keeps the tty from rewriting the program's bytes, so offset parity names each byte.
const RAW: &str = "stty -opost -echo; ";

/// A line of `yes` is 63 `x` and a line feed; long lines keep the Output log's parser cheap in a debug build.
const LINE: u64 = 64;
const LINE_TEXT: &str = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";

/// Drain `stream` to its end, checking `yes`'s 64-byte lines at every offset; returns the bytes
/// delivered and the gaps reported.
async fn check_alternating(mut stream: OutputStream) -> (u64, Vec<(u64, u64)>) {
    let (mut delivered, mut gaps) = (0, Vec::new());
    tokio::time::timeout(PATIENCE * 3, async {
        let mut next = stream.start();
        while let Some(output) = stream.next().await {
            match output {
                Output::Bytes { offset, bytes } => {
                    assert_eq!(offset, next, "bytes must follow the last delivered byte");
                    for (index, byte) in bytes.iter().enumerate() {
                        let expected = if (offset + index as u64) % LINE == LINE - 1 {
                            b'\n'
                        } else {
                            b'x'
                        };
                        assert_eq!(*byte, expected, "offset {}", offset + index as u64);
                    }
                    next += bytes.len() as u64;
                    delivered += bytes.len() as u64;
                }
                Output::Gap { from, to } => {
                    assert_eq!(from, next);
                    assert!(to > from);
                    gaps.push((from, to));
                    next = to;
                }
            }
        }
    })
    .await
    .expect("the stream ends when its Terminal exits");
    (delivered, gaps)
}

#[tokio::test]
async fn x14_output_is_isolated_from_other_terminals_and_the_bus() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, bus) = open(&dir);
    // Never read until the end: output on the global Bus would lag it, since A prints more
    // chunks than its capacity (a chunk is at most 8192 bytes; Linux hands out about 4 KiB, macOS about 700).
    let mut global = bus.subscribe();
    let flood = 12_000_000;
    assert!(flood / 8192 > bus.capacity());
    let a = terminals
        .spawn(sh(
            dir.path(),
            &format!("{RAW}yes {LINE_TEXT} | head -c {flood}"),
        ))
        .await
        .unwrap();
    let mut b = terminals
        .spawn(sh(dir.path(), &format!("{RAW}printf marker")))
        .await
        .unwrap();
    let mut a_events = a.events;
    let mut b_events = b.events;
    let a_bytes = tokio::spawn(check_alternating(a.output));

    let b_printed = drain(&mut b.output).await;
    assert_eq!(b_printed, "marker");
    let (delivered, gaps) = a_bytes.await.unwrap();
    assert!(
        delivered > 0
            && delivered + gaps.iter().map(|(from, to)| to - from).sum::<u64>() == flood as u64
    );
    // A's and B's exit events stay on their own receivers and the Bus.
    for events in [&mut a_events, &mut b_events] {
        loop {
            if matches!(
                events.recv().await.unwrap().as_ref(),
                EventData::TerminalExited(_)
            ) {
                break;
            }
        }
    }
    let mut exits = 0;
    loop {
        match global.try_recv() {
            Ok(event) => exits += usize::from(matches!(event.data, EventData::TerminalExited(_))),
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Lagged(missed)) => panic!("output lagged the global Bus by {missed}"),
            Err(TryRecvError::Closed) => panic!("the Bus closed"),
        }
    }
    assert_eq!(exits, 2);
}

/// Rebuild a screen from a snapshot plus the stream bytes the snapshot does not cover.
fn restore(
    snapshot: &contracts::terminal::Snapshot,
    chunks: &[(u64, std::sync::Arc<[u8]>)],
) -> String {
    let mut parser = vt100::Parser::new(snapshot.rows, snapshot.cols, 0);
    parser.process(&STANDARD.decode(&snapshot.data).unwrap());
    let mut next = snapshot.after;
    for (offset, bytes) in chunks {
        // A chunk is wholly before or wholly after the cut, never across it.
        assert!(
            *offset >= snapshot.after || offset + bytes.len() as u64 <= snapshot.after,
            "chunk at {offset} straddles the snapshot's {}",
            snapshot.after
        );
        if *offset >= snapshot.after {
            assert_eq!(*offset, next, "no gap and no repeat after the snapshot");
            parser.process(bytes);
            next += bytes.len() as u64;
        }
    }
    parser.screen().contents()
}

#[tokio::test]
async fn x14_attach_reconciles_snapshot_and_live_offsets() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut full = terminals
        .spawn(sh(
            dir.path(),
            "i=0; while [ $i -lt 3000 ]; do echo line$i; i=$((i+1)); [ $((i % 100)) = 0 ] && sleep 0.01; done; sleep 0.2; echo end",
        ))
        .await
        .unwrap();
    let mut whole = Vec::new();
    // Attach while it prints: subscription first, then the snapshot, as the App does.
    crate::common::until_printed(&mut full.output, "line500").await;
    let mut late = terminals.subscribe_output(&full.id).unwrap();
    let start = late.start();
    let snapshot = terminals.snapshot(&full.id).unwrap();
    assert!(
        snapshot.after >= start,
        "the snapshot is not older than the subscription"
    );
    let mut chunks = Vec::new();
    while let Some(output) = late.next().await {
        let Output::Bytes { offset, bytes } = output else {
            panic!("a gap in a small output")
        };
        chunks.push((offset, bytes));
    }
    // The first stream delivered everything from offset 0; its end is where the late one ends.
    while let Some(Output::Bytes { bytes, .. }) = full.output.next().await {
        whole.extend_from_slice(&bytes);
    }
    let restored = restore(&snapshot, &chunks);
    let mut expected = vt100::Parser::new(snapshot.rows, snapshot.cols, 0);
    let screen_end = terminals.snapshot(&full.id).unwrap();
    expected.process(&STANDARD.decode(screen_end.data).unwrap());
    assert_eq!(restored, expected.screen().contents());
    assert!(restored.contains("end"));
    until_exit(&mut full).await;
}

#[tokio::test]
async fn x14_lag_recovers_only_the_affected_terminal() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut slow = terminals
        .spawn(sh(dir.path(), "seq -f 'a%07g' 1 200000"))
        .await
        .unwrap();
    let mut other = terminals
        .spawn(sh(dir.path(), "read go; seq -f 'b%03g' 1 50"))
        .await
        .unwrap();
    terminals.write(&other.id, b"\n").await.unwrap();
    // `slow` is not read while its program prints far more than its backlog; `other` is unaffected.
    let (printed, _) = until_exit(&mut other).await;
    assert!(printed.contains("b050"), "{printed:?}");
    while terminals.list().iter().any(|t| t.running) {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    let Some(Output::Gap { from, to }) = slow.output.next().await else {
        panic!("a subscriber that fell behind is told so before any later byte");
    };
    assert_eq!(from, 0);
    let Some(Output::Bytes { offset, .. }) = slow.output.next().await else {
        panic!("delivery resumes after the gap");
    };
    assert_eq!(offset, to);
    // Recovery: a fresh snapshot covers the gap, and the stream's later bytes follow its offset.
    let snapshot = terminals.snapshot(&slow.id).unwrap();
    assert!(snapshot.after >= to);
    let mut chunks = Vec::new();
    while let Some(output) = slow.output.next().await {
        if let Output::Bytes { offset, bytes } = output {
            chunks.push((offset, bytes));
        }
    }
    let restored = restore(&snapshot, &chunks);
    assert!(restored.contains("a0200000"), "{restored:?}");
}

#[tokio::test]
async fn x14_output_subscription_lifetime() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    assert_eq!(
        terminals.subscribe_output("999").err().unwrap().code,
        code::NOT_FOUND
    );

    // The final bytes reach an attached subscriber, then its stream ends, then the exit is reported.
    let mut spawned = terminals
        .spawn(sh(dir.path(), "stty -echo; read go; printf final"))
        .await
        .unwrap();
    let mut attached = terminals.subscribe_output(&spawned.id).unwrap();
    terminals.write(&spawned.id, b"\n").await.unwrap();
    assert!(drain(&mut attached).await.ends_with("final"));
    assert!(drain(&mut spawned.output).await.ends_with("final"));
    let (_, code_of_exit) = until_exit(&mut spawned).await;
    assert_eq!(code_of_exit, Some(0));
    assert_eq!(
        terminals.subscribe_output(&spawned.id).err().unwrap().code,
        code::NOT_FOUND
    );

    // Closing an owned Terminal releases its stream.
    let owned = terminals
        .spawn_owned(sh(dir.path(), "sleep 30"), "w")
        .await
        .unwrap();
    let mut stream = terminals.subscribe_output(&owned.id).unwrap();
    terminals.close(&owned.id).await.unwrap();
    // Output already queued before the close may still arrive; the stream must then end.
    drain(&mut stream).await;
}
