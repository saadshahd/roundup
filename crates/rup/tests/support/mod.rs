//! Shared support for tests that start a rupd: `bind` it to a socket, then — for the one caller
//! that cannot otherwise tell a rupd started as a child process is actually serving —
//! `wait_for_ping` until it answers `daemon.ping`, rather than treating a bare connect as ready.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::UnixListener;

const READY_BOUND: Duration = Duration::from_secs(10);

/// Open a Daemon rooted at `dir` and bind it to `socket`. The caller decides how to run it: in
/// the background (`tokio::spawn`) for a client-driven test, or awaited directly when the
/// current process *is* the Daemon's event loop (a subprocess re-exec).
pub fn bind(dir: &Path, socket: &Path) -> (Arc<rupd::Daemon>, UnixListener) {
    let daemon = Arc::new(rupd::Daemon::open(&dir.join(".roundup")).unwrap());
    let listener = UnixListener::bind(socket).unwrap();
    (daemon, listener)
}

/// Connect to `socket` and return once rupd answers `daemon.ping`, bounded so a rupd that never
/// comes up fails the test instead of hanging it.
pub async fn wait_for_ping(socket: &Path) -> rpc::Client {
    tokio::time::timeout(READY_BOUND, async {
        loop {
            if let Ok(client) = rpc::Client::connect(socket).await
                && client
                    .request("daemon.ping", serde_json::Value::Null)
                    .await
                    .is_ok()
            {
                return client;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("rupd never answered daemon.ping")
}
