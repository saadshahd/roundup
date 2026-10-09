//! Scenario R10 (`scenarios/perf.md`).

use std::process::Command;

fn perf() -> Command {
    Command::new(env!("CARGO_BIN_EXE_perf"))
}

#[test]
fn r10_a_missing_budgets_file_exits_2_naming_it() {
    let out = perf()
        .args(["--budgets", "/nonexistent/budgets.json"])
        .output()
        .unwrap();

    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("budgets.json"));
}

#[test]
fn r10_a_missing_rupd_exits_2_naming_it() {
    let budgets = concat!(env!("CARGO_MANIFEST_DIR"), "/budgets.json");
    let out = perf()
        .args([
            "--budgets",
            budgets,
            "--runs",
            "1",
            "--rupd",
            "/nonexistent/rupd",
        ])
        .output()
        .unwrap();

    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("rupd"));
}
