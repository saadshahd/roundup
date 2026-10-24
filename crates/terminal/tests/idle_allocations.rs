//! R3696c: retained memory of idle Terminal event/output queues on the real spawn path.
use std::{alloc::System, collections::BTreeMap};

use contracts::terminal::SpawnParams;
use rpc::Bus;
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};

#[global_allocator]
static ALLOCATOR: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

#[tokio::test(flavor = "current_thread")]
async fn r3696c_ten_quiet_terminals_retain_under_800_kb() {
    let dir = tempfile::tempdir().unwrap();
    let terminals = terminal::Terminals::open(dir.path(), Bus::new()).unwrap();
    let mut spawned = Vec::new();
    let region = Region::new(ALLOCATOR);
    for _ in 0..10 {
        spawned.push(
            terminals
                .spawn(SpawnParams {
                    cwd: dir.path().to_string_lossy().into_owned(),
                    command: Some(vec!["/bin/cat".into()]),
                    env: BTreeMap::new(),
                    cols: 80,
                    rows: 24,
                })
                .await
                .unwrap(),
        );
    }
    let stats = region.change();
    let retained = stats
        .bytes_allocated
        .saturating_sub(stats.bytes_deallocated);
    eprintln!("ten quiet Terminals retain {retained} heap bytes");
    // Clean up before asserting, including when the allocation budget fails.
    for terminal in &spawned {
        terminals.kill(&terminal.id).await.unwrap();
    }
    assert!(
        retained < 800_000,
        "ten quiet Terminals retain {retained} heap bytes"
    );
    std::hint::black_box(spawned);
}
