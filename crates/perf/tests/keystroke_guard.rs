//! Scenario R12 (`scenarios/perf.md`).

use std::process::Command;

fn keystroke(allow: Option<&str>) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_perf-keystroke"));

    command
        .args(["--app", "/nonexistent/roundup"])
        .env_remove("ROUNDUP_ALLOW_WINDOW");

    if let Some(value) = allow {
        command.env("ROUNDUP_ALLOW_WINDOW", value);
    }

    command.output().unwrap()
}

#[test]
fn r12_without_the_variable_it_refuses_before_touching_the_app() {
    let out = keystroke(None);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert_eq!(out.status.code(), Some(2));
    assert!(stderr.contains("ROUNDUP_ALLOW_WINDOW") && stderr.contains("opens a window"));
    assert!(
        !stderr.contains("/nonexistent"),
        "it went on to start the App: {stderr}"
    );
}

#[test]
fn r12_only_the_value_1_allows_it() {
    for value in ["0", "true", ""] {
        let stderr = String::from_utf8_lossy(&keystroke(Some(value)).stderr).into_owned();

        assert!(
            !stderr.contains("/nonexistent"),
            "{value:?} let it start the App: {stderr}"
        );
    }
}

#[test]
fn r12_with_the_variable_it_says_what_it_opens_and_then_goes_on() {
    let stderr = String::from_utf8_lossy(&keystroke(Some("1")).stderr).into_owned();

    assert!(stderr.contains("opens a window over your screen for about 40 s per run"));
}
