//! The one way a test starts a `rupd`: spawn the binary on a temp Project with its own
//! `RUPD_SOCKET`, send the attached handshake (H18) when asked, and return once `daemon.ping`
//! answers on that socket. Both `crates/rupd/tests` and `crates/rup/tests` include this file by
//! path (`#[path = ".../support/rupd_harness.rs"]`); neither keeps a copy.
#![allow(dead_code)] // each includer uses its own part of the handle

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdin, Command, Stdio};
use std::time::{Duration, Instant};

/// How long a started Daemon has to answer `daemon.ping`.
const START_BOUND: Duration = Duration::from_secs(15);

/// A running Daemon; dropping it kills and reaps it.
pub struct Rupd {
    pub child: Child,
    /// Piped when attached; the Daemon exits, and stops its programs, when it closes.
    pub stdin: Option<ChildStdin>,
    /// Piped; a test that never reads it must drain it or the Daemon's logging blocks.
    pub stderr: Option<ChildStderr>,
    pub socket: PathBuf,
}

/// Spawn `rupd` (the binary at `rupd`) on `project` and return once it answers `daemon.ping`.
/// `configure` adds environment or stdio before the spawn. A miss, or a Daemon that exits first,
/// panics with the Daemon's stderr so far.
pub fn start(
    rupd: &Path,
    project: &Path,
    attached: bool,
    configure: impl FnOnce(&mut Command),
) -> Rupd {
    let socket = project.join("rupd.sock");
    let mut command = Command::new(rupd);
    command
        .arg(project)
        .env("RUPD_SOCKET", &socket)
        .stderr(Stdio::piped());
    if attached {
        command.arg("--attached").stdin(Stdio::piped());
    }
    configure(&mut command);
    let mut child = command.spawn().unwrap();
    let mut daemon = Rupd {
        stdin: child.stdin.take(),
        stderr: child.stderr.take(),
        child,
        socket,
    };
    if attached && let Some(stdin) = daemon.stdin.as_mut() {
        writeln!(stdin, "roundup-proof 1 {}", "0".repeat(64)).unwrap();
    }
    let started = Instant::now();
    loop {
        if ping(&daemon.socket) {
            return daemon;
        }
        let exited = daemon.child.try_wait().unwrap();
        if exited.is_some() || started.elapsed() > START_BOUND {
            let _ = daemon.child.kill();
            let _ = daemon.child.wait();
            let mut stderr = String::new();
            if let Some(mut pipe) = daemon.stderr.take() {
                let _ = pipe.read_to_string(&mut stderr);
            }
            panic!(
                "rupd did not answer daemon.ping within {START_BOUND:?} (exited: {exited:?}); stderr so far:\n{stderr}"
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// One `daemon.ping` on its own runtime thread, so it works inside or outside a test runtime.
fn ping(socket: &Path) -> bool {
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(async {
                        let Ok(client) = rpc::Client::connect(socket).await else {
                            return false;
                        };
                        let reply = tokio::time::timeout(
                            Duration::from_secs(2),
                            client.request("daemon.ping", ()),
                        )
                        .await;
                        matches!(reply, Ok(Ok(pong)) if pong["pong"] == true)
                    })
            })
            .join()
            .unwrap()
    })
}

impl Drop for Rupd {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
