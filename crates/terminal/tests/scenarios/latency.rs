//! U174 a File drop adds no wait of roundup's: the Daemon leg.

use std::time::{Duration, Instant};

use crate::common::{PATIENCE, ctx, decode, open, sh};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use contracts::EventData;
use contracts::terminal::WriteParams;
use rpc::Module;

const FRAME: Duration = Duration::from_millis(16);

async fn p95_of_100_writes(bracketed: bool) -> Duration {
    let dir = tempfile::tempdir().unwrap();
    let (terminals, bus) = open(&dir);
    let ctx = ctx(&dir, &bus);
    let mut spawned = terminals.spawn(sh(dir.path(), "cat")).await.unwrap();
    let mut seen = String::new();
    // Wait for `cat` to run: a write racing the shell's start can be flushed by it.
    let ready = WriteParams {
        id: spawned.id.clone(),
        data: STANDARD.encode("ready\n"),
    };
    terminals
        .call(&ctx, "terminal.write", serde_json::to_value(ready).unwrap())
        .await
        .unwrap();
    tokio::time::timeout(PATIENCE, async {
        while seen.matches("ready").count() < 2 {
            if let EventData::TerminalOutput(out) = spawned.events.recv().await.unwrap() {
                seen.push_str(&decode(&out.data));
            }
        }
    })
    .await
    .expect("cat is running");
    seen.clear();
    let mut took = Vec::new();
    for i in 0..100 {
        let path = format!("'/p/image-{i}.png' ");
        let bytes = if bracketed {
            format!("\x1b[200~{path}\x1b[201~")
        } else {
            path.clone()
        };
        let write = WriteParams {
            id: spawned.id.clone(),
            data: STANDARD.encode(bytes),
        };
        let start = Instant::now();
        terminals
            .call(&ctx, "terminal.write", serde_json::to_value(write).unwrap())
            .await
            .unwrap();
        tokio::time::timeout(PATIENCE, async {
            while !seen.contains(&path) {
                if let EventData::TerminalOutput(out) = spawned.events.recv().await.unwrap() {
                    seen.push_str(&decode(&out.data));
                }
            }
        })
        .await
        .expect("the bytes echo back");
        took.push(start.elapsed());
        seen.clear();
    }
    terminals.kill(&spawned.id).await.unwrap();
    took.sort();
    took[94]
}

#[tokio::test]
async fn u174_a_write_reaches_the_pty_within_a_frame() {
    for bracketed in [false, true] {
        let p95 = p95_of_100_writes(bracketed).await;
        assert!(
            p95 <= FRAME,
            "p95 {p95:?} with bracketed={bracketed} exceeds a frame"
        );
    }
}
