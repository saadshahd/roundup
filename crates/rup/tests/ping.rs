use tokio::net::UnixListener;

#[tokio::test]
async fn ping_round_trips_through_the_daemon() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("rupd.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    tokio::spawn(rupd::serve(listener));

    let reply = rpc::call(&socket, "daemon.ping").await.unwrap();

    assert_eq!(reply["result"]["pong"], true);
}
