//! Scenario R13 (`scenarios/perf.md`): a busy machine skips a metric's regression test instead of failing it on noise.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use perf::budget::{Budget, Budgets, Miss, Report, Skipped, judge_all, load_per_cpu};
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
    let result = report("linux", 1, &[3.0], &[("write_to_output_p95_ms", 1.0)]);
    let outcome = judge_all(&result, &budgets).unwrap();

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
    let result = report("linux", 1, &[2.0], &[("write_to_output_p95_ms", 2.0)]);
    let outcome = judge_all(&result, &budgets).unwrap();

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
fn r13_a_threshold_for_another_os_does_not_skip_this_os() {
    let budgets: Budgets = BTreeMap::from([(
        "write_to_output_p95_ms".to_string(),
        budget(None, 0.0, &[("linux", 1.0)], &[("macos", 0.45)]),
    )]);
    let result = report("linux", 1, &[50.0], &[("write_to_output_p95_ms", 2.0)]);
    let outcome = judge_all(&result, &budgets).unwrap();

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
fn r13_load_per_cpu_is_the_median_of_the_runs_divided_by_cpu_count() {
    // Median of [1, 2, 9] is 2 (/2 = 1.0); the mean is 4 (/2 = 2.0), so using
    // the mean here instead of the median would change the expected result.
    assert_eq!(load_per_cpu(&[1.0, 2.0, 9.0], 2), 1.0);
}

#[test]
fn r13_the_skip_summary_reports_the_median_load_per_cpu_over_multiple_runs() {
    let budgets: Budgets = BTreeMap::from([(
        "write_to_output_p95_ms".to_string(),
        budget(None, 0.0, &[("linux", 1.0)], &[("linux", 2.0)]),
    )]);
    // The median of [4, 6, 100] is 6; the mean (36.67) is far off, so a
    // mean-instead-of-median regression would print a different number here.
    let result = report(
        "linux",
        2,
        &[4.0, 100.0, 6.0],
        &[("write_to_output_p95_ms", 1.0)],
    );
    let outcome = judge_all(&result, &budgets).unwrap();
    let lines = render(&result, &outcome, Path::new("target/perf.json"));

    assert_eq!(
        lines.last().unwrap(),
        "skipped 1 regression tests: load 3.00 per cpu"
    );
}

#[test]
fn r13_the_skip_summary_counts_every_skipped_metric() {
    let budgets: Budgets = BTreeMap::from([
        (
            "write_to_output_p95_ms".to_string(),
            budget(None, 0.0, &[("linux", 1.0)], &[("linux", 2.0)]),
        ),
        (
            "terminal_write_p95_ms".to_string(),
            budget(None, 0.0, &[("linux", 1.0)], &[("linux", 2.0)]),
        ),
    ]);
    let result = report(
        "linux",
        1,
        &[3.0],
        &[
            ("write_to_output_p95_ms", 1.0),
            ("terminal_write_p95_ms", 1.0),
        ],
    );
    let outcome = judge_all(&result, &budgets).unwrap();
    let lines = render(&result, &outcome, Path::new("target/perf.json"));

    assert_eq!(
        lines.last().unwrap(),
        "skipped 2 regression tests: load 3.00 per cpu"
    );
}

#[test]
fn r13_an_unbudgeted_metric_does_not_stop_the_rest_from_being_judged() {
    let budgets: Budgets = BTreeMap::from([(
        "write_to_output_p95_ms".to_string(),
        budget(Some(16.0), 0.0, &[], &[]),
    )]);
    // "aaa_unbudgeted" sorts before "write_to_output_p95_ms" in the BTreeMap
    // iteration order, so it is the metric the loop visits first.
    let result = report(
        "linux",
        1,
        &[0.1],
        &[("aaa_unbudgeted", 1.0), ("write_to_output_p95_ms", 20.0)],
    );
    let outcome = judge_all(&result, &budgets).unwrap();

    assert_eq!(
        outcome.misses,
        vec![(
            "write_to_output_p95_ms".to_string(),
            Miss::OverLimit { limit: 16.0 }
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
    let outcome = judge_all(&result, &budgets).unwrap();

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
fn r13_a_limit_miss_is_shown_together_with_the_skip_reason() {
    let budgets: Budgets = BTreeMap::from([(
        "write_to_output_p95_ms".to_string(),
        budget(Some(16.0), 0.0, &[("linux", 1.0)], &[("linux", 2.0)]),
    )]);
    let result = report("linux", 1, &[3.0], &[("write_to_output_p95_ms", 20.0)]);
    let outcome = judge_all(&result, &budgets).unwrap();
    let lines = render(&result, &outcome, Path::new("target/perf.json"));

    assert!(
        lines[0].contains("FAIL over limit 16")
            && lines[0].contains("not compared: load 3.00 per cpu, above 2"),
        "{lines:?}"
    );
}

#[test]
fn r13_memory_is_compared_whatever_the_load() {
    let budgets: Budgets = BTreeMap::from([(
        "rss_extra_mb".to_string(),
        budget(Some(150.0), 0.05, &[("linux", 1.6)], &[]),
    )]);
    let result = report("linux", 1, &[50.0], &[("rss_extra_mb", 2.0)]);
    let outcome = judge_all(&result, &budgets).unwrap();

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
fn r13_the_skipped_key_in_perf_json_holds_the_skipped_metrics() {
    let cpus = std::thread::available_parallelism().unwrap().get() as f64;
    let load = cpus * 3.0;
    // `conclude` stamps the report with `std::env::consts::OS`, so the
    // budget's keys must follow the host OS, not a hardcoded "linux": this
    // test must hold on macOS CI (`.github/workflows/check.yml`) too.
    let os = std::env::consts::OS;
    let budgets: Budgets = BTreeMap::from([(
        "write_to_output_p95_ms".to_string(),
        budget(None, 0.0, &[(os, 1.0)], &[(os, 2.0)]),
    )]);
    let per_run = BTreeMap::from([("write_to_output_p95_ms".to_string(), vec![1.0])]);
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("perf.json");

    assert!(conclude(per_run, vec![load], &budgets, &out).unwrap());

    let written: serde_json::Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();

    assert_eq!(
        written["skipped"],
        serde_json::json!([{
            "metric": "write_to_output_p95_ms",
            "load_per_cpu": 3.0,
            "max_load_per_cpu": 2.0,
        }])
    );
}

#[test]
fn conclude_writes_perf_json_before_propagating_a_budget_check_error() {
    let budgets: Budgets = BTreeMap::from([(
        "cold_start_ms".to_string(),
        budget(Some(300.0), 0.0, &[], &[]),
    )]);
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("perf.json");

    assert!(conclude(BTreeMap::new(), vec![0.1], &budgets, &out).is_err());
    assert!(out.exists());
}

#[test]
fn r13_the_summary_line_is_last_when_nothing_was_skipped() {
    let budgets: Budgets = BTreeMap::from([(
        "write_to_output_p95_ms".to_string(),
        budget(None, 0.0, &[("linux", 1.0)], &[("linux", 2.0)]),
    )]);
    let out = PathBuf::from("target/perf.json");

    let compared_run = report("linux", 1, &[1.0], &[("write_to_output_p95_ms", 1.0)]);
    let outcome = judge_all(&compared_run, &budgets).unwrap();
    let lines = render(&compared_run, &outcome, &out);

    assert!(lines.last().unwrap().starts_with("os "));
}

#[test]
fn r13_exits_0_when_every_metric_was_skipped_and_nothing_exceeded_its_limit() {
    let os = std::env::consts::OS;
    let cpus = std::thread::available_parallelism().unwrap().get() as f64;
    let budgets: Budgets = BTreeMap::from([
        (
            "write_to_output_p95_ms".to_string(),
            budget(Some(16.0), 0.001, &[(os, 0.066)], &[(os, 2.0)]),
        ),
        (
            "terminal_write_p95_ms".to_string(),
            budget(None, 0.001, &[(os, 0.055)], &[(os, 2.0)]),
        ),
    ]);
    let per_run = BTreeMap::from([
        ("write_to_output_p95_ms".to_string(), vec![0.07]),
        // Far over a 10% regression ceiling on 0.055: the skip keeps this green.
        ("terminal_write_p95_ms".to_string(), vec![0.2]),
    ]);
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("perf.json");

    assert!(conclude(per_run, vec![cpus * 3.0], &budgets, &out).unwrap());

    // The 0 exit must come from both metrics' regression tests being
    // skipped for load, not from terminal_write_p95_ms coincidentally having
    // no budget for this OS (which is the bug this test guards against).
    let written: serde_json::Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    assert_eq!(written["skipped"].as_array().unwrap().len(), 2);
}

#[test]
fn r13_the_cli_exits_2_naming_missing_load_figures_when_no_runs_happened() {
    let budgets = concat!(env!("CARGO_MANIFEST_DIR"), "/budgets.json");
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("perf.json");
    let output = Command::new(env!("CARGO_BIN_EXE_perf"))
        .args(["--budgets", budgets, "--runs", "0", "--out"])
        .arg(&out)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("load"));
}
