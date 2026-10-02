//! `wait_for_ping` connects to a rupd's socket and waits until it actually answers `daemon.ping`,
//! rather than treating a bare connect as ready. `signal.rs` needs this: its rupd runs in a child
//! process, so the listener can be bound before that process's event loop is serving. `ping.rs`
//! tests this guarantee directly, by asserting it against a listener that never answers.

use std::path::Path;
use std::time::Duration;

const READY_BOUND: Duration = Duration::from_secs(10);

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
