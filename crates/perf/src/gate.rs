use std::collections::BTreeMap;
use std::path::Path;

use crate::budget::{Budgets, Miss, Report, median, misses};

/// Reads `budgets.json`-shaped files; the error names the file.
pub fn read_budgets(path: &Path) -> Result<Budgets, Box<dyn std::error::Error>> {
    serde_json::from_slice(
        &std::fs::read(path).map_err(|err| format!("{}: {err}", path.display()))?,
    )
    .map_err(|err| format!("{}: {err}", path.display()).into())
}

/// Takes the median of each metric over its runs, writes the report to `out`, prints each metric against its budget and returns whether every one passed.
pub fn conclude(
    per_run: BTreeMap<String, Vec<f64>>,
    loads: Vec<f64>,
    budgets: &Budgets,
    out: &Path,
) -> Result<bool, Box<dyn std::error::Error>> {
    let cpus = std::thread::available_parallelism()?.get();
    let result = Report {
        os: std::env::consts::OS.into(),
        cpus,
        runs: loads.len(),
        loads,
        metrics: per_run
            .iter()
            .map(|(name, values)| (name.clone(), median(values)))
            .collect(),
        per_run,
    };

    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }

    std::fs::write(out, serde_json::to_vec_pretty(&result)?)?;

    let missed = misses(&result, budgets)?;

    for (name, value) in &result.metrics {
        let spread = &result.per_run[name];
        let low = spread.iter().copied().fold(f64::INFINITY, f64::min);
        let high = spread.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let verdict = match missed.iter().find(|(missed_name, _)| missed_name == name) {
            None => "ok".to_string(),
            Some((_, Miss::OverLimit { limit })) => format!("FAIL over limit {limit}"),
            Some((_, Miss::Regressed { baseline, ceiling })) => {
                format!("FAIL regressed from {baseline} (ceiling {ceiling:.3})")
            }
        };

        println!("{name:<26} median {value:>9.3}  range {low:.3}..{high:.3}  {verdict}");
    }

    println!(
        "os {} cpus {} runs {} load {:?} -> {}",
        result.os,
        result.cpus,
        result.runs,
        result.loads,
        out.display()
    );

    Ok(missed.is_empty())
}
