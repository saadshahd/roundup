//! X9 a stuck write never blocks kill.

use std::future::Future;

use crate::common::{PATIENCE, open, sh, until_printed};
use rpc::code;

const STUCK: &str = "stty raw -echo; echo ready; exec sleep 60";
/// Larger than a PTY's input buffer, so one write never completes against a program that does not read.
const CHUNK: usize = 1 << 16;

/// A runtime whose blocking pool is as small as the scenario's "more writes than threads".
fn small_pool<F: Future>(test: F) -> F::Output {
    tokio::runtime::Builder::new_multi_thread()
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap()
        .block_on(test)
}

#[test]
fn x9_kill_returns_while_writes_are_stuck() {
    small_pool(async {
        let dir = tempfile::tempdir().unwrap();
        let (terminals, _) = open(&dir);
        let mut spawned = terminals.spawn(sh(dir.path(), STUCK)).await.unwrap();
        until_printed(&mut spawned.events, "ready").await;
        tokio::time::timeout(PATIENCE, async {
            for _ in 0..4 {
                terminals.write(&spawned.id, &[0; CHUNK]).await.unwrap();
            }
            terminals.kill(&spawned.id).await.unwrap();
        })
        .await
        .expect("kill is not held up by writes");
    });
}

#[test]
fn x9_a_write_that_cannot_be_queued_is_conflict() {
    small_pool(async {
        let dir = tempfile::tempdir().unwrap();
        let (terminals, _) = open(&dir);
        let mut spawned = terminals.spawn(sh(dir.path(), STUCK)).await.unwrap();
        until_printed(&mut spawned.events, "ready").await;
        let refused = tokio::time::timeout(PATIENCE, async {
            loop {
                if let Err(err) = terminals.write(&spawned.id, &[0; CHUNK]).await {
                    return err;
                }
            }
        })
        .await
        .expect("a full queue refuses instead of waiting");
        assert_eq!(refused.code, code::CONFLICT);
        terminals.kill(&spawned.id).await.unwrap();
    });
}

#[tokio::test]
async fn x9_a_killed_program_is_gone_and_a_second_kill_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, _) = open(&dir);
    let mut spawned = terminals
        .spawn(sh(dir.path(), "trap '' HUP; echo ready; exec cat"))
        .await
        .unwrap();
    until_printed(&mut spawned.events, "ready").await;
    terminals.kill(&spawned.id).await.unwrap();
    let again = terminals.kill(&spawned.id).await.unwrap_err();
    assert_eq!(again.code, code::NOT_FOUND);
}
