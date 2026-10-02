use std::collections::BTreeMap;
use std::path::Path;

use crate::budget::{Budgets, Miss, Outcome, Report, load_per_cpu, median, misses};

/// Reads `budgets.json`-shaped files; the error names the file.
pub fn read_budgets(path: &Path) -> Result<Budgets, Box<dyn std::error::Error>> {
    serde_json::from_slice(
        &std::fs::read(path).map_err(|err| format!("{}: {err}", path.display()))?,
    )
    .map_err(|err| format!("{}: {err}", path.display()).into())
}

/// What one metric's line says about its judgement: a plain pass or fail, and (R13) the load that stopped its regression test being compared at all.
fn verdict(name: &str, outcome: &Outcome) -> String {
    let miss = outcome
        .misses
        .iter()
        .find(|(missed_name, _)| missed_name == name)
        .map(|(_, miss)| miss);
    let skip = outcome.skipped.iter().find(|skip| skip.metric == name);

    let not_compared = skip.map(|skip| {
        format!(
            "not compared: load {:.2} per cpu, above {}",
            skip.load_per_cpu, skip.max_load_per_cpu
        )
    });

    match (miss, not_compared) {
        (Some(Miss::OverLimit { limit }), Some(not_compared)) => {
            format!("FAIL over limit {limit}; {not_compared}")
        }
        (_, Some(not_compared)) => not_compared,
        (Some(Miss::OverLimit { limit }), None) => format!("FAIL over limit {limit}"),
        (Some(Miss::Regressed { baseline, ceiling }), None) => {
            format!("FAIL regressed from {baseline} (ceiling {ceiling:.3})")
        }
        (None, None) => "ok".to_string(),
    }
}

/// Every printed line, in order: one per metric, the run summary, and (R13) the skip summary when at least one metric's regression test was skipped.
pub fn render(result: &Report, outcome: &Outcome, out: &Path) -> Vec<String> {
    let mut lines: Vec<String> = result
        .metrics
        .iter()
        .map(|(name, value)| {
            let spread = &result.per_run[name];
            let low = spread.iter().copied().fold(f64::INFINITY, f64::min);
            let high = spread.iter().copied().fold(f64::NEG_INFINITY, f64::max);

            format!(
                "{name:<26} median {value:>9.3}  range {low:.3}..{high:.3}  {}",
                verdict(name, outcome)
            )
        })
        .collect();

    lines.push(format!(
        "os {} cpus {} runs {} load {:?} -> {}",
        result.os,
        result.cpus,
        result.runs,
        result.loads,
        out.display()
    ));

    if !outcome.skipped.is_empty() {
        lines.push(format!(
            "skipped {} regression tests: load {:.2} per cpu",
            outcome.skipped.len(),
            load_per_cpu(&result.loads, result.cpus)
        ));
    }

    lines
}

/// Takes the median of each metric over its runs, writes the report to `out`, prints each metric against its budget and returns whether every one passed.
pub fn conclude(
    per_run: BTreeMap<String, Vec<f64>>,
    loads: Vec<f64>,
    budgets: &Budgets,
    out: &Path,
) -> Result<bool, Box<dyn std::error::Error>> {
    let cpus = std::thread::available_parallelism()?.get();
    let mut result = Report {
        os: std::env::consts::OS.into(),
        cpus,
        runs: loads.len(),
        loads,
        metrics: per_run
            .iter()
            .map(|(name, values)| (name.clone(), median(values)))
            .collect(),
        per_run,
        skipped: Vec::new(),
    };

    let outcome = misses(&result, budgets)?;

    result.skipped = outcome.skipped.clone();

    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }

    std::fs::write(out, serde_json::to_vec_pretty(&result)?)?;

    for line in render(&result, &outcome, out) {
        println!("{line}");
    }

    Ok(outcome.misses.is_empty())
}
