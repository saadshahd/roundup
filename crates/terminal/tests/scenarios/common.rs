//! Helpers shared by the scenario tests.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use contracts::terminal::SpawnParams;
use contracts::{Actor, EventData};
use provenance::Touches;
use rpc::{Bus, Ctx};
use terminal::{Output, OutputStream, Spawned, Terminals};

pub fn open(dir: &tempfile::TempDir) -> (Terminals, Bus) {
    let bus = Bus::new();
    (Terminals::open(dir.path(), bus.clone()).unwrap(), bus)
}

pub fn ctx(dir: &tempfile::TempDir, bus: &Bus) -> Ctx {
    Ctx {
        actor: Actor::user(),
        bus: bus.clone(),
        touches: Arc::new(Touches::open(&dir.path().join("provenance.db")).unwrap()),
    }
}

pub const PATIENCE: Duration = Duration::from_secs(30);

pub fn sh(cwd: &Path, script: &str) -> SpawnParams {
    SpawnParams {
        cwd: cwd.to_string_lossy().into_owned(),
        command: Some(vec!["/bin/sh".into(), "-c".into(), script.into()]),
        env: BTreeMap::new(),
        cols: 80,
        rows: 24,
    }
}

/// What the stream delivers until its Terminal has exited.
pub async fn drain(output: &mut OutputStream) -> String {
    let mut printed = String::new();
    tokio::time::timeout(PATIENCE, async {
        while let Some(output) = output.next().await {
            if let Output::Bytes { bytes, .. } = output {
                printed.push_str(&String::from_utf8_lossy(&bytes));
            }
        }
    })
    .await
    .expect("the output ends in time");
    printed
}

/// Everything the program printed and the exit code it ended with: the output ends before the exit is reported.
pub async fn until_exit(spawned: &mut Spawned) -> (String, Option<i32>) {
    let printed = drain(&mut spawned.output).await;
    tokio::time::timeout(PATIENCE, async {
        loop {
            if let EventData::TerminalExited(done) = spawned
                .events
                .recv()
                .await
                .expect("events stay open")
                .as_ref()
            {
                return (printed.clone(), done.code);
            }
        }
    })
    .await
    .expect("the program exits in time")
}

/// The exit code of a program whose output the test ignores.
pub async fn until_exit_code(spawned: &mut Spawned) -> Option<i32> {
    until_exit(spawned).await.1
}

/// Wait until the program's output so far contains `needle`.
pub async fn until_printed(output: &mut OutputStream, needle: &str) {
    let mut printed = String::new();
    tokio::time::timeout(PATIENCE, async {
        while !printed.contains(needle) {
            match output.next().await.expect("the output stays open") {
                Output::Bytes { bytes, .. } => printed.push_str(&String::from_utf8_lossy(&bytes)),
                Output::Gap { .. } => {}
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{needle:?} was not printed in time"));
}
