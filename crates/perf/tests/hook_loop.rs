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
        &[&payload],
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
        &[&payload],
        3,
    )
    .await
    .unwrap();

    assert!(elapsed.as_millis() > 0);
}

#[tokio::test]
async fn h15_hook_calls_runs_exactly_the_calls_asked_for_cycling_the_payloads() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("log");
    let rup = dir.path().join("rup");
    std::fs::write(
        &rup,
        format!(
            "#!/bin/sh\ncat >>{}\necho >>{}\n",
            log.display(),
            log.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&rup, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (pre, post) = (dir.path().join("pre"), dir.path().join("post"));
    std::fs::write(&pre, "pre").unwrap();
    std::fs::write(&post, "post").unwrap();

    hook_calls(&rup, &dir.path().join("s"), "1", &[&pre, &post], 5)
        .await
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(&log).unwrap(),
        "pre\npost\npre\npost\npre\n"
    );
}
