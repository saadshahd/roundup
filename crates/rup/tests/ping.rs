use std::sync::Arc;

use tokio::net::UnixListener;

#[tokio::test]
async fn ping_round_trips_through_the_daemon() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("rupd.sock");
    let daemon = Arc::new(rupd::Daemon::open(&dir.path().join(".roundup")).unwrap());
    tokio::spawn(rupd::serve(UnixListener::bind(&socket).unwrap(), daemon));

    let client = rpc::Client::connect(&socket).await.unwrap();
    let reply = client
        .request("daemon.ping", serde_json::Value::Null)
        .await
        .unwrap();

    assert_eq!(reply["pong"], true);
}
