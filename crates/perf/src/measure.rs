use std::collections::BTreeMap;
use std::io;
use std::os::unix::fs::PermissionsExt;
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

/// H15's tool-use loop: 100 tool uses, each running `rup signal` for `PreToolUse` and again for
/// `PostToolUse`, since `STATE_EVENTS` keeps both.
const HOOK_LOOP_CALLS: usize = 200;

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

/// From the write call to the `terminal.output` event it causes; the program is `cat` on a raw tty with echo off, so a byte only comes back if `cat` read and rewrote it.
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

/// H15: run the built `rup signal` `calls` times, one after another (Claude Code answers one hook at
/// a time, spike finding 8), and return the wall time. A call that exits non-zero fails the metric:
/// a `rup signal` that always fails would otherwise report a fast, passing loop.
pub async fn hook_calls(
    rup: &Path,
    socket: &Path,
    id: &str,
    payload: &Path,
    calls: usize,
) -> io::Result<Duration> {
    let started = Instant::now();

    for _ in 0..calls {
        let status = Command::new(rup)
            .args(["signal", id])
            .env("RUPD_SOCKET", socket)
            .stdin(std::fs::File::open(payload)?)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await
            .map_err(|err| io::Error::other(format!("{}: {err}", rup.display())))?;

        if !status.success() {
            return Err(io::Error::other(format!("rup signal exited {status}")));
        }
    }

    Ok(started.elapsed())
}

/// H15: the cost of a 100-tool-use loop's hook processes against a fresh Daemon, as the wall time
/// of `HOOK_LOOP_CALLS` `rup signal` runs. A Daemon and Agent of its own, so this metric shares no
/// state with `one_run`'s.
async fn hook_loop_ms(rupd: &Path, rup: &Path) -> io::Result<f64> {
    let dir = tempfile::tempdir()?;
    let socket = dir.path().join("rupd.sock");
    let claude = dir.path().join("fake-claude.sh");
    std::fs::write(&claude, "#!/bin/sh\nsleep 300\n")?;
    std::fs::set_permissions(&claude, std::fs::Permissions::from_mode(0o755))?;
    let payload = dir.path().join("post-tool-use.json");
    std::fs::write(&payload, r#"{"hook_event_name":"PostToolUse"}"#)?;

    let mut child = Command::new(rupd)
        .arg(dir.path())
        .env("RUPD_SOCKET", &socket)
        .env("CLAUDE_CONFIG_DIR", dir.path().join("claude-config"))
        .env("ROUNDUP_RUP_BIN", rup)
        .env("ROUNDUP_CLAUDE_BIN", &claude)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|err| io::Error::other(format!("{}: {err}", rupd.display())))?;

    first_ping(&socket, &mut child).await?;

    let client = Client::connect(&socket).await?;
    let agent = ask(
        &client,
        "agent.spawn",
        json!({ "cwd": dir.path().to_string_lossy(), "prompt": null, "parent": null }),
    )
    .await?;
    let id = agent["id"]
        .as_str()
        .ok_or_else(|| io::Error::other("agent.spawn returned no id"))?;

    let elapsed = hook_calls(rup, &socket, id, &payload, HOOK_LOOP_CALLS).await?;

    Ok(ms(elapsed))
}

/// One fresh Daemon on an empty Project: cold start, memory with ten login-shell Terminals, and call latencies with 10 and 40 Groups on the Rail.
pub async fn one_run(rupd: &Path, rup: &Path, calls: usize) -> io::Result<Sample> {
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
        .spawn()
        .map_err(|err| io::Error::other(format!("{}: {err}", rupd.display())))?;

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

    // The Terminal's first output is `ready`, printed once `stty` has made the tty raw: before that a write is line-edited, and more than a line of them is refused.
    let mut events = Client::connect(&socket).await?;

    ask(&events, "events.subscribe", Value::Null).await?;

    let echo = ask(
        &client,
        "terminal.spawn",
        json!({ "cwd": cwd, "command": ["/bin/sh", "-c", "stty raw -echo; echo ready; exec cat"], "env": {}, "cols": 80, "rows": 24 }),
    )
    .await?;
    let terminal = echo["id"]
        .as_str()
        .ok_or_else(|| io::Error::other("terminal.spawn returned no id"))?;

    wait_for_output(&mut events, terminal).await?;

    sample.insert(
        "write_to_output_p95_ms".into(),
        percentile(
            write_to_output(&client, &mut events, terminal, calls).await?,
            0.95,
        ),
    );

    // Last: its echoes are never read, so they must not sit in front of a write_to_output sample.
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

    sample.insert("hook_loop_ms".into(), hook_loop_ms(rupd, rup).await?);

    Ok(sample)
}
