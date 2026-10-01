//! Helpers shared by the scenario tests.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use contracts::EventData;
use contracts::terminal::SpawnParams;
use tokio::sync::broadcast::Receiver;

const PATIENCE: Duration = Duration::from_secs(10);

pub fn sh(cwd: &Path, script: &str) -> SpawnParams {
    SpawnParams {
        cwd: cwd.to_string_lossy().into_owned(),
        command: Some(vec!["/bin/sh".into(), "-c".into(), script.into()]),
        env: BTreeMap::new(),
        cols: 80,
        rows: 24,
    }
}

pub fn decode(data: &str) -> String {
    String::from_utf8_lossy(&STANDARD.decode(data).expect("base64 output")).into_owned()
}

/// Everything the program printed and the exit code it ended with.
pub async fn until_exit(events: &mut Receiver<EventData>) -> (String, Option<i32>) {
    let mut printed = String::new();
    tokio::time::timeout(PATIENCE, async {
        loop {
            match events.recv().await.expect("events stay open") {
                EventData::TerminalOutput(out) => printed.push_str(&decode(&out.data)),
                EventData::TerminalExited(done) => return (printed, done.code),
                _ => {}
            }
        }
    })
    .await
    .expect("the program exits in time")
}
