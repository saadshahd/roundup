use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use perf::budget::{Budgets, Miss, Report, median, misses};
use perf::measure::one_run;
use sysinfo::System;

/// Measure the Daemon against rule 7 and fail when a metric misses `budgets.json`.
#[derive(Parser)]
struct Args {
    /// Fresh Daemons to measure; each metric is the median over them.
    #[arg(long, default_value_t = 11)]
    runs: usize,
    /// Calls per latency measurement.
    #[arg(long, default_value_t = 1000)]
    calls: usize,
    #[arg(long, default_value = "target/release/rupd")]
    rupd: PathBuf,
    #[arg(long, default_value = "crates/perf/budgets.json")]
    budgets: PathBuf,
    #[arg(long, default_value = "target/perf.json")]
    out: PathBuf,
}

async fn run(args: &Args) -> Result<bool, Box<dyn std::error::Error>> {
    let budgets: Budgets = serde_json::from_slice(
        &std::fs::read(&args.budgets)
            .map_err(|err| format!("{}: {err}", args.budgets.display()))?,
    )
    .map_err(|err| format!("{}: {err}", args.budgets.display()))?;
    let cpus = std::thread::available_parallelism()?.get();
    let mut loads = Vec::new();
    let mut per_run: BTreeMap<String, Vec<f64>> = BTreeMap::new();

    for turn in 1..=args.runs {
        let load = System::load_average().one;

        eprintln!("run {turn}/{}: load {load:.1} on {cpus} cpus", args.runs);
        loads.push(load);

        for (name, value) in one_run(&args.rupd, args.calls).await? {
            per_run.entry(name).or_default().push(value);
        }
    }

    let result = Report {
        os: std::env::consts::OS.into(),
        cpus,
        runs: args.runs,
        loads,
        metrics: per_run
            .iter()
            .map(|(name, values)| (name.clone(), median(values)))
            .collect(),
        per_run,
    };

    if let Some(dir) = args.out.parent() {
        std::fs::create_dir_all(dir)?;
    }

    std::fs::write(&args.out, serde_json::to_vec_pretty(&result)?)?;

    let missed = misses(&result, &budgets)?;

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
        args.out.display()
    );

    Ok(missed.is_empty())
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(&Args::parse()).await {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(err) => {
            eprintln!("perf: {err}");
            ExitCode::from(2)
        }
    }
}
