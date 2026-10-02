//! Shared support for tests that start a rupd: opens a Daemon rooted at a directory and binds it
//! to a socket.

use std::path::Path;
use std::sync::Arc;

use tokio::net::UnixListener;

/// Open a Daemon rooted at `dir` and bind it to `socket`. The caller decides how to run it: in
/// the background (`tokio::spawn`) for a client-driven test, or awaited directly when the
/// current process *is* the Daemon's event loop (a subprocess re-exec).
pub fn open_and_bind(dir: &Path, socket: &Path) -> (Arc<rupd::Daemon>, UnixListener) {
    let daemon = Arc::new(rupd::Daemon::open(&dir.join(".roundup")).unwrap());
    let listener = UnixListener::bind(socket).unwrap();
    (daemon, listener)
}
