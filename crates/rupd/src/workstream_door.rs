use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use agents::claude_code::Launcher;
use contracts::agent::StatusEvent;
use contracts::{Actor, EventData, Kind, Status};
use rpc::{Bus, Client};
use serde_json::{Value, json};
use tokio::net::UnixListener;

use super::{Daemon, serve};

#[tokio::test]
async fn a7_b1_b6_b9_real_daemon_fences_old_status_after_door_restart() {
    let project = tempfile::tempdir().unwrap();
    let dir = project.path().join(".roundup");
    std::fs::create_dir(&dir).unwrap();
    let fake = project.path().join("fake-claude");
    std::fs::write(&fake, "#!/bin/sh\n# A20: the launch's Attempt is in the hook command of its --settings file, not the program's env.\nattempt=$(sed -n \"s/.*ROUNDUP_ATTEMPT='\\([0-9]*\\)'.*/\\1/p\" \"$2\" | head -n 1)\nwhile read line; do printf '%s:%s\\n' \"$attempt\" \"$line\" >> \"$(dirname \"$0\")/input\"; done\n").unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let rup = project.path().join("rup");
    std::fs::write(&rup, "").unwrap();
    let bus = Bus::new();
    let terminals = Arc::new(terminal::Terminals::open(&dir, bus.clone()).unwrap());
    let agents = Arc::new(
        agents::Agents::open_with(
            &dir,
            bus.clone(),
            terminals.clone(),
            Launcher::new(
                fake.to_str().unwrap(),
                project.path().join("claude.json"),
                rup,
                None,
            ),
            agents::worktree::Git::from_env(),
        )
        .unwrap(),
    );
    let mut daemon = Daemon {
        modules: Default::default(),
        bus: bus.clone(),
        touches: Arc::new(provenance::Touches::open(&dir.join("provenance.db")).unwrap()),
        digests: Default::default(),
    };
    daemon.register(Arc::new(
        messages::Messages::open(
            &dir,
            bus.clone(),
            agents.clone(),
            Arc::new(|_, _| Box::pin(async { Err(messages::Refusal::Busy) })),
        )
        .unwrap(),
    ));
    daemon.register(agents);
    daemon.register(terminals);
    let daemon = Arc::new(daemon);
    let socket = project.path().join("rupd.sock");
    let serving = tokio::spawn(serve(UnixListener::bind(&socket).unwrap(), daemon.clone()));
    let client = Client::connect(&socket).await.unwrap();
    let workstream = client
        .request(
            "rail.createWorkstream",
            json!({"name":"workstream","parent":null}),
        )
        .await
        .unwrap();
    let id = workstream["id"].as_str().unwrap();
    let child = client
        .request(
            "agent.spawn",
            json!({"cwd":project.path(),"prompt":null,"parent":id}),
        )
        .await
        .unwrap();
    let first = client
        .request("rail.startDoor", json!({"id":id}))
        .await
        .unwrap();
    let send = json!({"to":id,"kind":"note","body":"pending"});
    let old = client.request("message.send", send.clone()).await.unwrap();
    client
        .request("takeover.begin", json!({"agent":id}))
        .await
        .unwrap();
    client
        .request("agent.stop", json!({"id":id}))
        .await
        .unwrap();
    for (method, params) in [
        ("message.send", send.clone()),
        ("takeover.begin", json!({"agent":id})),
    ] {
        assert_eq!(
            client.request(method, params).await.unwrap_err().code,
            rpc::code::CONFLICT
        );
    }
    let second = client
        .request("rail.startDoor", json!({"id":id}))
        .await
        .unwrap();
    assert_eq!(second["id"], first["id"]);
    assert_eq!(second["attempt"], "2");
    assert_ne!(second["terminal_id"], first["terminal_id"]);
    client
        .request(
            "terminal.write",
            json!({"id":second["terminal_id"],"data":"aGVsbG8K"}),
        )
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if std::fs::read_to_string(project.path().join("input"))
                .is_ok_and(|text| text.contains("2:hello"))
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let new = client.request("message.send", send.clone()).await.unwrap();
    assert_eq!(new["status"], "pending");
    client
        .request("takeover.begin", json!({"agent":id}))
        .await
        .unwrap();
    let ext = Client::connect(&socket).await.unwrap();
    ext.request(
        "daemon.identify",
        json!({"actor":{"kind":"ext","id":"observer","parent":null}}),
    )
    .await
    .unwrap();
    let held = ext.request("message.send", send).await.unwrap();
    assert_eq!(held["status"], "held");
    let mut events = Client::connect(&socket).await.unwrap();
    events
        .request("events.subscribe", Value::Null)
        .await
        .unwrap();
    bus.emit(
        Actor::daemon(),
        EventData::AgentStatus(StatusEvent {
            id: id.into(),
            attempt: "1".into(),
            status_revision: "1".into(),
            status: Status {
                kind: Kind::Done,
                label: "old exit".into(),
                since: 0,
            },
        }),
    );
    // This subscription and Messages consume the same bus in FIFO order; a current-generation
    // status plus its dropped Message is the barrier that proves the old event was consumed.
    let barrier = client
        .request(
            "message.send",
            json!({"to":child["id"],"kind":"note","body":"barrier"}),
        )
        .await
        .unwrap();
    client
        .request("agent.stop", json!({"id":child["id"]}))
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if let EventData::MessageDropped(message) = events.next_event().await.unwrap().data
                && json!(message.id) == barrier["id"]
            {
                break;
            }
        }
    })
    .await
    .unwrap();
    for (message, expected) in [(&old, "dropped"), (&new, "pending"), (&held, "held")] {
        let found = client
            .request("message.get", json!({"id":message["id"]}))
            .await
            .unwrap();
        assert_eq!(found["status"], expected);
    }
    let again = ext
        .request(
            "message.send",
            json!({"to":id,"kind":"note","body":"still held"}),
        )
        .await
        .unwrap();
    assert_eq!(again["reason"], "takeover");
    let stale = client
        .request(
            "agent.signal",
            json!({"id":id,"attempt":"1","payload":{"hook_event_name":"Stop"}}),
        )
        .await
        .unwrap();
    // A20: the earlier Attempt's Signal is ignored, never an error to the caller.
    assert_eq!(stale, Value::Null);
    let tree = client.request("rail.tree", Value::Null).await.unwrap();
    let nodes = tree.as_array().unwrap();
    assert_eq!(
        nodes.iter().find(|n| n["id"] == child["id"]).unwrap()["parent"],
        id
    );
    assert_eq!(
        nodes.iter().find(|n| n["id"] == workstream["id"]).unwrap()["terminal_id"],
        second["terminal_id"]
    );
    daemon.stop_terminals().await.unwrap();
    serving.abort();
}
