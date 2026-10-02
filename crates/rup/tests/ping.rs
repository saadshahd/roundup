mod support;

use std::time::Duration;

#[tokio::test]
async fn ping_round_trips_through_the_daemon() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("rupd.sock");
    let (daemon, listener) = support::bind(dir.path(), &socket);
    tokio::spawn(rupd::serve(listener, daemon));

    let client = rpc::Client::connect(&socket).await.unwrap();
    let reply = client
        .request("daemon.ping", serde_json::Value::Null)
        .await
        .unwrap();

    assert_eq!(reply["pong"], true);
}

#[tokio::test]
async fn ping_wait_for_ping_rejects_a_listener_that_never_answers() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("rupd.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    std::thread::spawn(move || {
        // Accepted and kept open, never answered: a bare connect must not look ready.
        let mut held = vec![];
        for stream in listener.incoming() {
            held.push(stream);
        }
    });

    let outcome =
        tokio::time::timeout(Duration::from_millis(300), support::wait_for_ping(&socket)).await;

    assert!(
        outcome.is_err(),
        "a connect with no daemon.ping reply must not satisfy wait_for_ping"
    );
}
