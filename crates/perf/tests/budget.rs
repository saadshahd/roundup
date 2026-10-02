//! Scenarios R1 to R10 (`scenarios/perf.md`).

use std::collections::BTreeMap;

use perf::budget::{Budget, Budgets, Miss, Report, judge, median, misses};

fn budget(limit: Option<f64>, noise: f64, baseline: &[(&str, f64)]) -> Budget {
    Budget {
        limit,
        noise,
        baseline: baseline
            .iter()
            .map(|(os, value)| (os.to_string(), *value))
            .collect(),
    }
}

fn report(os: &str, metrics: &[(&str, f64)]) -> Report {
    Report {
        os: os.into(),
        cpus: 1,
        runs: 1,
        loads: vec![0.0],
        metrics: metrics
            .iter()
            .map(|(name, value)| (name.to_string(), *value))
            .collect(),
        per_run: BTreeMap::new(),
    }
}

#[test]
fn r1_a_value_nine_percent_over_its_baseline_passes_and_ten_percent_is_the_edge() {
    let b = budget(None, 0.0, &[("linux", 100.0)]);

    assert_eq!(judge(109.0, &b, "linux"), None);
    assert_eq!(judge(110.0, &b, "linux"), None);
}

#[test]
fn r2_a_value_eleven_percent_over_its_baseline_fails() {
    let b = budget(None, 0.0, &[("linux", 100.0)]);

    assert!(
        matches!(judge(111.0, &b, "linux"), Some(Miss::Regressed { baseline, .. }) if baseline == 100.0)
    );
}

#[test]
fn r3_a_value_over_the_limit_fails_whatever_the_baseline() {
    let b = budget(Some(300.0), 0.0, &[("linux", 400.0)]);

    assert_eq!(
        judge(350.0, &b, "linux"),
        Some(Miss::OverLimit { limit: 300.0 })
    );
}

#[test]
fn r4_the_noise_allowance_covers_a_metric_near_the_clock_resolution() {
    let b = budget(None, 0.3, &[("linux", 0.03)]);

    assert_eq!(judge(0.2, &b, "linux"), None);
    assert!(judge(0.4, &b, "linux").is_some());
}

#[test]
fn r5_the_median_ignores_one_outlier_and_averages_the_middle_pair() {
    assert_eq!(median(&[9.0, 1000.0, 11.0]), 11.0);
    assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), 2.5);
}

#[test]
fn r6_a_platform_without_a_baseline_is_held_to_the_limit_only() {
    let b = budget(Some(300.0), 0.0, &[("linux", 10.0)]);

    assert_eq!(judge(299.0, &b, "macos"), None);
    assert_eq!(
        judge(301.0, &b, "macos"),
        Some(Miss::OverLimit { limit: 300.0 })
    );
}

#[test]
fn r7_a_budget_for_a_metric_that_was_not_measured_is_an_error() {
    let budgets = BTreeMap::from([("cold_start_ms".to_string(), budget(Some(300.0), 0.0, &[]))]);

    assert!(misses(&report("linux", &[]), &budgets).is_err());
    assert_eq!(
        misses(&report("linux", &[("cold_start_ms", 20.0)]), &budgets),
        Ok(vec![])
    );
}

fn committed() -> Budgets {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/budgets.json");

    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn next_up(value: f64) -> f64 {
    f64::from_bits(value.to_bits() + 1)
}

#[test]
fn r8_the_committed_limits_are_rule_sevens_budgets() {
    let budgets = committed();

    assert_eq!(budgets["cold_start_ms"].limit, Some(300.0));
    assert_eq!(budgets["rss_extra_mb"].limit, Some(150.0));
    assert_eq!(budgets["write_to_output_p95_ms"].limit, Some(16.0));
}

#[test]
fn r8_every_committed_baseline_gates_within_fifteen_percent_and_the_rest_have_a_limit() {
    for (name, budget) in committed() {
        if budget.baseline.is_empty() {
            assert!(
                budget.limit.is_some(),
                "{name} has neither a baseline nor a limit"
            );
        }

        for (os, baseline) in &budget.baseline {
            let ceiling = baseline * 1.10 + budget.noise;

            assert!(
                ceiling <= baseline * 1.15,
                "{name} on {os}: ceiling {ceiling} is more than 15% over {baseline}"
            );
        }
    }
}

#[test]
fn r9_a_value_at_a_committed_limit_passes_and_the_next_one_fails() {
    for (name, budget) in committed() {
        let Some(limit) = budget.limit else { continue };

        assert_eq!(judge(limit, &budget, "none"), None, "{name} at its limit");
        assert_eq!(
            judge(next_up(limit), &budget, "none"),
            Some(Miss::OverLimit { limit }),
            "{name} just over"
        );
    }
}

#[test]
fn r9_the_regression_ceiling_is_inclusive_and_exact_at_the_float_edge() {
    let b = budget(None, 0.0, &[("linux", 100.0)]);
    let ceiling = 100.0 * 1.10;

    assert_eq!(ceiling, 110.00000000000001);
    assert_eq!(judge(ceiling, &b, "linux"), None);
    assert!(judge(next_up(ceiling), &b, "linux").is_some());
}

#[test]
fn r8_the_committed_baselines_are_the_recorded_values() {
    let recorded = [
        ("cold_start_ms", "macos", 7.1),
        ("rss_extra_mb", "linux", 1.6),
        ("rss_extra_mb", "macos", 3.1),
        ("write_to_output_p95_ms", "linux", 0.066),
        ("write_to_output_p95_ms", "macos", 0.056),
        ("terminal_write_p95_ms", "linux", 0.055),
        ("terminal_write_p95_ms", "macos", 0.0365),
        ("ping_p95_ms", "linux", 0.025),
        ("rail_tree_40_p95_ms", "linux", 0.265),
        ("rail_tree_40_p95_ms", "macos", 0.1345),
    ];
    let budgets = committed();
    let committed_count: usize = budgets.values().map(|budget| budget.baseline.len()).sum();

    assert_eq!(
        committed_count,
        recorded.len(),
        "a baseline was added or removed"
    );

    for (name, os, value) in recorded {
        assert_eq!(
            budgets[name].baseline.get(os),
            Some(&value),
            "{name} on {os}"
        );
    }
}
