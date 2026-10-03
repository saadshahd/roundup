//! Starting `rupd` for one Project and learning when it is ready or gone.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rpc::{Client, RpcError};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, Command};
use tokio::time::{sleep, timeout};

use crate::config::Config;

const STDERR_LINES_KEPT: usize = 20;
const PING_RETRY: Duration = Duration::from_millis(25);
/// A grandchild that inherited the Daemon's stderr can hold the pipe open after the Daemon is gone.
const STDERR_DRAIN_BOUND: Duration = Duration::from_secs(1);

pub struct Started {
    pub client: Client,
    /// Held and never written: its closing is how the Daemon learns the App is gone (D1).
    pub stdin: ChildStdin,
    pub child: Child,
    pub socket: OwnedSocket,
}

/// The socket file this App's Daemon creates; removed when the App is done with the Daemon, so a
/// crashed Daemon's leftover path never makes the next run's bind fail.
pub struct OwnedSocket(PathBuf);

impl OwnedSocket {
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for OwnedSocket {
    fn drop(&mut self) {
        match std::fs::remove_file(&self.0) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => eprintln!("desktop: could not remove {}: {err}", self.0.display()),
        }
    }
}

#[derive(Clone, Default)]
struct StderrTail(Arc<Mutex<VecDeque<String>>>);

impl StderrTail {
    fn push(&self, line: String) {
        let mut lines = self.0.lock().expect("stderr tail lock");
        if lines.len() == STDERR_LINES_KEPT {
            lines.pop_front();
        }
        lines.push_back(line);
    }

    fn text(&self) -> String {
        let lines = self.0.lock().expect("stderr tail lock");
        lines.iter().cloned().collect::<Vec<_>>().join("\n")
    }
}

/// Starts `rupd <project> --attached` on `socket` and returns once it answers `daemon.ping`.
pub async fn start(config: &Config, project: &Path, socket: &Path) -> Result<Started, RpcError> {
    let mut child = Command::new(&config.rupd_bin)
        .arg(project)
        .arg("--attached")
        .env("RUPD_SOCKET", socket)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
            RpcError::internal(format!("cannot start {}: {err}", config.rupd_bin.display()))
        })?;
    let stdin = child.stdin.take().expect("stdin was requested piped");
    let stderr = child.stderr.take().expect("stderr was requested piped");
    let owned_socket = OwnedSocket(socket.to_path_buf());
    let tail = StderrTail::default();
    let drain = tauri::async_runtime::spawn(drain(stderr, tail.clone()));

    let failure = tokio::select! {
        status = child.wait() => match status {
            Ok(status) => format!("exited before it answered ({})", describe(status)),
            Err(err) => format!("could not be waited on: {err}"),
        },
        ready = timeout(config.ready_bound, ping_until_answered(socket)) => match ready {
            Ok(client) => return Ok(Started { client, stdin, child, socket: owned_socket }),
            Err(_) => format!("did not answer daemon.ping within {:?}", config.ready_bound),
        },
    };
    if let Err(err) = child.kill().await {
        eprintln!("desktop: could not stop the Daemon that failed to start: {err}");
    }
    // The drain ends when the pipe closes; if it does not, the tail is as complete as it will get.
    let _ = timeout(STDERR_DRAIN_BOUND, drain).await;
    Err(RpcError::internal(format!(
        "rupd {failure}; its last stderr lines:\n{}",
        tail.text()
    )))
}

/// Calls `on_exit` with the Daemon's exit code, `None` when a signal ended it.
pub fn watch_exit(mut child: Child, on_exit: impl FnOnce(Option<i32>) + Send + 'static) {
    tauri::async_runtime::spawn(async move {
        match child.wait().await {
            Ok(status) => on_exit(status.code()),
            Err(err) => {
                eprintln!("desktop: lost track of the Daemon: {err}");
                on_exit(None);
            }
        }
    });
}

fn describe(status: ExitStatus) -> String {
    status.code().map_or_else(
        || "ended by a signal".to_owned(),
        |code| format!("exit code {code}"),
    )
}

async fn ping_until_answered(socket: &Path) -> Client {
    loop {
        if let Ok(client) = Client::connect(socket).await
            && client.request("daemon.ping", Value::Null).await.is_ok()
        {
            return client;
        }
        sleep(PING_RETRY).await;
    }
}

async fn drain(stderr: ChildStderr, tail: StderrTail) {
    let mut reader = BufReader::new(stderr);
    let mut bytes = Vec::new();
    loop {
        bytes.clear();
        match reader.read_until(b'\n', &mut bytes).await {
            Ok(0) => return,
            Ok(_) => {
                let line = String::from_utf8_lossy(&bytes).trim_end().to_owned();
                eprintln!("{line}");
                tail.push(line);
            }
            Err(err) => {
                eprintln!("desktop: stopped reading the Daemon's stderr: {err}");
                return;
            }
        }
    }
}
