//! Keystroke-to-render in the real App (`scenarios/perf.md` R11): runs a build made with `VITE_ROUNDUP_PERF`,
//! reads the `PERF <json>` line the webview writes through its Terminal, and compares its p95 with the budget.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{ExitCode, Stdio};
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use clap::Parser;
use contracts::{Event, EventData};
use perf::gate::{conclude, read_budgets};
use perf::webview::failure;
use perf::webview::{checked, perf_line};
use rpc::Client;
use serde_json::Value;
use sysinfo::System;
use tokio::process::Command;

const RUN_BOUND: Duration = Duration::from_secs(240);

/// Measure keystroke-to-render in the App's own webview; each metric is the median over fresh App runs.
#[derive(Parser)]
struct Args {
    #[arg(long, default_value_t = 3)]
    runs: usize,
    /// The App built with `VITE_ROUNDUP_PERF`; `rupd` must sit beside it.
    #[arg(long, default_value = "target/perf-app/release/roundup")]
    app: PathBuf,
    #[arg(long, default_value = "crates/perf/keystroke-budgets.json")]
    budgets: PathBuf,
    #[arg(long, default_value = "target/perf-keystroke.json")]
    out: PathBuf,
}

/// Starts the Terminal's program: a raw tty, a `ready` line, then `cat`, so each typed byte comes back only through the program.
const SHELL: &str = "#!/bin/sh\nstty raw -echo\necho ready\nexec cat\n";

async fn read_report(socket: &Path) -> io::Result<Value> {
    let give_up = Instant::now() + RUN_BOUND;

    let mut events = loop {
        match Client::connect(socket).await {
            Ok(client) => break client,
            Err(_) if Instant::now() < give_up => {
                tokio::time::sleep(Duration::from_millis(50)).await
            }
            Err(err) => return Err(failure(format!("{}: {err}", socket.display()))),
        }
    };

    events
        .request("events.subscribe", Value::Null)
        .await
        .map_err(|err| failure(format!("events.subscribe: {err}")))?;

    let mut printed: BTreeMap<String, Vec<u8>> = BTreeMap::new();

    loop {
        let event = tokio::time::timeout_at(give_up.into(), events.next_event())
            .await
            .map_err(|_| failure("the webview wrote no PERF line in time"))?
            .ok_or_else(|| failure("the Daemon closed before the webview reported"))?;

        if let Event {
            data: EventData::TerminalOutput(output),
            ..
        } = event
        {
            let bytes = printed.entry(output.id).or_default();

            bytes.extend(
                STANDARD
                    .decode(output.data)
                    .map_err(|err| failure(format!("terminal.output: {err}")))?,
            );

            if let Some(line) = perf_line(&String::from_utf8_lossy(bytes)) {
                return serde_json::from_str(line)
                    .map_err(|err| failure(format!("the PERF line is not JSON: {err}")));
            }
        }
    }
}

/// One fresh App, one report. The App is stopped when this returns, which stops its Daemon (D1).
async fn one_run(app: &Path) -> io::Result<Value> {
    let dir = tempfile::tempdir()?;
    let shell = dir.path().join("shell");
    let project = dir.path().join("project");

    std::fs::create_dir(&project)?;
    std::fs::write(&shell, SHELL)?;
    std::fs::set_permissions(&shell, std::os::unix::fs::PermissionsExt::from_mode(0o755))?;

    let child = Command::new(app)
        .arg(&project)
        // Without this macOS naps the App a few seconds in: its timers and frames stop, and keys time out instead of rendering.
        .args(["-NSAppSleepDisabled", "YES"])
        .env("SHELL", &shell)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|err| failure(format!("{}: {err}", app.display())))?;
    let pid = child.id().ok_or_else(|| failure("the App has no pid"))?;
    let socket = std::env::temp_dir().join(format!("roundup-{pid}.sock"));
    let report = read_report(&socket).await?;

    drop(child);

    Ok(report)
}

async fn run(args: &Args) -> Result<bool, Box<dyn std::error::Error>> {
    let budgets = read_budgets(&args.budgets)?;
    let mut per_run: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut loads = Vec::new();

    for turn in 1..=args.runs {
        let load = System::load_average().one;

        eprintln!("run {turn}/{}: load {load:.1}", args.runs);
        loads.push(load);

        let report = one_run(&args.app).await?;

        checked(&report)?;
        eprintln!("  {report}");

        for (name, key) in [
            ("keystroke_p50_ms", "p50"),
            ("keystroke_p95_ms", "p95"),
            ("keystroke_p99_ms", "p99"),
        ] {
            per_run.entry(name.into()).or_default().push(
                report[key]
                    .as_f64()
                    .ok_or_else(|| failure(format!("the report has no {key}")))?,
            );
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
            eprintln!("perf-keystroke: {err}");
            ExitCode::from(2)
        }
    }
}
