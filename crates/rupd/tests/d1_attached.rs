//! D1 attached: the Daemon exits with its stdin, and takes its Terminals' programs with it.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStderr, ChildStdin, Command, Stdio};
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use contracts::EventData;
use contracts::terminal::{SpawnParams, TerminalId};

/// How long the Daemon has to stop its programs and exit after its stdin closes.
const EXIT_BOUND: Duration = Duration::from_secs(15);

struct Running {
    daemon: Child,
    stdin: Option<ChildStdin>,
    /// Held open unless a test drops it: the App's death closes stderr together with stdin.
    stderr: Option<BufReader<ChildStderr>>,
    socket: std::path::PathBuf,
    _dir: tempfile::TempDir,
}

/// Start `rupd` on a temp Project and return once it says it is serving.
fn start(extra_args: &[&str], stdin: impl FnOnce(&mut Command)) -> Running {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("rupd.sock");
    let mut command = Command::new(env!("CARGO_BIN_EXE_rupd"));
    command
        .arg(dir.path())
        .args(extra_args)
        .env("RUPD_SOCKET", &socket)
        .stderr(Stdio::piped());
    stdin(&mut command);
    let mut daemon = command.spawn().unwrap();
    if extra_args.contains(&"--attached")
        && let Some(stdin) = daemon.stdin.as_mut()
    {
        writeln!(stdin, "roundup-proof 1 {}", "ab".repeat(32)).unwrap();
    }
    let mut serving = String::new();
    let mut stderr = BufReader::new(daemon.stderr.take().unwrap());
    stderr.read_line(&mut serving).unwrap();
    assert!(
        serving.contains("serving"),
        "unexpected first line: {serving:?}"
    );
    Running {
        stdin: daemon.stdin.take(),
        daemon,
        stderr: Some(stderr),
        socket,
        _dir: dir,
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

/// Kills the program on drop, so a failing test leaks no `sleep`.
struct Program(u32);

impl Drop for Program {
    fn drop(&mut self) {
        let _ = Command::new("kill")
            .args(["-9", &self.0.to_string()])
            .stderr(Stdio::null())
            .status();
    }
}

fn piped(command: &mut Command) {
    command.stdin(Stdio::piped());
}

fn wait_for_exit(daemon: &mut Child) -> bool {
    let deadline = Instant::now() + EXIT_BOUND;
    while Instant::now() < deadline {
        if daemon.try_wait().unwrap().is_some() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn is_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success()
}

/// Start a program that ignores SIGHUP, so only a SIGKILL ends it; return its pid.
async fn spawn_stubborn_program(client: &mut rpc::Client, cwd: &Path) -> Program {
    client.request("events.subscribe", ()).await.unwrap();
    let params = SpawnParams {
        cwd: cwd.to_string_lossy().into_owned(),
        command: Some(vec![
            "/bin/sh".into(),
            "-c".into(),
            "trap '' HUP; echo pid=$$; exec sleep 1000".into(),
        ]),
        env: BTreeMap::new(),
        cols: 80,
        rows: 24,
    };
    let reply = client.request("terminal.spawn", params).await.unwrap();
    let _: TerminalId = serde_json::from_value(reply).unwrap();
    let mut printed = String::new();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let event = client.next_event().await.expect("events stay open");
            if let EventData::TerminalOutput(out) = event.data {
                printed.push_str(&String::from_utf8_lossy(
                    &STANDARD.decode(out.data).unwrap(),
                ));
                if let Some(pid) = printed
                    .lines()
                    .find_map(|line| line.trim().strip_prefix("pid="))
                    .and_then(|pid| pid.parse().ok())
                {
                    return Program(pid);
                }
            }
        }
    })
    .await
    .expect("the program prints its pid")
}

#[tokio::test]
async fn d1_attached_daemon_exits_when_stdin_closes_and_its_programs_are_gone() {
    let mut running = start(&["--attached"], piped);
    let mut client = rpc::Client::connect(&running.socket).await.unwrap();
    let program = spawn_stubborn_program(&mut client, running._dir.path()).await;
    assert!(is_alive(program.0));

    // The App's death closes both pipes at once.
    drop(running.stdin.take());
    drop(running.stderr.take());

    assert!(
        wait_for_exit(&mut running.daemon),
        "the Daemon did not exit"
    );
    assert!(!is_alive(program.0), "the program outlived the Daemon");
}

#[tokio::test]
async fn d1_attached_daemon_exits_cleanly_when_programs_end_by_themselves_during_shutdown() {
    let mut running = start(&["--attached"], piped);
    let mut client = rpc::Client::connect(&running.socket).await.unwrap();
    let program = spawn_stubborn_program(&mut client, running._dir.path()).await;
    for _ in 0..30 {
        let params = SpawnParams {
            cwd: running._dir.path().to_string_lossy().into_owned(),
            command: Some(vec!["/bin/sh".into(), "-c".into(), "sleep 0.3".into()]),
            env: BTreeMap::new(),
            cols: 80,
            rows: 24,
        };
        client.request("terminal.spawn", params).await.unwrap();
    }
    // Close stdin just as the programs reach their end, so some finish between list and kill.
    tokio::time::sleep(Duration::from_millis(250)).await;

    drop(running.stdin.take());

    assert!(
        wait_for_exit(&mut running.daemon),
        "the Daemon did not exit"
    );
    let status = running.daemon.wait().unwrap();
    assert!(status.success(), "the Daemon exited with {status}");
    assert!(!is_alive(program.0), "the program outlived the Daemon");
}

#[tokio::test]
async fn d1_attached_daemon_exits_when_stdin_is_already_closed() {
    // The handshake `start` writes is all there is: stdin then ends (a null stdin never completes one, H18).
    let mut running = start(&["--attached"], piped);
    drop(running.stdin.take());

    assert!(
        wait_for_exit(&mut running.daemon),
        "the Daemon did not exit"
    );
}

#[tokio::test]
async fn d1_without_attached_a_closed_stdin_changes_nothing() {
    let mut running = start(&[], piped);
    drop(running.stdin.take());
    tokio::time::sleep(Duration::from_millis(500)).await;

    let client = rpc::Client::connect(&running.socket).await.unwrap();
    let pong = client.request("daemon.ping", ()).await.unwrap();

    assert_eq!(pong["pong"], true);
}
