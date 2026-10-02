//! Scenario R13 (`scenarios/perf.md`): a busy machine skips a metric's regression test instead of failing it on noise.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use perf::budget::{Budget, Budgets, Miss, Report, Skipped, misses};
use perf::gate::{conclude, render};

fn budget(
    limit: Option<f64>,
    noise: f64,
    baseline: &[(&str, f64)],
    max_load_per_cpu: &[(&str, f64)],
) -> Budget {
    Budget {
        limit,
        noise,
        baseline: baseline
            .iter()
            .map(|(os, value)| (os.to_string(), *value))
            .collect(),
        max_load_per_cpu: max_load_per_cpu
            .iter()
            .map(|(os, value)| (os.to_string(), *value))
            .collect(),
    }
}

fn report(os: &str, cpus: usize, loads: &[f64], metrics: &[(&str, f64)]) -> Report {
    Report {
        os: os.into(),
        cpus,
        runs: loads.len(),
        loads: loads.to_vec(),
        metrics: metrics
            .iter()
            .map(|(name, value)| (name.to_string(), *value))
            .collect(),
        per_run: metrics
            .iter()
            .map(|(name, value)| (name.to_string(), vec![*value]))
            .collect(),
        skipped: Vec::new(),
    }
}

#[test]
fn r13_skipped_above_the_threshold_says_not_compared_with_the_load_and_max() {
    let budgets: Budgets = BTreeMap::from([(
        "write_to_output_p95_ms".to_string(),
        budget(None, 0.0, &[("linux", 1.0)], &[("linux", 2.0)]),
    )]);
    // load per cpu 3.0 on 1 cpu, above the 2.0 threshold.
    let result = report("linux", 1, &[3.0], &[("write_to_output_p95_ms", 1.0)]);
    let outcome = misses(&result, &budgets).unwrap();

    assert!(outcome.misses.is_empty());
    assert_eq!(
        outcome.skipped,
        vec![Skipped {
            metric: "write_to_output_p95_ms".into(),
            load_per_cpu: 3.0,
            max_load_per_cpu: 2.0,
        }]
    );

    let lines = render(&result, &outcome, Path::new("target/perf.json"));
    assert!(
        lines[0].contains("not compared: load 3.00 per cpu, above 2"),
        "{lines:?}"
    );
}

#[test]
fn r13_at_the_threshold_the_regression_test_still_compares() {
    let budgets: Budgets = BTreeMap::from([(
        "write_to_output_p95_ms".to_string(),
        budget(None, 0.0, &[("linux", 1.0)], &[("linux", 2.0)]),
    )]);
    // load per cpu exactly 2.0: at the threshold, not above it.
    let result = report("linux", 1, &[2.0], &[("write_to_output_p95_ms", 2.0)]);
    let outcome = misses(&result, &budgets).unwrap();

    assert!(outcome.skipped.is_empty());
    assert_eq!(
        outcome.misses,
        vec![(
            "write_to_output_p95_ms".to_string(),
            Miss::Regressed {
                baseline: 1.0,
                ceiling: 1.1
            }
        )]
    );
}

#[test]
fn r13_a_limit_still_fails_while_its_regression_test_is_skipped() {
    let budgets: Budgets = BTreeMap::from([(
        "write_to_output_p95_ms".to_string(),
        budget(Some(16.0), 0.0, &[("linux", 1.0)], &[("linux", 2.0)]),
    )]);
    let result = report("linux", 1, &[3.0], &[("write_to_output_p95_ms", 20.0)]);
    let outcome = misses(&result, &budgets).unwrap();

    assert_eq!(outcome.skipped.len(), 1);
    assert_eq!(
        outcome.misses,
        vec![(
            "write_to_output_p95_ms".to_string(),
            Miss::OverLimit { limit: 16.0 }
        )]
    );
}

#[test]
fn r13_memory_is_compared_whatever_the_load() {
    let budgets: Budgets = BTreeMap::from([(
        "rss_extra_mb".to_string(),
        budget(Some(150.0), 0.05, &[("linux", 1.6)], &[]),
    )]);
    // load per cpu 50.0: far above every time metric's threshold, but rss_extra_mb has none.
    let result = report("linux", 1, &[50.0], &[("rss_extra_mb", 2.0)]);
    let outcome = misses(&result, &budgets).unwrap();

    assert!(outcome.skipped.is_empty());
    assert_eq!(
        outcome.misses,
        vec![(
            "rss_extra_mb".to_string(),
            Miss::Regressed {
                baseline: 1.6,
                ceiling: 1.6 * 1.1 + 0.05
            }
        )]
    );
}

#[test]
fn r13_the_skipped_key_is_present_and_empty_when_nothing_was_skipped() {
    let budgets: Budgets = BTreeMap::from([(
        "rss_extra_mb".to_string(),
        budget(Some(150.0), 0.05, &[("linux", 1.6)], &[]),
    )]);
    let per_run = BTreeMap::from([("rss_extra_mb".to_string(), vec![1.6])]);
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("perf.json");

    assert!(conclude(per_run, vec![0.1], &budgets, &out).unwrap());

    let written: serde_json::Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();

    assert_eq!(written["skipped"], serde_json::json!([]));
}

#[test]
fn r13_the_skip_summary_is_the_last_line_only_when_something_was_skipped() {
    let budgets: Budgets = BTreeMap::from([(
        "write_to_output_p95_ms".to_string(),
        budget(None, 0.0, &[("linux", 1.0)], &[("linux", 2.0)]),
    )]);
    let out = PathBuf::from("target/perf.json");

    let skipped_run = report("linux", 1, &[3.0], &[("write_to_output_p95_ms", 1.0)]);
    let outcome = misses(&skipped_run, &budgets).unwrap();
    let lines = render(&skipped_run, &outcome, &out);

    assert_eq!(
        lines.last().unwrap(),
        "skipped 1 regression tests: load 3.00 per cpu"
    );

    let compared_run = report("linux", 1, &[1.0], &[("write_to_output_p95_ms", 1.0)]);
    let outcome = misses(&compared_run, &budgets).unwrap();
    let lines = render(&compared_run, &outcome, &out);

    assert!(lines.last().unwrap().starts_with("os "));
}

#[test]
fn r13_exits_0_when_every_metric_was_skipped_and_nothing_exceeded_its_limit() {
    let budgets: Budgets = BTreeMap::from([
        (
            "write_to_output_p95_ms".to_string(),
            budget(Some(16.0), 0.001, &[("linux", 0.066)], &[("linux", 2.0)]),
        ),
        (
            "terminal_write_p95_ms".to_string(),
            budget(None, 0.001, &[("linux", 0.055)], &[("linux", 2.0)]),
        ),
    ]);
    let per_run = BTreeMap::from([
        ("write_to_output_p95_ms".to_string(), vec![0.07]),
        // Far over a 10% regression ceiling on 0.055: only the skip, not a coincidental pass, keeps this green.
        ("terminal_write_p95_ms".to_string(), vec![0.2]),
    ]);
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("perf.json");

    // load per cpu 5.0 on 1 cpu: above both metrics' 2.0 threshold.
    assert!(conclude(per_run, vec![5.0], &budgets, &out).unwrap());
}

#[test]
fn r13_the_cli_exits_2_naming_missing_load_figures_when_no_runs_happened() {
    let budgets = concat!(env!("CARGO_MANIFEST_DIR"), "/budgets.json");
    let out = Command::new(env!("CARGO_BIN_EXE_perf"))
        .args(["--budgets", budgets, "--runs", "0"])
        .output()
        .unwrap();

    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("load"));
}
