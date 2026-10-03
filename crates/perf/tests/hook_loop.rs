//! Scenario H15 (`scenarios/control.md`): the `hook_loop_ms` metric.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use perf::measure::hook_calls;

fn fake_rup(dir: &Path, exit: u8) -> std::path::PathBuf {
    let path = dir.join("rup");
    std::fs::write(&path, format!("#!/bin/sh\nexit {exit}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[tokio::test]
async fn h15_a_failing_rup_signal_fails_the_metric() {
    let dir = tempfile::tempdir().unwrap();
    let payload = dir.path().join("payload.json");
    std::fs::write(&payload, "{}").unwrap();

    let err = hook_calls(
        &fake_rup(dir.path(), 1),
        &dir.path().join("s"),
        "1",
        &payload,
        3,
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("rup signal exited"), "{err}");
}

#[tokio::test]
async fn h15_calls_that_exit_0_report_their_wall_time() {
    let dir = tempfile::tempdir().unwrap();
    let payload = dir.path().join("payload.json");
    std::fs::write(&payload, "{}").unwrap();

    let elapsed = hook_calls(
        &fake_rup(dir.path(), 0),
        &dir.path().join("s"),
        "1",
        &payload,
        3,
    )
    .await
    .unwrap();

    assert!(elapsed.as_millis() > 0);
}
