mod support;

#[tokio::test]
async fn ping_round_trips_through_the_daemon() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("rupd.sock");
    let (daemon, listener) = support::open_and_bind(dir.path(), &socket);
    tokio::spawn(rupd::serve(listener, daemon));

    let client = rpc::Client::connect(&socket).await.unwrap();
    let reply = client
        .request("daemon.ping", serde_json::Value::Null)
        .await
        .unwrap();

    assert_eq!(reply["pong"], true);
}
