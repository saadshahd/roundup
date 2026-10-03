use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use perf::gate::{conclude, read_budgets};
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
    /// The built `rup`, run as Claude Code's hook command (H15's `hook_loop_ms`).
    #[arg(long, default_value = "target/release/rup")]
    rup: PathBuf,
    #[arg(long, default_value = "crates/perf/budgets.json")]
    budgets: PathBuf,
    #[arg(long, default_value = "target/perf.json")]
    out: PathBuf,
}

async fn run(args: &Args) -> Result<bool, Box<dyn std::error::Error>> {
    let budgets = read_budgets(&args.budgets)?;
    let cpus = std::thread::available_parallelism()?.get();
    let mut loads = Vec::new();
    let mut per_run: BTreeMap<String, Vec<f64>> = BTreeMap::new();

    for turn in 1..=args.runs {
        let load = System::load_average().one;

        eprintln!("run {turn}/{}: load {load:.1} on {cpus} cpus", args.runs);
        loads.push(load);

        for (name, value) in one_run(&args.rupd, &args.rup, args.calls).await? {
            per_run.entry(name).or_default().push(value);
        }
    }

    conclude(per_run, loads, &budgets, &args.out)
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
