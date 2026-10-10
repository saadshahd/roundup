//! B28 through a real `rupd`: a failed Messages operation is one stderr line and one `INTERNAL`.

#[path = "support/rupd_harness.rs"]
mod rupd_harness;

use std::io::Read;
use std::path::Path;

#[tokio::test]
async fn b28_through_rupd_the_stderr_line_and_the_callers_error() {
    let dir = tempfile::tempdir().unwrap();
    let mut daemon = rupd_harness::start(
        Path::new(env!("CARGO_BIN_EXE_rupd")),
        dir.path(),
        true,
        |command| {
            command.env("RUPD_MESSAGES_FAULT", "route.list");
        },
    );
    let client = rpc::Client::connect(&daemon.socket).await.unwrap();

    let err = client.request("route.list", ()).await.unwrap_err();
    assert!(err.to_string().contains("injected failure in route.list"));
    assert!(err.to_string().contains(&rpc::code::INTERNAL.to_string()));
    client.request("route.list", ()).await.unwrap();

    let mut stderr = daemon.stderr.take().unwrap();
    daemon.child.kill().unwrap();
    daemon.child.wait().unwrap();
    let mut log = String::new();
    stderr.read_to_string(&mut log).unwrap();
    let lines: Vec<_> = log.lines().filter(|l| l.contains("failed")).collect();
    assert_eq!(
        lines,
        ["rupd: messages: route.list failed: injected failure in route.list"]
    );
}
