use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use contracts::{Event, EventData};
use rpc::Client;
use serde_json::{Value, json};
use sysinfo::{Pid, ProcessesToUpdate, System};
use tokio::process::{Child, Command};

const READY_WITHIN: Duration = Duration::from_secs(10);
const SETTLE: Duration = Duration::from_millis(500);
const WARMUP: usize = 100;
const TERMINALS: usize = 10;
const TREE_SIZES: [usize; 2] = [10, 40];

/// Metrics of one Daemon, by name.
pub type Sample = BTreeMap<String, f64>;

fn ms(elapsed: Duration) -> f64 {
    elapsed.as_secs_f64() * 1000.0
}

/// Nearest-rank percentile, `p` in 0..=1.
fn percentile(mut values: Vec<f64>, p: f64) -> f64 {
    values.sort_by(f64::total_cmp);

    values[((p * values.len() as f64).ceil() as usize).max(1) - 1]
}

fn resident_mb(system: &mut System, pid: u32) -> io::Result<f64> {
    let pid = Pid::from_u32(pid);

    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);

    system
        .process(pid)
        .map(|process| process.memory() as f64 / 1_048_576.0)
        .ok_or_else(|| io::Error::other("rupd is gone"))
}

fn call_failed(method: &str, err: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("{method}: {err}"))
}

async fn ask(client: &Client, method: &str, params: Value) -> io::Result<Value> {
    client
        .request(method, params)
        .await
        .map_err(|err| call_failed(method, err))
}

async fn latencies(
    client: &Client,
    method: &str,
    params: Value,
    calls: usize,
) -> io::Result<Vec<f64>> {
    for _ in 0..WARMUP {
        ask(client, method, params.clone()).await?;
    }

    let mut taken = Vec::with_capacity(calls);

    for _ in 0..calls {
        let started = Instant::now();

        ask(client, method, params.clone()).await?;
        taken.push(ms(started.elapsed()));
    }

    Ok(taken)
}

async fn wait_for_output(events: &mut Client, terminal: &str) -> io::Result<Instant> {
    loop {
        match events.next_event().await {
            Some(Event {
                data: EventData::TerminalOutput(output),
                ..
            }) if output.id == terminal => return Ok(Instant::now()),
            Some(_) => {}
            None => return Err(io::Error::other("the Daemon closed the event connection")),
        }
    }
}

/// From the write call to the `terminal.output` event it causes; a tty echoes a typed byte at once.
async fn write_to_output(
    writer: &Client,
    events: &mut Client,
    terminal: &str,
    calls: usize,
) -> io::Result<Vec<f64>> {
    let params = json!({ "id": terminal, "data": "YQ==" });
    let mut taken = Vec::with_capacity(calls);

    for turn in 0..WARMUP + calls {
        let started = Instant::now();
        let (reply, seen) = tokio::join!(
            ask(writer, "terminal.write", params.clone()),
            wait_for_output(events, terminal)
        );

        reply?;

        if turn >= WARMUP {
            taken.push(ms(seen?.duration_since(started)));
        }
    }

    Ok(taken)
}

async fn first_ping(socket: &Path, child: &mut Child) -> io::Result<()> {
    let give_up = Instant::now() + READY_WITHIN;

    loop {
        if let Some(status) = child.try_wait()? {
            return Err(io::Error::other(format!(
                "rupd exited before it answered: {status}"
            )));
        }

        if let Ok(client) = Client::connect(socket).await
            && client.request("daemon.ping", Value::Null).await.is_ok()
        {
            return Ok(());
        }

        if Instant::now() > give_up {
            return Err(io::Error::other("rupd did not answer daemon.ping in time"));
        }

        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

/// One fresh Daemon on an empty Project: cold start, memory with ten login-shell Terminals, and call latencies with 10 and 40 Groups on the Rail.
pub async fn one_run(rupd: &Path, calls: usize) -> io::Result<Sample> {
    let dir = tempfile::tempdir()?;
    let socket = dir.path().join("rupd.sock");
    let cwd = dir.path().to_string_lossy().into_owned();
    let mut sample = Sample::new();

    let started = Instant::now();
    let mut child = Command::new(rupd)
        .arg(dir.path())
        .env("RUPD_SOCKET", &socket)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()?;

    first_ping(&socket, &mut child).await?;
    sample.insert("cold_start_ms".into(), ms(started.elapsed()));

    let pid = child
        .id()
        .ok_or_else(|| io::Error::other("rupd has no pid"))?;
    let mut system = System::new();
    let client = Client::connect(&socket).await?;

    tokio::time::sleep(SETTLE).await;

    let idle = resident_mb(&mut system, pid)?;

    for _ in 0..TERMINALS {
        ask(
            &client,
            "terminal.spawn",
            json!({ "cwd": cwd, "command": null, "env": {}, "cols": 80, "rows": 24 }),
        )
        .await?;
    }

    tokio::time::sleep(SETTLE).await;
    sample.insert("rss_extra_mb".into(), resident_mb(&mut system, pid)? - idle);

    let mut nodes = 0;

    for size in TREE_SIZES {
        for index in nodes..size {
            ask(
                &client,
                "rail.createGroup",
                json!({ "name": format!("g{index}"), "parent": null }),
            )
            .await?;
        }

        nodes = size;
        sample.insert(
            format!("rail_tree_{size}_p95_ms"),
            percentile(
                latencies(&client, "rail.tree", Value::Null, calls).await?,
                0.95,
            ),
        );
    }

    sample.insert(
        "ping_p95_ms".into(),
        percentile(
            latencies(&client, "daemon.ping", Value::Null, calls).await?,
            0.95,
        ),
    );

    let echo = ask(
        &client,
        "terminal.spawn",
        json!({ "cwd": cwd, "command": ["/bin/cat"], "env": {}, "cols": 80, "rows": 24 }),
    )
    .await?;
    let terminal = echo["id"]
        .as_str()
        .ok_or_else(|| io::Error::other("terminal.spawn returned no id"))?;

    sample.insert(
        "terminal_write_p95_ms".into(),
        percentile(
            latencies(
                &client,
                "terminal.write",
                json!({ "id": terminal, "data": "YQ==" }),
                calls,
            )
            .await?,
            0.95,
        ),
    );

    let mut events = Client::connect(&socket).await?;

    ask(&events, "events.subscribe", Value::Null).await?;
    sample.insert(
        "write_to_output_p95_ms".into(),
        percentile(
            write_to_output(&client, &mut events, terminal, calls).await?,
            0.95,
        ),
    );

    Ok(sample)
}
