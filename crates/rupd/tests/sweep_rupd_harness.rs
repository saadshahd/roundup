//! sweep-rupd-harness: the shared helper starts a Daemon that answers `daemon.ping`, and kills it.

#[path = "support/rupd_harness.rs"]
mod rupd_harness;

use std::path::Path;

fn is_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
        .success()
}

#[tokio::test]
async fn sweep_rupd_harness_starts_and_pings() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = rupd_harness::start(
        Path::new(env!("CARGO_BIN_EXE_rupd")),
        dir.path(),
        true,
        |_| {},
    );
    let pid = daemon.child.id();

    let client = rpc::Client::connect(&daemon.socket).await.unwrap();
    let pong = client.request("daemon.ping", ()).await.unwrap();
    assert_eq!(pong["pong"], true);

    drop(daemon);
    assert!(!is_alive(pid), "the Daemon outlived its handle");
}
