//! Scenario R12 (`scenarios/perf.md`).

use std::process::{Command, Output};

const REFUSAL: &str = "refusing without ROUNDUP_ALLOW_WINDOW=1";

/// `--budgets` names a real file and `--app` a missing one, so a run that is allowed stops at the App's path, never at the budgets and never opens a window.
fn keystroke(allow: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_perf-keystroke"));

    command
        .args([
            "--app",
            "/nonexistent/roundup",
            "--budgets",
            concat!(env!("CARGO_MANIFEST_DIR"), "/keystroke-budgets.json"),
        ])
        .env_remove("ROUNDUP_ALLOW_WINDOW");

    if let Some(value) = allow {
        command.env("ROUNDUP_ALLOW_WINDOW", value);
    }

    command.output().unwrap()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn r12_without_the_variable_it_refuses_before_touching_the_app() {
    let out = keystroke(None);

    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains(REFUSAL) && stderr(&out).contains("opens a window"));
    assert!(
        !stderr(&out).contains("/nonexistent"),
        "it went on to start the App: {}",
        stderr(&out)
    );
}

#[test]
fn r12_only_the_value_1_allows_it() {
    for value in ["0", "true", "", "11", "yes"] {
        let out = keystroke(Some(value));

        assert_eq!(out.status.code(), Some(2), "{value:?}");
        assert!(
            stderr(&out).contains(REFUSAL),
            "{value:?} was not refused: {}",
            stderr(&out)
        );
    }
}

#[test]
fn r12_with_the_variable_it_says_what_it_opens_and_then_goes_on_to_the_app() {
    let out = keystroke(Some("1"));

    assert!(stderr(&out).contains("opens a window over your screen for about 40 s per run"));
    assert!(!stderr(&out).contains(REFUSAL));
    assert!(
        stderr(&out).contains("/nonexistent/roundup"),
        "it did not reach the App: {}",
        stderr(&out)
    );
}
