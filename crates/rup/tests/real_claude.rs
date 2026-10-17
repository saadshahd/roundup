//! A26, A32, D2, E3, E6, E11, E12, F3, H15, H21, LC9, M9, Q11: real Claude at the model boundary; aimock owns matching and the request journal.
#[path = "e2e/daemon.rs"]
#[allow(dead_code)]
mod daemon;
use daemon::aimock;

use contracts::{EventData, Kind};
use daemon::{next, rail_tree};
use serde_json::{Value, json};
use std::time::Duration;

#[test]
fn e11_an_unmatched_request_fails() {
    let model = aimock::Model::start("unmatched");
    let response = std::process::Command::new("curl")
        .args(["--silent", "--show-error", "--max-time", "10", "-X", "POST"])
        .arg(format!("{}/v1/messages", model.url))
        .args(["-H", "content-type: application/json", "-H"])
        .arg(format!("x-api-key: {}", aimock::KEY))
        .arg("-d")
        .arg(
            json!({
                "model": "claude-sonnet-4-6", "max_tokens": 32,
                "messages": [{"role": "user", "content": "this turn has no fixture"}]
            })
            .to_string(),
        )
        .output()
        .unwrap();
    assert!(response.status.success());
    let journal = model.journal();
    assert_eq!(journal.len(), 1);
    assert_eq!(journal[0]["response"]["status"], 503, "{journal:#?}");
    assert!(journal[0]["response"]["fixture"].is_null());
}

#[tokio::test]
async fn e12_model_routing_keeps_credentials_out_of_hooks_and_mcp() {
    use std::os::unix::fs::PermissionsExt;
    let probe = tempfile::tempdir().unwrap();
    let wrapper = probe.path().join("rup-observer");
    let log = probe.path().join("environment.jsonl");
    std::fs::write(
        &wrapper,
        r##"#!/usr/bin/env python3
import json, os, sys
key = "sk-mock-key-000000000000000000000001"
with open(os.environ["REAL_CLAUDE_ENV_LOG"], "a") as log:
    log.write(json.dumps({"command": sys.argv[1], "credentials": [
        name for name, value in os.environ.items()
        if name in ("ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN") or key in value
    ]}) + "\n")
os.execv(os.environ["REAL_CLAUDE_RUP_BIN"], [os.environ["REAL_CLAUDE_RUP_BIN"], *sys.argv[1:]])
"##,
    )
    .unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let project = daemon::start_real(
        "e12_model_routing",
        &[
            ("ROUNDUP_RUP_BIN", wrapper.to_str().unwrap()),
            ("REAL_CLAUDE_RUP_BIN", env!("CARGO_BIN_EXE_rup")),
            ("REAL_CLAUDE_ENV_LOG", log.to_str().unwrap()),
            ("ANTHROPIC_API_KEY", "parent-key-must-be-stripped"),
            ("ANTHROPIC_AUTH_TOKEN", "parent-token-must-be-stripped"),
        ],
    );
    let client = project.client().await;
    client
        .request(
            "agent.spawn",
            json!({
                "cwd": project.dir.path(), "prompt": "E10 create split checkout"
            }),
        )
        .await
        .unwrap();
    project
        .model
        .as_ref()
        .unwrap()
        .tool_result("toolu_create_todo")
        .await;
    // Aimock redacts credentials in its journal. Require its inbound-key gate instead:
    // real Claude's successful tool round above used the dummy key; a wrong one is refused.
    let refused = std::process::Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--max-time",
            "10",
            "--noproxy",
            "*",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            "-X",
            "POST",
            "-H",
            "x-api-key: wrong-key",
        ])
        .arg(format!(
            "{}/v1/messages",
            project.model.as_ref().unwrap().url
        ))
        .output()
        .unwrap();
    assert!(refused.status.success());
    assert_eq!(String::from_utf8(refused.stdout).unwrap(), "401");
    drop(project); // Observe shutdown hooks too.
    let records: Vec<serde_json::Value> = std::fs::read_to_string(log)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    for command in ["signal", "mcp"] {
        assert!(
            records.iter().any(|entry| entry["command"] == command),
            "no {command}: {records:?}"
        );
    }
    assert!(
        records
            .iter()
            .all(|entry| entry["credentials"].as_array().unwrap().is_empty()),
        "credential reached a hook or MCP: {records:?}"
    );
}

#[tokio::test]
async fn e6_a_shim_started_at_once_goes_pending_to_up_and_never_missing() {
    use contracts::EventData;
    use contracts::agent::{Channel, RailNode};
    use std::time::Duration;

    let project = daemon::start_real("e6_immediate_shim", &[]);
    let mut events = project.subscribed().await;

    let node: RailNode = serde_json::from_value(
        tokio::time::timeout(
            Duration::from_secs(30),
            events.request(
                "agent.spawn",
                json!({
                    "cwd": project.dir.path(), "prompt": "E10 immediate shim"
                }),
            ),
        )
        .await
        .expect("agent.spawn answers within the bound")
        .unwrap(),
    )
    .unwrap();
    assert_eq!(node.channel, Some(Channel::Pending));

    let started = std::time::Instant::now();
    let mut channels = vec![Channel::Pending];
    loop {
        let channel = daemon::next(&mut events, |data| match data {
            EventData::AgentChannel(event) if event.id == node.id => Some(event.channel),
            _ => None,
        })
        .await;
        eprintln!(
            "E6 channel {channel:?} at {:?} after spawn response",
            started.elapsed()
        );
        channels.push(channel);
        if channel == Channel::Up {
            break;
        }
    }
    assert_eq!(channels, [Channel::Pending, Channel::Up]);
    // Observe events across the Daemon's full default deadline: a final tree read
    // alone would miss a transient Missing followed by Up.
    assert!(
        tokio::time::timeout(agents::CHANNEL_DEADLINE, async {
            loop {
                let channel = daemon::next(&mut events, |data| match data {
                    EventData::AgentChannel(event) if event.id == node.id => Some(event.channel),
                    _ => None,
                })
                .await;
                assert_eq!(channel, Channel::Up, "Channel changed after Up");
            }
        })
        .await
        .is_err()
    );
    assert_eq!(
        daemon::rail_tree(&events)
            .await
            .into_iter()
            .find(|entry| entry.id == node.id)
            .unwrap()
            .channel,
        Some(Channel::Up)
    );
    let model = project.model.as_ref().unwrap();
    tokio::time::timeout(Duration::from_secs(30), async {
        while !model
            .journal()
            .iter()
            .any(|entry| entry["body"]["tools"].is_array())
        {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("real Claude reaches the model");
    model.assert_matched();
}

#[tokio::test]
async fn e3_real_claude_runs_both_session_start_commands_and_reads_the_context() {
    let project = daemon::start_real("e3_startup_context", &[]);
    let user = project.client().await;
    let mut events = project.subscribed().await;
    let node = user
        .request(
            "agent.spawn",
            json!({
                "cwd": project.dir.path(), "prompt": "E3"
            }),
        )
        .await
        .unwrap();
    let model = project.model.as_ref().unwrap();
    let request = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Some(entry) = model
                .journal()
                .into_iter()
                .find(|entry| entry["body"]["tools"].is_array())
            {
                return entry;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("real Claude requests its first work turn");
    let messages = request["body"]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|message| message["content"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(messages.contains("Parent: none"), "{request:#?}");
    assert!(
        messages.contains(&format!(
            "Your Agent id is {}. It never changes.",
            node["id"].as_str().unwrap()
        )),
        "{request:#?}"
    );
    for line in ["Ask: the user", "Peers: none.", "Todos: none open."] {
        assert!(messages.contains(line), "{request:#?}");
    }
    let brief = user
        .request("agent.brief", json!({"id": node["id"]}))
        .await
        .unwrap();
    let hook: Value = serde_json::from_str(brief["stdout"].as_str().unwrap()).unwrap();
    assert_eq!(hook["hookSpecificOutput"]["hookEventName"], "SessionStart");
    assert!(
        messages.contains(
            hook["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .unwrap()
        )
    );
    for kind in [Kind::Working, Kind::Idle] {
        next(&mut events, |event| match event {
            EventData::AgentStatus(status)
                if status.id == node["id"].as_str().unwrap() && status.status.kind == kind =>
            {
                Some(())
            }
            _ => None,
        })
        .await;
    }
    model.assert_matched();
    assert_eq!(
        model
            .journal()
            .iter()
            .filter(|entry| entry["body"]["tools"].is_array())
            .count(),
        1
    );
    assert_eq!(
        rail_tree(&user)
            .await
            .into_iter()
            .find(|item| item.id == node["id"].as_str().unwrap())
            .unwrap()
            .name,
        "e3"
    );
}

#[tokio::test]
async fn d2_the_first_prompt_names_the_agent() {
    let project = daemon::start_real("d2_the_first_prompt_names_the_agent", &[]);
    let mut client = project.subscribed().await;
    let agent: contracts::agent::RailNode = serde_json::from_value(
        tokio::time::timeout(
            Duration::from_secs(30),
            client.request(
                "agent.spawn",
                json!({"cwd": project.dir.path(), "prompt": "fix the refresh race"}),
            ),
        )
        .await
        .expect("agent.spawn answers within the bound")
        .unwrap(),
    )
    .unwrap();
    for kind in [Kind::Working, Kind::Idle] {
        next(&mut client, |event| match event {
            EventData::AgentStatus(status)
                if status.id == agent.id && status.status.kind == kind =>
            {
                Some(())
            }
            _ => None,
        })
        .await;
    }
    let tree = rail_tree(&client).await;
    let named: Vec<_> = tree.iter().filter(|node| node.id == agent.id).collect();
    assert_eq!(named.len(), 1);
    assert_eq!(named[0].name, "fix-refresh-race");
    let model = project.model.as_ref().unwrap();
    model.assert_matched();
    assert!(
        model.journal().iter().any(|entry| {
            entry["body"]["tools"].is_array()
                && entry["body"]["messages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|message| {
                        message["role"] == "user"
                            && message["content"]
                                .as_str()
                                .is_some_and(|text| text.contains("fix the refresh race"))
                    })
        }),
        "real Claude sends the first prompt to aimock"
    );
}

#[tokio::test]
async fn d2_status_follows_the_hooks_in_order() {
    let project = daemon::start_real("d2_status_follows_the_hooks_in_order", &[]);
    let mut client = project.subscribed().await;
    let agent: contracts::agent::RailNode = serde_json::from_value(
        tokio::time::timeout(
            Duration::from_secs(30),
            client.request(
                "agent.spawn",
                json!({"cwd": project.dir.path(), "prompt": "fix the refresh race"}),
            ),
        )
        .await
        .expect("agent.spawn answers within the bound")
        .unwrap(),
    )
    .unwrap();
    let mut kinds = Vec::new();
    for _ in 0..5 {
        kinds.push(daemon::next_kind(&mut client).await);
        if kinds.len() == 3 {
            let decisions: Vec<contracts::decision::Decision> = serde_json::from_value(
                tokio::time::timeout(
                    Duration::from_secs(30),
                    client.request("decision.list", Value::Null),
                )
                .await
                .expect("decision.list answers within the bound")
                .unwrap(),
            )
            .unwrap();
            let decision = decisions
                .iter()
                .find(|decision| decision.agent == agent.id)
                .unwrap();
            assert_eq!(decision.tool, "Bash");
            tokio::time::timeout(
                Duration::from_secs(30),
                client.request(
                    "decision.answer",
                    json!({
                        "id": decision.id, "answer": "allow", "proof": "0".repeat(64)
                    }),
                ),
            )
            .await
            .expect("decision.answer answers within the bound")
            .unwrap();
        }
    }
    assert_eq!(
        kinds,
        [
            Kind::Idle,
            Kind::Working,
            Kind::NeedsYou,
            Kind::Working,
            Kind::Idle
        ]
    );
    let model = project.model.as_ref().unwrap();
    model.tool_result("toolu_d2_touch").await;
    assert!(project.dir.path().join("d2-permitted").is_file());
    model.assert_matched();
}

#[tokio::test]
async fn m9_a_door_asked_for_a_workstream_makes_one() {
    let project = daemon::start_real("m9_create_workstream", &[]);
    let mut events = project.subscribed().await;
    let door = events
        .request(
            "rail.createWorkstream",
            json!({"name": "team", "parent": null}),
        )
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    events
        .request("rail.startDoor", json!({"id": door}))
        .await
        .unwrap();
    // SessionStart also emits Idle; observe a completed turn before reusing its bundle.
    for kind in [Kind::Working, Kind::Idle] {
        next(&mut events, |event| match event {
            EventData::AgentStatus(status) if status.id == door && status.status.kind == kind => {
                Some(())
            }
            _ => None,
        })
        .await;
    }
    events.request("message.send", json!({"to": door, "kind": "note", "body": "E10 create another Workstream named x and start its Door"})).await.unwrap();
    let model = project.model.as_ref().unwrap();
    let result = model.tool_result("toolu_m9_create").await;
    let made: contracts::agent::RailNode =
        serde_json::from_str(result["content"].as_str().unwrap()).unwrap();
    assert_eq!(made.name, "x");
    model.tool_result("toolu_m9_start").await;
    let tree = rail_tree(&events).await;
    let node = tree.iter().find(|n| n.id == made.id).unwrap();
    assert!(node.parent.is_none());
    assert!(node.attempt.is_some(), "its Door is started");
    assert!(
        tree.iter().any(|n| n.id == door),
        "the Door that asked stays"
    );
    model.assert_matched();
}

#[tokio::test]
async fn q11_a_loaded_bundle_allows_a_subsequent_agent() {
    let project = daemon::start_real("m9_create_workstream", &[]);
    let mut events = project.subscribed().await;
    let door = events
        .request(
            "rail.createWorkstream",
            json!({"name": "team", "parent": null}),
        )
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    events
        .request("rail.startDoor", json!({"id": door}))
        .await
        .unwrap();
    // SessionStart also emits Idle; observe a completed turn before reusing its bundle.
    for kind in [Kind::Working, Kind::Idle] {
        next(&mut events, |event| match event {
            EventData::AgentStatus(status) if status.id == door && status.status.kind == kind => {
                Some(())
            }
            _ => None,
        })
        .await;
    }
    let agent = events
        .request(
            "agent.spawn",
            json!({
                "cwd": project.dir.path(), "prompt": "What is this Workstream for?"
            }),
        )
        .await
        .unwrap();
    for kind in [Kind::Working, Kind::Idle] {
        next(&mut events, |event| match event {
            EventData::AgentStatus(status)
                if status.id == agent["id"].as_str().unwrap() && status.status.kind == kind =>
            {
                Some(())
            }
            _ => None,
        })
        .await;
    }
    let model = project.model.as_ref().unwrap();
    model.assert_matched();
    assert_eq!(
        model
            .journal()
            .iter()
            .filter(|entry| entry["body"]["tools"].is_array())
            .count(),
        2,
        "both real Claude programs reached the model boundary"
    );
}

#[tokio::test]
async fn b37_real_claude_receives_d1_after_a_scripted_server_error() {
    let mut project = daemon::start_real("b37_server_error", &[]);
    let user = project.client().await;
    let mut events = project.subscribed().await;
    let door = user
        .request("rail.createWorkstream", json!({"name":"B37 supervisor"}))
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    user.request("rail.startDoor", json!({"id":door}))
        .await
        .unwrap();
    next(&mut events, |data| match data {
        EventData::AgentStatus(s) if s.id == door && s.status.kind == Kind::Idle => Some(()),
        _ => None,
    })
    .await;
    let child = user
        .request(
            "agent.spawn",
            json!({"parent":door,"cwd":project.dir.path(),"name":"B37 child","prompt":"B37 prepare child"}),
        )
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    project
        .model
        .as_ref()
        .unwrap()
        .tool_result("toolu_b37_child_ready")
        .await;
    next(&mut events, |data| match data {
        EventData::AgentStatus(s)
            if s.id == child && s.status.kind == Kind::Working && s.status.label == "working" =>
        {
            Some(())
        }
        _ => None,
    })
    .await;
    next(&mut events, |data| match data {
        EventData::AgentStatus(s) if s.id == child && s.status.kind == Kind::Idle => Some(()),
        _ => None,
    })
    .await;
    // The push window closes on the child's final idle Digest, not its earlier Rail status.
    next(&mut events, |data| match data {
        EventData::MessageDelivered(m)
            if m.to == door
                && m.body.contains("[digest] B37 child is idle")
                && m.body.contains("open todos: 1") =>
        {
            Some(())
        }
        _ => None,
    })
    .await;
    // B50(c): the transcript records the Digest after the turn it starts may already be over, so the
    // door's idle edge can precede the Delivered event; the Rail poll below waits for idle instead.
    // Hold the child's Digests as well as its Decision note: otherwise needs-you can
    // start an intervening turn before tell_doors has queued the note.
    user.request("takeover.begin", json!({"agent":door}))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let nodes = rail_tree(&user).await;
            let node = nodes.iter().find(|n| n.id == door).unwrap();
            if node.status.as_ref().unwrap().kind == Kind::Idle {
                match user
                    .request(
                        "agent.prompt",
                        json!({"id":door,"text":"B37 scripted server error"}),
                    )
                    .await
                {
                    Ok(_) => break,
                    Err(e) if e.code == rpc::code::BUSY => {}
                    Err(e) => panic!("{e}"),
                }
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let failed = next(&mut events, |data| match data {
        EventData::AgentStatus(s) if s.id == door && s.status.kind == Kind::Error => Some(s),
        _ => None,
    })
    .await;
    assert_eq!(failed.status.label, "server_error");
    let nodes = rail_tree(&user).await;
    let live = nodes.iter().find(|n| n.id == door).unwrap();
    assert!(live.terminal_id.is_some());
    assert_eq!(live.attempt.as_deref(), Some(failed.attempt.as_str()));
    let caller = project.client().await;
    let child_id = child.clone();
    let permission = tokio::spawn(async move {
        caller.request("agent.permission", json!({"id":child_id,"payload":{"hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"echo B37"}}})).await
    });
    let decision = next(&mut events, |data| match data {
        EventData::DecisionOpened(d) if d.agent == child => Some(d),
        _ => None,
    })
    .await;
    assert_eq!(decision.id, "d1");
    next(&mut events, |data| match data {
        EventData::MessageHeld(m) if m.to == door && m.body.starts_with("[decision d1]") => {
            Some(())
        }
        _ => None,
    })
    .await;
    user.request(
        "decision.answer",
        json!({"id":"d1","answer":"allow","proof":"0".repeat(64)}),
    )
    .await
    .unwrap();
    assert!(permission.await.unwrap().is_ok());
    user.request("takeover.end", json!({"agent":door}))
        .await
        .unwrap();
    next(&mut events, |data| match data {
        EventData::MessageDelivered(m) if m.to == door && m.body.starts_with("[decision d1]") => {
            Some(())
        }
        _ => None,
    })
    .await;
    // Polled, not awaited as an event: the transcript records the note after its turn may be over.
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let nodes = rail_tree(&user).await;
            let node = nodes.iter().find(|n| n.id == door).unwrap();
            if node.status.as_ref().unwrap().kind == Kind::Idle
                && node.status_revision.as_deref() != Some(failed.status_revision.as_str())
            {
                assert_eq!(node.attempt.as_deref(), Some(failed.attempt.as_str()));
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    // This observer expects one failed model request; the shared success-only Drop check does not.
    let model = project.model.take().unwrap();
    let journal = model.journal();
    assert_eq!(
        journal
            .iter()
            .filter(|e| e["response"]["status"] == 500)
            .count(),
        1,
        "{journal:#?}"
    );
    assert!(
        journal.iter().all(|e| !e["response"]["fixture"].is_null()),
        "{journal:#?}"
    );
    let failed_request = journal
        .iter()
        .position(|e| e["response"]["status"] == 500)
        .unwrap();
    let identity = &journal[failed_request]["headers"]["x-claude-code-session-id"];
    assert!(identity.is_string());
    let following = journal
        .iter()
        .skip(failed_request + 1)
        .find(|e| {
            &e["headers"]["x-claude-code-session-id"] == identity && e["body"]["stream"] == true
        })
        .expect("the Door makes its next model request");
    assert_eq!(following["response"]["status"], 200);
    assert!(
        following["body"]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| { m["role"] == "user" && m["content"].to_string().contains("[decision d1]") }),
        "Door request sequence: {:#?}",
        journal
            .iter()
            .filter(|e| &e["headers"]["x-claude-code-session-id"] == identity)
            .map(
                |e| json!({"stream": e["body"]["stream"], "response": e["response"],
                "users": e["body"]["messages"].as_array().unwrap().iter()
                    .filter(|m| m["role"] == "user").collect::<Vec<_>>()})
            )
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn e6_a_late_shim_goes_missing_then_up() {
    use contracts::agent::Channel;
    use std::os::unix::fs::PermissionsExt;

    let gate = tempfile::tempdir().unwrap();
    let wrapper = gate.path().join("rup-gated");
    let release = gate.path().join("release");
    std::fs::write(
        &wrapper,
        r#"#!/usr/bin/env python3
import os, pathlib, sys, time
if sys.argv[1] == "mcp":
    deadline = time.monotonic() + 30
    while not pathlib.Path(os.environ["E6_SHIM_RELEASE"]).exists():
        if time.monotonic() >= deadline:
            sys.exit("E6 shim release deadline")
        time.sleep(0.025)
os.execv(os.environ["E6_RUP_BIN"], [os.environ["E6_RUP_BIN"], *sys.argv[1:]])
"#,
    )
    .unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let project = daemon::start_real(
        "e6_late_shim",
        &[
            ("ROUNDUP_RUP_BIN", wrapper.to_str().unwrap()),
            ("E6_RUP_BIN", env!("CARGO_BIN_EXE_rup")),
            ("E6_SHIM_RELEASE", release.to_str().unwrap()),
            ("ROUNDUP_CHANNEL_DEADLINE_MS", "100"),
        ],
    );
    let mut events = project.subscribed().await;
    let agent = events
        .request(
            "agent.spawn",
            json!({"cwd": project.dir.path(), "prompt": "E6 late shim"}),
        )
        .await
        .unwrap();
    let id = agent["id"].as_str().unwrap();
    assert_eq!(agent["channel"], "pending");
    let missing = next(&mut events, |event| match event {
        EventData::AgentChannel(channel) if channel.id == id => Some(channel.channel),
        _ => None,
    })
    .await;
    assert_eq!(missing, Channel::Missing);
    std::fs::write(release, "release real MCP").unwrap();
    let up = next(&mut events, |event| match event {
        EventData::AgentChannel(channel) if channel.id == id => Some(channel.channel),
        _ => None,
    })
    .await;
    assert_eq!(up, Channel::Up);
    let model = project.model.as_ref().unwrap();
    model.tool_result("toolu_e6_context").await;
    model.assert_matched();
}

#[tokio::test]
async fn a39_the_placement_call_is_one_slim_request() {
    // Inject dirty parent credentials into a test subprocess, without mutating this runner's
    // shared environment. The production call must strip them before its observer starts.
    if std::env::var_os("ROUNDUP_A39_OBSERVER_CHILD").is_none() {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "a39_the_placement_call_is_one_slim_request",
                "--nocapture",
            ])
            .env("ROUNDUP_A39_OBSERVER_CHILD", "1");
        for name in agents::claude_code::MARKERS
            .into_iter()
            .chain(agents::claude_code::OFF_PLAN)
        {
            command.env(name, "must-be-stripped");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    use agents::{
        claude_code::Launcher,
        placement::{Destination, Request, place},
    };
    use std::os::unix::fs::PermissionsExt;
    let model = aimock::Model::start("a39_placement");
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    aimock::configure(&config, dir.path());
    let wrapper = dir.path().join("placement");
    let log = dir.path().join("launch.json");
    let script = format!(
        r#"#!/usr/bin/env python3
import os, sys, json
with open({log:?}, 'w') as f:
    json.dump({{'argv':sys.argv[1:], 'files':os.listdir('.'), 'cwd':os.getcwd(), 'pid':os.getpid(), 'off_plan':[n for n in {off_plan:?} if n in os.environ], 'markers':[n for n in {markers:?} if n in os.environ]}}, f)
os.environ['CLAUDE_CONFIG_DIR'] = {config:?}
os.environ['ANTHROPIC_BASE_URL'] = {url:?}
os.environ['ANTHROPIC_API_KEY'] = {key:?}
os.environ['CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC'] = '1'
os.environ['DISABLE_AUTOUPDATER'] = '1'
os.environ['CLAUDE_CODE_MAX_RETRIES'] = '0'
os.execv({binary:?}, [{binary:?}, *sys.argv[1:]])
"#,
        log = log.to_str().unwrap(),
        config = config.to_str().unwrap(),
        url = model.url,
        key = aimock::KEY,
        binary = aimock::package_bin("claude").to_str().unwrap(),
        off_plan = agents::claude_code::OFF_PLAN,
        markers = agents::claude_code::MARKERS
    );
    std::fs::write(&wrapper, script).unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let launcher = Launcher::from_vars(
        |name| match name {
            "ROUNDUP_CLAUDE_BIN" => Some(wrapper.as_os_str().to_owned()),
            "HOME" => Some(dir.path().as_os_str().to_owned()),
            _ => None,
        },
        &std::env::current_exe().unwrap(),
    )
    .unwrap();
    let rail = serde_json::from_value(json!({"id":"work", "kind":"workstream", "name":"Repair", "parent":null, "order":0, "status":null, "attempt":null, "status_revision":null, "terminal_id":null, "worktree":null, "can_resume":false, "channel":null, "work":{"kind":"work", "title":"Repair café", "ask":"Repair the café", "limits":[]}, "stray":[], "model":null, "effort":null})).unwrap();
    let earlier = (0..7)
        .map(|at| agents::placement::EarlierAsk {
            at,
            parts: vec![agents::placement::AskPart {
                text: format!("earlier {at}"),
                destination: Destination::Routed {
                    workstream: "work".into(),
                },
            }],
        })
        .collect();
    let request = Request::new(
        "A39 fix café; /ship 原文; decide".into(),
        earlier,
        vec![rail],
    );
    let parts = place(&launcher.placement(), "ask-a39", &request, |id, cause| {
        panic!("{id}: {cause}")
    })
    .await;
    assert_eq!(
        parts,
        vec![
            agents::placement::AskPart {
                text: "A39 fix café".into(),
                destination: Destination::Routed {
                    workstream: "work".into()
                }
            },
            agents::placement::AskPart {
                text: "/ship 原文".into(),
                destination: Destination::New {
                    title: "Ship".into()
                }
            },
            agents::placement::AskPart {
                text: "decide".into(),
                destination: Destination::Unclear {
                    question: "Which work?".into(),
                    candidates: vec!["work".into()]
                }
            },
        ]
    );
    model.assert_matched();
    let journal = model.journal();
    assert_eq!(journal.len(), 1, "{journal:#?}");
    let body = &journal[0]["body"];
    assert!(
        body["model"].as_str().unwrap().starts_with("claude-haiku"),
        "{body}"
    );
    assert!(
        body.to_string()
            .contains("You place the user's Ask into Workstreams."),
        "{body}"
    );
    let user = body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| message["role"] == "user")
        .map(|message| message["content"].to_string())
        .collect::<String>();
    for fact in ["earlier 6", "earlier 2", "Repair the café", "/ship 原文"] {
        assert!(user.contains(fact), "missing {fact}: {body}");
    }
    for omitted in ["earlier 0", "earlier 1"] {
        assert!(!user.contains(omitted), "{body}");
    }
    let tools = body["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1, "{tools:?}");
    assert_eq!(tools[0]["function"]["name"], "StructuredOutput");
    let launch: Value = serde_json::from_slice(&std::fs::read(log).unwrap()).unwrap();
    assert!(
        !std::process::Command::new("kill")
            .args(["-0", &launch["pid"].to_string()])
            .output()
            .unwrap()
            .status
            .success(),
        "Placement process remains running or unreaped"
    );
    assert_eq!(launch["files"], json!([]));
    assert_eq!(launch["off_plan"], json!([]));
    assert_eq!(launch["markers"], json!([]));
    assert!(!std::path::Path::new(launch["cwd"].as_str().unwrap()).exists());
    let argv = launch["argv"].as_array().unwrap();
    assert_eq!(argv.len(), 18, "{argv:?}");
    let system = body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| message["role"] == "system")
        .map(|message| message["content"].to_string())
        .collect::<String>();
    assert!(system.contains(argv[17].as_str().unwrap()), "{body}");
    assert_eq!(
        &argv[..10],
        json!([
            "-p",
            "--model",
            "haiku",
            "--effort",
            "low",
            "--no-session-persistence",
            "--output-format",
            "json",
            "--json-schema",
            argv[9]
        ])
        .as_array()
        .unwrap()
    );
    assert_eq!(
        &argv[10..18],
        json!([
            "--tools",
            "",
            "--strict-mcp-config",
            "--disable-slash-commands",
            "--setting-sources",
            "",
            "--system-prompt",
            argv[17]
        ])
        .as_array()
        .unwrap()
    );
}

#[tokio::test]
async fn f3_a_door_starts_a_child_with_agent_spawn_and_the_child_is_the_doors_own() {
    let project = daemon::start_real("f3_child_home", &[]);
    let mut client = project.subscribed().await;
    let door = client
        .request(
            "rail.createWorkstream",
            json!({"name": "team", "parent": null}),
        )
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    client
        .request("rail.startDoor", json!({"id": door}))
        .await
        .unwrap();
    let model = project.model.as_ref().unwrap();
    let result = model.tool_result("toolu_f3_spawn").await;
    let spawned: contracts::agent::RailNode =
        serde_json::from_str(result["content"].as_str().unwrap()).unwrap();
    assert_eq!(spawned.parent.as_deref(), Some(door.as_str()));
    assert!(spawned.worktree.is_none());
    let tree = rail_tree(&client).await;
    let child = tree.iter().find(|n| n.id == spawned.id).unwrap();
    assert_eq!(child.parent.as_deref(), Some(door.as_str()));
    assert!(tree.iter().all(|n| n.parent.as_deref() != Some("999")));
    for kind in [Kind::Working, Kind::Idle] {
        next(&mut client, |event| match event {
            EventData::AgentStatus(status)
                if status.id == spawned.id && status.status.kind == kind =>
            {
                Some(())
            }
            _ => None,
        })
        .await;
    }
    model.assert_matched();
}

/// The names of the tools in a request's `tools` array, and the request's text size in characters.
fn tool_names_and_size(request: &Value) -> (Vec<String>, usize) {
    let mut names: Vec<String> = request["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|tool| tool["function"]["name"].as_str().map(str::to_owned))
        .collect();
    names.sort();
    (names, request.to_string().len())
}

#[tokio::test]
async fn lc9_the_profile_holds_in_the_real_tui() {
    let project = daemon::start_real("f3_child_home", &[]);
    let mut client = project.subscribed().await;
    let door = client
        .request(
            "rail.createWorkstream",
            json!({"name": "team", "parent": null}),
        )
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    client
        .request("rail.startDoor", json!({"id": door}))
        .await
        .unwrap();
    let model = project.model.as_ref().unwrap();
    let spawned: contracts::agent::RailNode = serde_json::from_str(
        model.tool_result("toolu_f3_spawn").await["content"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    for kind in [Kind::Working, Kind::Idle] {
        next(&mut client, |event| match event {
            EventData::AgentStatus(status)
                if status.id == spawned.id && status.status.kind == kind =>
            {
                Some(())
            }
            _ => None,
        })
        .await;
    }
    let journal = model.journal();
    let first_with = |needle: &str, absent: bool| {
        journal
            .iter()
            .map(|entry| &entry["body"])
            .filter(|body| body["tools"].is_array())
            .find(|body| body.to_string().contains(needle) != absent)
            .map(tool_names_and_size)
            .unwrap()
    };
    let (door_tools, door_size) = first_with("coordinating Door", false);
    let (worker_tools, worker_size) = first_with("coordinating Door", true);
    // Recorded for the PR body: the first request of each, in characters (about four to a token).
    eprintln!(
        "lc9 door {door_size} chars {door_tools:?}\nlc9 worker {worker_size} chars {worker_tools:?}"
    );
    for name in &door_tools {
        assert!(
            [
                "DeferredToolPlaceholder",
                "Glob",
                "Grep",
                "Read",
                "ToolSearch"
            ]
            .contains(&name.as_str()),
            "a Door lists only its row's tools, and roundup's load on demand: {name}"
        );
    }
    assert!(door_tools.iter().any(|name| name == "Read"));
    assert!(worker_tools.iter().any(|name| name == "Skill"));
    assert!(worker_tools.iter().any(|name| name == "Bash"));
    assert!(worker_size > door_size, "a worker carries more than a Door");
}

#[tokio::test]
async fn m16_real_claude_runs_tests_with_bash_without_a_run() {
    let project = daemon::start_real("m16_door_child_bash", &[]);
    std::fs::write(project.dir.path().join("test.sh"), "exit 0\n").unwrap();
    let client = project.subscribed().await;
    let door = client
        .request(
            "rail.createWorkstream",
            json!({"name": "team", "parent": null}),
        )
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    client
        .request("rail.startDoor", json!({"id": door}))
        .await
        .unwrap();
    let model = project.model.as_ref().unwrap();
    let spawned: contracts::agent::RailNode = serde_json::from_str(
        model.tool_result("toolu_m16_spawn").await["content"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let owned = |terminals: &Value, owner: &str| {
        terminals
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry["owner"] == owner)
            .count()
    };
    let before = owned(
        &client.request("terminal.list", Value::Null).await.unwrap(),
        &spawned.id,
    );
    let decision = daemon::h21_decision(&client, &spawned.id, None).await;
    assert_eq!(decision["tool"], "Bash");
    client
        .request(
            "decision.answer",
            json!({"id": decision["id"], "answer": "allow", "proof": daemon::H21_PROOF}),
        )
        .await
        .unwrap();
    let result = model.tool_result("toolu_m16_bash").await;
    assert_eq!(result["is_error"], json!(null), "{result}");
    let after = owned(
        &client.request("terminal.list", Value::Null).await.unwrap(),
        &spawned.id,
    );
    assert_eq!(after, before, "the child's Bash started no Terminal");
    model.assert_matched();
}

#[tokio::test]
async fn f3_a_door_spawns_into_a_worktree_when_the_setting_is_on() {
    let project = daemon::start_real("f3_child_worktree", &[]);
    let path = project.dir.path();
    for args in [
        vec!["init", "-q", "-b", "main"],
        vec!["config", "user.name", "roundup"],
        vec!["config", "user.email", "roundup@example.com"],
    ] {
        assert!(
            std::process::Command::new("git")
                .current_dir(path)
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    }
    std::fs::write(path.join("README"), "keep").unwrap();
    for args in [vec!["add", "README"], vec!["commit", "-qm", "base"]] {
        assert!(
            std::process::Command::new("git")
                .current_dir(path)
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    }
    let mut client = project.subscribed().await;
    client
        .request("project.setWorktrees", json!({"on": true, "check": null}))
        .await
        .unwrap();
    let door = client
        .request(
            "rail.createWorkstream",
            json!({"name": "team", "parent": null}),
        )
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    client
        .request("rail.startDoor", json!({"id": door}))
        .await
        .unwrap();
    let model = project.model.as_ref().unwrap();
    let result = model.tool_result("toolu_f3_spawn").await;
    let spawned: contracts::agent::RailNode =
        serde_json::from_str(result["content"].as_str().unwrap()).unwrap();
    assert_eq!(spawned.parent.as_deref(), Some(door.as_str()));
    let worktree = spawned.worktree.expect("the child has a Worktree");
    assert_eq!(worktree.branch, format!("roundup/agent-{}", spawned.id));
    let listed = std::process::Command::new("git")
        .current_dir(path)
        .args(["worktree", "list"])
        .output()
        .unwrap();
    assert!(listed.status.success());
    let listed = String::from_utf8(listed.stdout).unwrap();
    assert!(listed.contains(&worktree.path), "{listed}");
    for kind in [Kind::Working, Kind::Idle] {
        next(&mut client, |event| match event {
            EventData::AgentStatus(status)
                if status.id == spawned.id && status.status.kind == kind =>
            {
                Some(())
            }
            _ => None,
        })
        .await;
    }
    model.assert_matched();
}

#[tokio::test]
async fn a26_a_subagent_started_by_a_subagent_sits_three_deep_under_the_door_through_rup_mcp() {
    let project = daemon::start_real("a26_three_deep", &[]);
    let mut client = project.subscribed().await;
    let door = client
        .request(
            "rail.createWorkstream",
            json!({"name": "team", "parent": null}),
        )
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    client
        .request("rail.startDoor", json!({"id": door}))
        .await
        .unwrap();
    let model = project.model.as_ref().unwrap();
    let first = model.tool_result("toolu_a26_child").await;
    let a: contracts::agent::RailNode =
        serde_json::from_str(first["content"].as_str().unwrap()).unwrap();
    let second = model.tool_result("toolu_a26_grandchild").await;
    let b: contracts::agent::RailNode =
        serde_json::from_str(second["content"].as_str().unwrap()).unwrap();
    assert_eq!(a.parent.as_deref(), Some(door.as_str()));
    assert_eq!(b.parent.as_deref(), Some(a.id.as_str()));
    let tree = rail_tree(&client).await;
    let ids: Vec<_> = tree.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, [door.as_str(), a.id.as_str(), b.id.as_str()]);
    // Wait through all three real turns so late unmatched requests cannot escape the observer.
    let mut working = std::collections::BTreeSet::new();
    let mut idle = std::collections::BTreeSet::new();
    while idle.len() < 3 {
        let status = next(&mut client, |event| match event {
            EventData::AgentStatus(status) if [&door, &a.id, &b.id].contains(&&status.id) => {
                Some(status)
            }
            _ => None,
        })
        .await;
        if status.status.kind == Kind::Working {
            working.insert(status.id.clone());
        }
        if status.status.kind == Kind::Idle && working.contains(&status.id) {
            idle.insert(status.id);
        }
    }
    model.assert_matched();
}

#[tokio::test]
async fn e6_no_shim_is_missing_with_the_status_unchanged_and_hooks_do_not_count() {
    use contracts::agent::Channel;
    use std::os::unix::fs::PermissionsExt;

    let probe = tempfile::tempdir().unwrap();
    let wrapper = probe.path().join("rup-without-mcp");
    let log = probe.path().join("hooks.jsonl");
    std::fs::write(
        &wrapper,
        r#"#!/usr/bin/env python3
import json, os, sys
if sys.argv[1] == "mcp":
    sys.exit(1)
if sys.argv[1] == "signal":
    payload = sys.stdin.read()
    with open(os.environ["E6_HOOK_LOG"], "a") as log:
        log.write(json.dumps(json.loads(payload)) + "\n")
    import subprocess
    sys.exit(subprocess.run([os.environ["E6_RUP_BIN"], *sys.argv[1:]], input=payload, text=True).returncode)
os.execv(os.environ["E6_RUP_BIN"], [os.environ["E6_RUP_BIN"], *sys.argv[1:]])
"#,
    )
    .unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let project = daemon::start_real(
        "e6_no_shim",
        &[
            ("ROUNDUP_RUP_BIN", wrapper.to_str().unwrap()),
            ("E6_RUP_BIN", env!("CARGO_BIN_EXE_rup")),
            ("E6_HOOK_LOG", log.to_str().unwrap()),
            ("ROUNDUP_CHANNEL_DEADLINE_MS", "100"),
        ],
    );
    let mut events = project.subscribed().await;
    let agent = events
        .request(
            "agent.spawn",
            json!({"cwd": project.dir.path(), "prompt": "E6 no shim"}),
        )
        .await
        .unwrap();
    let id = agent["id"].as_str().unwrap();
    assert_eq!(agent["channel"], "pending");
    let mut initial = None;
    let mut working = false;
    let mut stopped = false;
    let mut missing = false;
    while !stopped || !missing {
        let event = next(&mut events, |event| match event {
            EventData::AgentStatus(status) if status.id == id => {
                Some(EventData::AgentStatus(status))
            }
            EventData::AgentChannel(channel) if channel.id == id => {
                Some(EventData::AgentChannel(channel))
            }
            _ => None,
        })
        .await;
        match event {
            EventData::AgentStatus(status) => {
                initial.get_or_insert(status.status.kind);
                working |= status.status.kind == Kind::Working;
                stopped |= working && status.status.kind == Kind::Idle;
            }
            EventData::AgentChannel(channel) => {
                assert_eq!(channel.channel, Channel::Missing);
                missing = true;
            }
            _ => unreachable!(),
        }
    }
    let node = rail_tree(&events)
        .await
        .into_iter()
        .find(|node| node.id == id)
        .unwrap();
    assert_eq!(node.channel, Some(Channel::Missing));
    assert_eq!(node.status.unwrap().kind, initial.unwrap());
    let hooks: Vec<Value> = std::fs::read_to_string(log)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    for name in ["SessionStart", "UserPromptSubmit", "Stop"] {
        assert!(
            hooks.iter().any(|hook| hook["hook_event_name"] == name),
            "{hooks:#?}"
        );
    }
    let model = project.model.as_ref().unwrap();
    model.assert_matched();
    assert!(
        model
            .journal()
            .iter()
            .any(|entry| entry["body"]["tools"].is_array()
                && entry["body"]["messages"].to_string().contains("E6 no shim"))
    );
}

#[tokio::test]
async fn a41_a_leading_slash_runs_the_skill() {
    let project = daemon::start_real("a41_skill", &[]);
    let skill = project.dir.path().join(".claude/skills/a41-proof");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(skill.join("SKILL.md"), "---\nname: a41-proof\ndescription: Prove Ask delivery preserves skill invocation.\n---\nA41_SKILL_MARKER. Reply with Skill received.\n").unwrap();
    let mut client = project.client().await;
    client
        .request("events.subscribe", Value::Null)
        .await
        .unwrap();
    let node = client.request("rail.createWorkstream", json!({"name":"Skill","order":{"kind":"work","title":"Skill","ask":"A41 initial","limits":[]}})).await.unwrap();
    let id = node["id"].as_str().unwrap();
    client
        .request("rail.startDoor", json!({"id":id}))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(15), next(&mut client, |event| matches!(event, EventData::AgentStatus(status) if status.id == id && status.status.kind == Kind::Idle).then_some(()))).await.unwrap_or_else(|_| panic!("{:?}",project.model.as_ref().unwrap().journal()));
    let ask = client
        .request("ask.send", json!({"text":"@Skill /a41-proof x"}))
        .await
        .unwrap();
    next(&mut client, |event| matches!(event, EventData::AskChanged { ask: changed } if changed.id == ask["id"].as_str().unwrap() && changed.parts[0].state == contracts::ask::State::Typed).then_some(())).await;
    tokio::time::timeout(Duration::from_secs(15), next(&mut client, |event| matches!(event, EventData::AgentStatus(status) if status.id == id && status.status.kind == Kind::Idle).then_some(()))).await.unwrap_or_else(|_| panic!("{:?}",project.model.as_ref().unwrap().journal()));
    let journal = project.model.as_ref().unwrap().journal();
    let text = serde_json::to_string(&journal).unwrap();
    assert!(text.contains("A41_SKILL_MARKER"), "{text}");
    assert!(
        journal.iter().all(|entry| entry["response"]["status"]
            .as_i64()
            .is_some_and(|status| status < 400)),
        "{journal:#?}"
    );
    assert!(
        journal.iter().all(|entry| !entry["body"]["model"]
            .as_str()
            .is_some_and(|model| model.starts_with("claude-haiku"))),
        "named routing made a Placement call"
    );
}

#[tokio::test]
async fn h15_every_hook_call_of_a_hundred_tool_use_loop_is_answered_and_the_final_stop_leaves_the_agent_idle()
 {
    use std::os::unix::fs::PermissionsExt;
    let probe = tempfile::tempdir().unwrap();
    let wrapper = probe.path().join("rup-hook-observer");
    let log = probe.path().join("hooks.jsonl");
    std::fs::write(&wrapper, r##"#!/usr/bin/env python3
import json, os, subprocess, sys
if sys.argv[1] != "signal":
    os.execv(os.environ["REAL_CLAUDE_RUP_BIN"], [os.environ["REAL_CLAUDE_RUP_BIN"], *sys.argv[1:]])
payload = sys.stdin.buffer.read()
result = subprocess.run([os.environ["REAL_CLAUDE_RUP_BIN"], *sys.argv[1:]], input=payload, stdout=subprocess.PIPE, timeout=5)
with open(os.environ["REAL_CLAUDE_HOOK_LOG"], "a") as log:
    log.write(json.dumps({"event": json.loads(payload)["hook_event_name"], "exit": result.returncode}) + "\n")
sys.stdout.buffer.write(result.stdout)
sys.exit(result.returncode)
"##).unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let project = daemon::start_real(
        "h15_hundred_tools",
        &[
            ("ROUNDUP_RUP_BIN", wrapper.to_str().unwrap()),
            ("REAL_CLAUDE_RUP_BIN", env!("CARGO_BIN_EXE_rup")),
            ("REAL_CLAUDE_HOOK_LOG", log.to_str().unwrap()),
        ],
    );
    for index in 0..100 {
        std::fs::write(
            project.dir.path().join(format!("h15-{index:03}.txt")),
            "H15 hook round\n",
        )
        .unwrap();
    }
    let mut client = project.subscribed().await;
    let agent: contracts::agent::RailNode = serde_json::from_value(
        tokio::time::timeout(
            Duration::from_secs(30),
            client.request(
                "agent.spawn",
                json!({"cwd": project.dir.path(), "prompt": "H15 exercise one hundred tools"}),
            ),
        )
        .await
        .expect("agent.spawn answers within the bound")
        .unwrap(),
    )
    .unwrap();
    tokio::time::timeout(Duration::from_secs(180), async {
        let mut working = false;
        loop {
            let event = client
                .next_event()
                .await
                .expect("the connection stays open");
            if let EventData::AgentStatus(status) = event.data
                && status.id == agent.id
            {
                if status.status.kind == Kind::Working {
                    working = true;
                } else if status.status.kind == Kind::Idle && working {
                    break;
                }
            }
        }
    })
    .await
    .expect("the hundred-tool turn completes");
    let model = project.model.as_ref().unwrap();
    model.assert_matched();
    let journal = model.journal();
    // Aimock truncates long request bodies at 64 KB. Its selected fixture records the
    // last-result ID and content constraints that actually matched even after that cap.
    for index in 0..100 {
        let id = format!("toolu_h15_{index:03}");
        assert!(
            journal.iter().any(|entry| {
                let matched = &entry["response"]["fixture"]["match"];
                matched["toolCallId"] == id && matched["toolResultContains"] == "H15 hook round"
            }),
            "missing successful tool result {id}"
        );
    }
    let hooks: Vec<Value> = std::fs::read_to_string(log)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(hooks.iter().all(|hook| hook["exit"] == 0), "{hooks:#?}");
    let uses: Vec<_> = hooks
        .iter()
        .filter(|hook| matches!(hook["event"].as_str(), Some("PreToolUse" | "PostToolUse")))
        .collect();
    assert_eq!(uses.len(), 200, "{hooks:#?}");
    assert!(
        uses.chunks(2).all(|pair| pair[0]["event"] == "PreToolUse"
            && pair[1]["event"] == "PostToolUse"
            && pair.iter().all(|hook| hook["exit"] == 0)),
        "{hooks:#?}"
    );
    let tree = rail_tree(&client).await;
    assert_eq!(
        tree.iter()
            .find(|node| node.id == agent.id)
            .unwrap()
            .status
            .as_ref()
            .map(|status| status.label.as_str()),
        Some("idle")
    );
}

#[tokio::test]
async fn a32_an_empty_reason_changes_nothing() {
    let project = daemon::start_real("a32_invalid_clear", &[]);
    let path = project.dir.path();
    for args in [
        vec!["init", "-q", "-b", "main"],
        vec!["config", "user.name", "roundup"],
        vec!["config", "user.email", "roundup@example.com"],
    ] {
        assert!(
            std::process::Command::new("git")
                .current_dir(path)
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    }
    std::fs::write(path.join("README"), "keep").unwrap();
    for args in [vec!["add", "README"], vec!["commit", "-qm", "base"]] {
        assert!(
            std::process::Command::new("git")
                .current_dir(path)
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    }
    let mut client = project.subscribed().await;
    client
        .request("project.setWorktrees", json!({"on": true, "check": null}))
        .await
        .unwrap();
    let door = client
        .request(
            "rail.createWorkstream",
            json!({"name": "team", "parent": null}),
        )
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    client
        .request("rail.startDoor", json!({"id": door}))
        .await
        .unwrap();
    let model = project.model.as_ref().unwrap();
    let first = model.tool_result("toolu_a32_child").await;
    let a: contracts::agent::RailNode =
        serde_json::from_str(first["content"].as_str().unwrap()).unwrap();
    let second = model.tool_result("toolu_a32_grandchild").await;
    let b: contracts::agent::RailNode =
        serde_json::from_str(second["content"].as_str().unwrap()).unwrap();
    // Wait through all three real turns so late unmatched requests cannot escape the observer.
    let mut working = std::collections::BTreeSet::new();
    let mut idle = std::collections::BTreeSet::new();
    while idle.len() < 3 {
        let status = next(&mut client, |event| match event {
            EventData::AgentStatus(status) if [&door, &a.id, &b.id].contains(&&status.id) => {
                Some(status)
            }
            _ => None,
        })
        .await;
        if status.status.kind == Kind::Working {
            working.insert(status.id.clone());
        }
        if status.status.kind == Kind::Idle && working.contains(&status.id) {
            idle.insert(status.id);
        }
    }
    let before: Vec<_> = rail_tree(&client)
        .await
        .into_iter()
        .map(|n| (n.id, n.parent, n.iteration, n.worktree))
        .collect();
    let asked = model.journal().len();
    for reason in ["", "  \n ", &"x".repeat(2001)] {
        let error = client
            .request("workstream.clear", json!({"id": door, "reason": reason}))
            .await
            .unwrap_err();
        assert_eq!(error.code, rpc::code::INVALID_PARAMS);
    }
    let missing = client
        .request("workstream.clear", json!({"id": "9999", "reason": "r"}))
        .await
        .unwrap_err();
    assert_eq!(missing.code, rpc::code::NOT_FOUND);
    let sub = client
        .request("workstream.clear", json!({"id": a.id, "reason": "r"}))
        .await
        .unwrap_err();
    assert_eq!(sub.code, rpc::code::INVALID_PARAMS);
    let after: Vec<_> = rail_tree(&client)
        .await
        .into_iter()
        .map(|n| (n.id, n.parent, n.iteration, n.worktree))
        .collect();
    assert_eq!(after, before);
    assert_eq!(
        model.journal().len(),
        asked,
        "invalid clear sends no prompt to Claude"
    );
    model.assert_matched();
}

#[tokio::test]
async fn a32_only_the_door_may_hand_off() {
    let project = daemon::start_real("a32_handoff_authority", &[]);
    let mut client = project.subscribed().await;
    let door = client
        .request(
            "rail.createWorkstream",
            json!({"name": "team", "parent": null}),
        )
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    client
        .request("rail.startDoor", json!({"id": door}))
        .await
        .unwrap();
    let model = project.model.as_ref().unwrap();
    let mut working = false;
    loop {
        let status = next(&mut client, |event| match event {
            EventData::AgentStatus(status) if status.id == door => Some(status),
            _ => None,
        })
        .await;
        working |= status.status.kind == Kind::Working;
        if working && status.status.kind == Kind::Idle {
            break;
        }
    }
    let actor = |id: String| async {
        let who = project.client().await;
        who.request(
            "daemon.identify",
            contracts::IdentifyParams {
                actor: contracts::Actor {
                    kind: contracts::ActorKind::Agent,
                    id,
                    parent: None,
                },
            },
        )
        .await
        .unwrap();
        who
    };
    let owner = actor(door.clone()).await;
    let params = json!({"id": door, "summary": "s", "keep_todos": [], "keep_pads": []});
    let error = owner
        .request("workstream.handoff", params.clone())
        .await
        .unwrap_err();
    assert_eq!(error.code, rpc::code::CONFLICT);
    client
        .request(
            "workstream.clear",
            json!({"id": door, "reason": "authority check"}),
        )
        .await
        .unwrap();
    // Digest turns also emit Working/Idle, so only the request proves Clear arrived.
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            model.assert_matched();
            if model.journal().iter().any(|entry| {
                entry["body"]["messages"]
                    .as_array()
                    .is_some_and(|messages| {
                        messages.iter().any(|message| {
                            message["role"] == "user"
                                && message["content"].as_str().is_some_and(|text| {
                                    text.contains("authority check")
                                        && text.contains("workstream_handoff")
                                })
                        })
                    })
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "clear prompt missing from aimock journal: {:#?}",
            model.journal()
        )
    });
    // Spawn the child during Clear, so its startup digests cannot race Clear's Steer.
    let result = model.tool_result("toolu_a32_child").await;
    let child: contracts::agent::RailNode =
        serde_json::from_str(result["content"].as_str().unwrap()).unwrap();
    for who in [actor(child.id).await, project.client().await] {
        let error = who
            .request("workstream.handoff", params.clone())
            .await
            .unwrap_err();
        assert_eq!(error.code, rpc::code::FORBIDDEN);
    }
    model.assert_matched();
}

async fn clear_fixture(
    fixture: &str,
    env: &[(&str, &str)],
) -> (daemon::Project, rpc::Client, String) {
    // The new Door waits for the real `claude` to start; a loaded host takes longer than the Daemon's
    // 8 s default, so a Clear is not failed for it unless the caller sets the bound.
    let env: Vec<_> = std::iter::once(("ROUNDUP_RESTART_BOUND_MS", "60000"))
        .chain(env.iter().copied())
        .collect();
    let project = daemon::start_real(fixture, &env);
    let path = project.dir.path();
    for args in [
        vec!["init", "-q", "-b", "main"],
        vec!["config", "user.name", "roundup"],
        vec!["config", "user.email", "roundup@example.com"],
    ] {
        assert!(
            std::process::Command::new("git")
                .current_dir(path)
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    }
    std::fs::write(path.join("README"), "keep").unwrap();
    for args in [vec!["add", "README"], vec!["commit", "-qm", "base"]] {
        assert!(
            std::process::Command::new("git")
                .current_dir(path)
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    }
    let mut client = project.subscribed().await;
    client
        .request("project.setWorktrees", json!({"on": true, "check": null}))
        .await
        .unwrap();
    let door = client
        .request(
            "rail.createWorkstream",
            json!({"name": "team", "parent": null}),
        )
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    client
        .request("rail.startDoor", json!({"id": door}))
        .await
        .unwrap();
    // Hold child Digests so Clear cannot race a startup or Todo/Pad update turn.
    // Clear steers the Door directly; Takeover only holds Message delivery here.
    client
        .request("takeover.begin", json!({"agent": door}))
        .await
        .unwrap();
    let model = project.model.as_ref().unwrap();
    let first = model.tool_result("toolu_a32_child").await;
    let a: contracts::agent::RailNode =
        serde_json::from_str(first["content"].as_str().unwrap()).unwrap();
    let second = model.tool_result("toolu_a32_grandchild").await;
    let b: contracts::agent::RailNode =
        serde_json::from_str(second["content"].as_str().unwrap()).unwrap();
    // Wait through all three real turns so late unmatched requests cannot escape the observer.
    let mut working = std::collections::BTreeSet::new();
    let mut idle = std::collections::BTreeSet::new();
    while idle.len() < 3 {
        let status = next(&mut client, |event| match event {
            EventData::AgentStatus(status) if [&door, &a.id, &b.id].contains(&&status.id) => {
                Some(status)
            }
            _ => None,
        })
        .await;
        if status.status.kind == Kind::Working {
            working.insert(status.id.clone());
        }
        if status.status.kind == Kind::Idle && working.contains(&status.id) {
            idle.insert(status.id);
        }
    }

    for (owner, title, name) in [
        (&door, "build the login page", "notes"),
        (&a.id, "refactor everything", "scratch"),
    ] {
        let actor = project.client().await;
        actor
            .request(
                "daemon.identify",
                contracts::IdentifyParams {
                    actor: contracts::Actor {
                        kind: contracts::ActorKind::Agent,
                        id: owner.clone(),
                        parent: None,
                    },
                },
            )
            .await
            .unwrap();
        actor
            .request(
                "todo.create",
                json!({"title": title, "body": format!("{title} body")}),
            )
            .await
            .unwrap();
        actor
            .request(
                "pad.create",
                json!({"name": name, "text": format!("{name} text")}),
            )
            .await
            .unwrap();
    }
    (project, client, door)
}

#[tokio::test]
async fn a32_no_handoff_times_out_and_changes_nothing() {
    let (project, mut client, door) =
        clear_fixture("a32_no_handoff", &[("ROUNDUP_HANDOFF_BOUND_MS", "10000")]).await;
    let model = project.model.as_ref().unwrap();
    let before = rail_tree(&client).await;
    let todos = client.request("todo.list", Value::Null).await.unwrap();
    let pads = client.request("pad.list", Value::Null).await.unwrap();
    let reply = client
        .request(
            "workstream.clear",
            json!({"id": door, "reason": "no handoff observer"}),
        )
        .await
        .unwrap();
    assert_eq!(reply, json!({"pending": true}));
    let failed = next(&mut client, |event| match event {
        EventData::WorkstreamClearFailed(failed) if failed.id == door => Some(failed),
        _ => None,
    })
    .await;
    assert_eq!(failed.code, rpc::code::NOT_ACKED as i32, "{failed:?}");
    let after = rail_tree(&client).await;
    let shape = |nodes: Vec<contracts::agent::RailNode>| {
        nodes
            .into_iter()
            .map(|node| (node.id, node.parent, node.iteration, node.worktree))
            .collect::<Vec<_>>()
    };
    assert_eq!(shape(after), shape(before));
    assert_eq!(
        client.request("todo.list", Value::Null).await.unwrap(),
        todos
    );
    assert_eq!(client.request("pad.list", Value::Null).await.unwrap(), pads);
    assert!(
        model
            .journal()
            .iter()
            .any(|entry| entry["body"]["messages"]
                .as_array()
                .is_some_and(
                    |messages| messages.iter().any(|message| message["role"] == "user"
                        && message["content"]
                            .as_str()
                            .is_some_and(|text| text.contains("no handoff observer")
                                && text.contains("workstream_handoff")))
                )),
        "Clear must reach real Claude: {:#?}",
        model.journal()
    );
    client
        .request("agent.stop", json!({"id": door}))
        .await
        .unwrap();
    let stopped = client
        .request("workstream.clear", json!({"id": door, "reason": "r"}))
        .await
        .unwrap_err();
    assert_eq!(stopped.code, rpc::code::CONFLICT);
    model.assert_matched();
}

#[tokio::test]
async fn a32_a_valid_clear_only_asks_the_door() {
    let (project, mut client, door) = clear_fixture("a32_valid_clear", &[]).await;
    let model = project.model.as_ref().unwrap();
    let before = rail_tree(&client).await;
    let todos = client.request("todo.list", Value::Null).await.unwrap();
    let pads = client.request("pad.list", Value::Null).await.unwrap();
    client
        .request(
            "message.send",
            json!({"to": door, "kind": "note", "body": "I want a login page"}),
        )
        .await
        .unwrap();
    let reply = client
        .request(
            "workstream.clear",
            json!({"id": door, "reason": "too much fluff"}),
        )
        .await
        .unwrap();
    assert_eq!(reply, json!({"pending": true}));
    let ask = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            model.assert_matched();
            for entry in model.journal() {
                if let Some(messages) = entry["body"]["messages"].as_array() {
                    for message in messages {
                        if message["role"] == "user"
                            && let Some(text) = message["content"].as_str()
                            && text.contains("too much fluff")
                            && text.contains("workstream_handoff")
                        {
                            return text.to_owned();
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("Clear missing from aimock journal");
    assert!(ask.contains("1. I want a login page"), "{ask}");
    for todo in todos.as_array().unwrap() {
        assert!(
            ask.contains(&format!(
                "{} \"{}\"",
                todo["id"],
                todo["title"].as_str().unwrap()
            )),
            "{ask}"
        );
    }
    assert!(ask.contains("scratch (owner "), "{ask}");
    let shape = |nodes: Vec<contracts::agent::RailNode>| {
        nodes
            .into_iter()
            .map(|node| (node.id, node.parent, node.iteration, node.worktree))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        shape(rail_tree(&client).await),
        shape(before),
        "nothing leaves the Rail before Handoff"
    );
    assert_eq!(
        client.request("todo.list", Value::Null).await.unwrap(),
        todos
    );
    assert_eq!(client.request("pad.list", Value::Null).await.unwrap(), pads);
    let second = client
        .request("workstream.clear", json!({"id": door, "reason": "again"}))
        .await
        .unwrap_err();
    assert_eq!(second.code, rpc::code::CONFLICT);
    // Finish the real Clear turn before checking the complete request journal.
    let mut working = false;
    loop {
        let status = next(&mut client, |event| match event {
            EventData::AgentStatus(status) if status.id == door => Some(status),
            _ => None,
        })
        .await;
        working |= status.status.kind == Kind::Working;
        if working && status.status.kind == Kind::Idle {
            break;
        }
    }
    model.assert_matched();
}

#[tokio::test]
async fn a32_a_bad_id_in_the_handoff_keeps_the_clear_pending() {
    let (project, mut client, door) = clear_fixture("a32_bad_handoff", &[]).await;
    let model = project.model.as_ref().unwrap();
    let todos = client.request("todo.list", Value::Null).await.unwrap();
    let keep = todos[0]["id"].clone();
    assert_eq!(
        client
            .request(
                "workstream.clear",
                json!({"id": door, "reason": "bad handoff observer"})
            )
            .await
            .unwrap(),
        json!({"pending": true})
    );
    let mut working = false;
    loop {
        let status = next(&mut client, |event| match event {
            EventData::AgentStatus(status) if status.id == door => Some(status),
            _ => None,
        })
        .await;
        working |= status.status.kind == Kind::Working;
        if working && status.status.kind == Kind::Idle {
            break;
        }
    }
    assert!(
        model.journal().iter().any(|entry| entry["body"]["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty())
            && entry["body"]["messages"]
                .as_array()
                .is_some_and(
                    |messages| messages.iter().any(|message| message["role"] == "user"
                        && message["content"]
                            .as_str()
                            .is_some_and(|text| text.contains("bad handoff observer")
                                && text.contains("workstream_handoff")))
                )),
        "Clear must reach real Claude"
    );
    let actor = project.client().await;
    actor
        .request(
            "daemon.identify",
            contracts::IdentifyParams {
                actor: contracts::Actor {
                    kind: contracts::ActorKind::Agent,
                    id: door.clone(),
                    parent: None,
                },
            },
        )
        .await
        .unwrap();
    for bad in [
        json!({"keep_todos": [999], "keep_pads": []}),
        json!({"keep_todos": [], "keep_pads": ["nope"]}),
        json!({"keep_todos": [], "keep_pads": [], "summary": " "}),
    ] {
        let mut params = json!({"id": door, "summary": "s"});
        params
            .as_object_mut()
            .unwrap()
            .extend(bad.as_object().unwrap().clone());
        let err = actor
            .request("workstream.handoff", params)
            .await
            .unwrap_err();
        assert_eq!(err.code, rpc::code::INVALID_PARAMS);
    }
    assert_eq!(
        client.request("todo.list", Value::Null).await.unwrap(),
        todos,
        "a bad Handoff changes nothing"
    );
    actor.request("workstream.handoff", json!({"id": door, "summary": "Valid handoff after bad ids", "keep_todos": [keep], "keep_pads": []})).await.unwrap();
    let cleared = next(&mut client, |event| match event {
        EventData::WorkstreamCleared(cleared) if cleared.id == door => Some(cleared),
        _ => None,
    })
    .await;
    assert_eq!(cleared.iteration, 2);
    let mut working = false;
    loop {
        let status = next(&mut client, |event| match event {
            EventData::AgentStatus(status) if status.id == door && status.attempt == "2" => {
                Some(status)
            }
            _ => None,
        })
        .await;
        working |= status.status.kind == Kind::Working;
        if working && status.status.kind == Kind::Idle {
            break;
        }
    }
    assert!(
        model.journal().iter().any(|entry| entry["body"]["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty())
            && entry["body"]["messages"]
                .as_array()
                .is_some_and(
                    |messages| messages.iter().any(|message| message["role"] == "user"
                        && message["content"].as_str().is_some_and(|text| text
                            .contains("Follow this instruction: Valid handoff after bad ids")))
                )),
        "the real Door must receive the valid Handoff: {:#?}",
        model.journal()
    );
    model.assert_matched();
}

#[tokio::test]
async fn a32_no_slash_command_is_typed() {
    let (project, mut client, door) = clear_fixture("a32_no_slash", &[]).await;
    let model = project.model.as_ref().unwrap();
    assert_eq!(
        client
            .request(
                "workstream.clear",
                json!({"id": door, "reason": "/clear please"})
            )
            .await
            .unwrap(),
        json!({"pending": true})
    );
    received(model, "/clear please").await;
    assert!(
        model.journal().iter().any(|entry| entry["body"]["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty())
            && entry["body"]["messages"]
                .as_array()
                .is_some_and(
                    |messages| messages.iter().any(|message| message["role"] == "user"
                        && message["content"]
                            .as_str()
                            .is_some_and(|text| text.contains("/clear please")
                                && text.contains("workstream_handoff")))
                )),
        "Clear must reach real Claude"
    );
    let actor = project.client().await;
    actor
        .request(
            "daemon.identify",
            contracts::IdentifyParams {
                actor: contracts::Actor {
                    kind: contracts::ActorKind::Agent,
                    id: door.clone(),
                    parent: None,
                },
            },
        )
        .await
        .unwrap();
    actor
        .request(
            "workstream.handoff",
            json!({"id": door, "summary": "Slash-free handoff", "keep_todos": [], "keep_pads": []}),
        )
        .await
        .unwrap();
    let cleared = next(&mut client, |event| match event {
        EventData::WorkstreamCleared(cleared) if cleared.id == door => Some(cleared),
        _ => None,
    })
    .await;
    assert_eq!(cleared.iteration, 2);
    turn_done(model, &client, &door, "Slash-free handoff").await;
    assert!(
        model.journal().iter().any(|entry| entry["body"]["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty())
            && entry["body"]["messages"]
                .as_array()
                .is_some_and(
                    |messages| messages.iter().any(|message| message["role"] == "user"
                        && message["content"].as_str().is_some_and(
                            |text| text.contains("Follow this instruction: Slash-free handoff")
                        ))
                )),
        "the real Door must receive the valid Handoff: {:#?}",
        model.journal()
    );
    for entry in model.journal() {
        for message in entry["body"]["messages"].as_array().unwrap() {
            if message["role"] == "user"
                && let Some(text) = message["content"].as_str()
            {
                assert!(!text.starts_with('/'), "slash command typed: {text}");
            }
        }
    }
    model.assert_matched();
}

#[tokio::test]
async fn a32_the_workstream_and_its_terminals_survive() {
    let (project, mut client, door) = clear_fixture("a32_terminals", &[]).await;
    let model = project.model.as_ref().unwrap();
    let opened = client
        .request("terminal.open", json!({"workstream": door}))
        .await
        .unwrap();
    let terminal = opened["terminal_id"]
        .as_str()
        .expect("the opened Terminal")
        .to_owned();
    let before = rail_tree(&client)
        .await
        .into_iter()
        .find(|node| node.id == door)
        .unwrap();
    assert_eq!(
        client
            .request(
                "workstream.clear",
                json!({"id": door, "reason": "preserve the terminals"})
            )
            .await
            .unwrap(),
        json!({"pending": true})
    );
    let mut working = false;
    loop {
        let status = next(&mut client, |event| match event {
            EventData::AgentStatus(status) if status.id == door => Some(status),
            _ => None,
        })
        .await;
        working |= status.status.kind == Kind::Working;
        if working && status.status.kind == Kind::Idle {
            break;
        }
    }
    assert!(
        model.journal().iter().any(|entry| entry["body"]["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty())
            && entry["body"]["messages"]
                .as_array()
                .is_some_and(
                    |messages| messages.iter().any(|message| message["role"] == "user"
                        && message["content"]
                            .as_str()
                            .is_some_and(|text| text.contains("preserve the terminals")
                                && text.contains("workstream_handoff")))
                )),
        "Clear must reach real Claude"
    );
    let actor = project.client().await;
    actor
        .request(
            "daemon.identify",
            contracts::IdentifyParams {
                actor: contracts::Actor {
                    kind: contracts::ActorKind::Agent,
                    id: door.clone(),
                    parent: None,
                },
            },
        )
        .await
        .unwrap();
    actor
        .request(
            "workstream.handoff",
            json!({"id": door, "summary": "Terminals survived", "keep_todos": [], "keep_pads": []}),
        )
        .await
        .unwrap();
    let cleared = next(&mut client, |event| match event {
        EventData::WorkstreamCleared(cleared) if cleared.id == door => Some(cleared),
        _ => None,
    })
    .await;
    assert_eq!(cleared.iteration, 2);
    let mut working = false;
    loop {
        let status = next(&mut client, |event| match event {
            EventData::AgentStatus(status) if status.id == door && status.attempt == "2" => {
                Some(status)
            }
            _ => None,
        })
        .await;
        working |= status.status.kind == Kind::Working;
        if working && status.status.kind == Kind::Idle {
            break;
        }
    }
    assert!(
        model.journal().iter().any(|entry| entry["body"]["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty())
            && entry["body"]["messages"]
                .as_array()
                .is_some_and(
                    |messages| messages.iter().any(|message| message["role"] == "user"
                        && message["content"].as_str().is_some_and(
                            |text| text.contains("Follow this instruction: Terminals survived")
                        ))
                )),
        "the real Door must receive the valid Handoff: {:#?}",
        model.journal()
    );
    let tree = rail_tree(&client).await;
    let after = tree.iter().find(|node| node.id == door).unwrap();
    assert_eq!(
        (&after.name, &after.parent, after.order),
        (&before.name, &before.parent, before.order)
    );
    let terminals = client.request("terminal.list", Value::Null).await.unwrap();
    let kept = terminals
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == terminal)
        .expect("the Terminal stays");
    assert_eq!(kept["running"], true);
    assert!(project.dir.path().join("README").is_file());
    model.assert_matched();
}

#[tokio::test]
async fn a32_worktrees_and_branches_survive_even_dirty() {
    let (project, mut client, door) = clear_fixture("a32_worktrees", &[]).await;
    let model = project.model.as_ref().unwrap();
    let trees: Vec<_> = rail_tree(&client)
        .await
        .into_iter()
        .filter(|node| node.id != door)
        .map(|node| node.worktree.expect("child has a Worktree"))
        .collect();
    assert_eq!(trees.len(), 2);
    std::fs::write(
        std::path::Path::new(&trees[1].path).join("dirty.txt"),
        "unlanded",
    )
    .unwrap();
    assert_eq!(
        client
            .request(
                "workstream.clear",
                json!({"id": door, "reason": "preserve dirty worktrees"})
            )
            .await
            .unwrap(),
        json!({"pending": true})
    );
    let mut working = false;
    loop {
        let status = next(&mut client, |event| match event {
            EventData::AgentStatus(status) if status.id == door => Some(status),
            _ => None,
        })
        .await;
        working |= status.status.kind == Kind::Working;
        if working && status.status.kind == Kind::Idle {
            break;
        }
    }
    assert!(
        model.journal().iter().any(|entry| entry["body"]["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty())
            && entry["body"]["messages"]
                .as_array()
                .is_some_and(
                    |messages| messages.iter().any(|message| message["role"] == "user"
                        && message["content"]
                            .as_str()
                            .is_some_and(|text| text.contains("preserve dirty worktrees")
                                && text.contains("workstream_handoff")))
                )),
        "Clear must reach real Claude"
    );
    let actor = project.client().await;
    actor
        .request(
            "daemon.identify",
            contracts::IdentifyParams {
                actor: contracts::Actor {
                    kind: contracts::ActorKind::Agent,
                    id: door.clone(),
                    parent: None,
                },
            },
        )
        .await
        .unwrap();
    actor
        .request(
            "workstream.handoff",
            json!({"id": door, "summary": "Worktrees survived", "keep_todos": [], "keep_pads": []}),
        )
        .await
        .unwrap();
    let cleared = next(&mut client, |event| match event {
        EventData::WorkstreamCleared(cleared) if cleared.id == door => Some(cleared),
        _ => None,
    })
    .await;
    assert_eq!(cleared.iteration, 2);
    let mut working = false;
    loop {
        let status = next(&mut client, |event| match event {
            EventData::AgentStatus(status) if status.id == door && status.attempt == "2" => {
                Some(status)
            }
            _ => None,
        })
        .await;
        working |= status.status.kind == Kind::Working;
        if working && status.status.kind == Kind::Idle {
            break;
        }
    }
    let branches = std::process::Command::new("git")
        .current_dir(project.dir.path())
        .args(["branch", "--list"])
        .output()
        .unwrap();
    assert!(branches.status.success());
    let branches = String::from_utf8(branches.stdout).unwrap();
    for tree in &trees {
        assert!(
            std::path::Path::new(&tree.path).is_dir(),
            "{} was removed",
            tree.path
        );
        assert!(branches.contains(&tree.branch), "{branches}");
    }
    assert!(
        std::path::Path::new(&trees[1].path)
            .join("dirty.txt")
            .is_file()
    );
    let journal = model.journal();
    let handoff = journal
        .iter()
        .filter(|entry| entry["body"]["tools"].is_array())
        .flat_map(|entry| entry["body"]["messages"].as_array().into_iter().flatten())
        .filter(|message| message["role"] == "user")
        .filter_map(|message| message["content"].as_str())
        .find(|text| text.contains("Follow this instruction: Worktrees survived"))
        .expect("the real Door receives the Handoff");
    for tree in &trees {
        assert!(
            handoff.contains(&tree.path) && handoff.contains(&tree.branch),
            "{handoff}"
        );
    }
    model.assert_matched();
}

#[tokio::test]
async fn a32_unkept_todos_and_pads_are_archived_not_lost() {
    let (project, mut client, door) = clear_fixture("a32_archive", &[]).await;
    let model = project.model.as_ref().unwrap();
    let todos = client.request("todo.list", Value::Null).await.unwrap();
    let todos = todos.as_array().unwrap();
    let keep = todos
        .iter()
        .find(|todo| todo["title"] == "build the login page")
        .unwrap()["id"]
        .clone();
    let fluff = todos
        .iter()
        .find(|todo| todo["title"] == "refactor everything")
        .unwrap();
    let fluff_id = fluff["id"].clone();
    let owner = fluff["creator"]["id"].as_str().unwrap().to_owned();
    assert_eq!(
        client
            .request(
                "workstream.clear",
                json!({"id": door, "reason": "archive unkept items"})
            )
            .await
            .unwrap(),
        json!({"pending": true})
    );
    let mut working = false;
    loop {
        let status = next(&mut client, |event| match event {
            EventData::AgentStatus(status) if status.id == door => Some(status),
            _ => None,
        })
        .await;
        working |= status.status.kind == Kind::Working;
        if working && status.status.kind == Kind::Idle {
            break;
        }
    }
    assert!(
        model.journal().iter().any(|entry| entry["body"]["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty())
            && entry["body"]["messages"]
                .as_array()
                .is_some_and(
                    |messages| messages.iter().any(|message| message["role"] == "user"
                        && message["content"]
                            .as_str()
                            .is_some_and(|text| text.contains("archive unkept items")
                                && text.contains("workstream_handoff")))
                )),
        "Clear must reach real Claude"
    );
    let actor = project.client().await;
    actor
        .request(
            "daemon.identify",
            contracts::IdentifyParams {
                actor: contracts::Actor {
                    kind: contracts::ActorKind::Agent,
                    id: door.clone(),
                    parent: None,
                },
            },
        )
        .await
        .unwrap();
    // The restarted Door can emit Working before workstream.cleared (clear::apply).
    // Keep those statuses while the other subscription waits for the clear result.
    let mut restarted = project.subscribed().await;
    actor
        .request(
            "workstream.handoff",
            json!({"id": door, "summary": "Kept items survived", "keep_todos": [keep], "keep_pads": ["notes"]}),
        )
        .await
        .unwrap();
    let cleared = next(&mut client, |event| match event {
        EventData::WorkstreamCleared(cleared) if cleared.id == door => Some(cleared),
        _ => None,
    })
    .await;
    assert_eq!(cleared.iteration, 2);
    let mut working = false;
    loop {
        let status = next(&mut restarted, |event| match event {
            EventData::AgentStatus(status) if status.id == door && status.attempt == "2" => {
                Some(status)
            }
            _ => None,
        })
        .await;
        working |= status.status.kind == Kind::Working;
        if working && status.status.kind == Kind::Idle {
            break;
        }
    }
    assert!(
        model.journal().iter().any(|entry| entry["body"]["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty())
            && entry["body"]["messages"]
                .as_array()
                .is_some_and(
                    |messages| messages.iter().any(|message| message["role"] == "user"
                        && message["content"]
                            .as_str()
                            .is_some_and(|text| text
                                .contains("Follow this instruction: Kept items survived")))
                )),
        "the real Door must receive the valid Handoff: {:#?}",
        model.journal()
    );
    let todos = client.request("todo.list", Value::Null).await.unwrap();
    assert_eq!(
        todos
            .as_array()
            .unwrap()
            .iter()
            .map(|todo| todo["id"].clone())
            .collect::<Vec<_>>(),
        [keep]
    );
    let pads = client.request("pad.list", Value::Null).await.unwrap();
    let pads = pads.as_array().unwrap();
    assert_eq!(pads.len(), 2);
    let archive = pads
        .iter()
        .find(|pad| pad["name"] == "archive-1")
        .expect("the archive Pad");
    assert_eq!(archive["owner"]["kind"], "user");
    let text = archive["text"].as_str().unwrap();
    assert!(
        text.contains(&format!(
            "## refactor everything (todo {fluff_id}, owner {owner})"
        )),
        "{text}"
    );
    assert!(text.contains("refactor everything body"), "{text}");
    assert!(text.contains("## scratch (pad, owner "), "{text}");
    assert!(text.contains("scratch text"), "{text}");
    assert!(!text.contains("build the login page"), "{text}");
    model.assert_matched();
}

#[tokio::test]
async fn a32_kept_items_keep_their_ids_and_owners() {
    let (project, mut client, door) = clear_fixture("a32_kept_items", &[]).await;
    let model = project.model.as_ref().unwrap();
    let todos = client.request("todo.list", Value::Null).await.unwrap();
    let todos = todos.as_array().unwrap();
    let keep = todos
        .iter()
        .find(|todo| todo["title"] == "build the login page")
        .unwrap()["id"]
        .clone();
    let fluff = todos
        .iter()
        .find(|todo| todo["title"] == "refactor everything")
        .unwrap();
    let fluff_id = fluff["id"].clone();
    client
        .request(
            "todo.setBlockers",
            json!({"id": keep, "blockers": [fluff_id]}),
        )
        .await
        .unwrap();
    let before = client.request("todo.list", Value::Null).await.unwrap();
    let before = before
        .as_array()
        .unwrap()
        .iter()
        .find(|todo| todo["id"] == keep)
        .unwrap();
    assert_eq!(
        client
            .request(
                "workstream.clear",
                json!({"id": door, "reason": "preserve kept items"})
            )
            .await
            .unwrap(),
        json!({"pending": true})
    );
    let mut working = false;
    loop {
        let status = next(&mut client, |event| match event {
            EventData::AgentStatus(status) if status.id == door => Some(status),
            _ => None,
        })
        .await;
        working |= status.status.kind == Kind::Working;
        // A33: the kept Todo is blocked until Clear archives its blocker.
        if working && status.status.kind == Kind::Blocked {
            break;
        }
    }
    assert!(
        model.journal().iter().any(|entry| entry["body"]["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty())
            && entry["body"]["messages"]
                .as_array()
                .is_some_and(
                    |messages| messages.iter().any(|message| message["role"] == "user"
                        && message["content"]
                            .as_str()
                            .is_some_and(|text| text.contains("preserve kept items")
                                && text.contains("workstream_handoff")))
                )),
        "Clear must reach real Claude"
    );
    let actor = project.client().await;
    actor
        .request(
            "daemon.identify",
            contracts::IdentifyParams {
                actor: contracts::Actor {
                    kind: contracts::ActorKind::Agent,
                    id: door.clone(),
                    parent: None,
                },
            },
        )
        .await
        .unwrap();
    actor
        .request(
            "workstream.handoff",
            json!({"id": door, "summary": "Kept items survived", "keep_todos": [keep], "keep_pads": ["scratch"]}),
        )
        .await
        .unwrap();
    let cleared = next(&mut client, |event| match event {
        EventData::WorkstreamCleared(cleared) if cleared.id == door => Some(cleared),
        _ => None,
    })
    .await;
    assert_eq!(cleared.iteration, 2);
    let mut working = false;
    loop {
        let status = next(&mut client, |event| match event {
            EventData::AgentStatus(status) if status.id == door && status.attempt == "2" => {
                Some(status)
            }
            _ => None,
        })
        .await;
        working |= status.status.kind == Kind::Working;
        if working && status.status.kind == Kind::Idle {
            break;
        }
    }
    assert!(
        model.journal().iter().any(|entry| entry["body"]["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty())
            && entry["body"]["messages"]
                .as_array()
                .is_some_and(
                    |messages| messages.iter().any(|message| message["role"] == "user"
                        && message["content"]
                            .as_str()
                            .is_some_and(|text| text
                                .contains("Follow this instruction: Kept items survived")))
                )),
        "the real Door must receive the valid Handoff: {:#?}",
        model.journal()
    );
    let todos = client.request("todo.list", Value::Null).await.unwrap();
    let after = todos
        .as_array()
        .unwrap()
        .iter()
        .find(|todo| todo["id"] == keep)
        .unwrap();
    assert_eq!(
        after["blockers"],
        json!([]),
        "an archived blocker leaves the list"
    );
    assert_eq!(
        (&after["title"], &after["creator"], &after["home"]),
        (&before["title"], &before["creator"], &before["home"])
    );
    let pads = client.request("pad.list", Value::Null).await.unwrap();
    let scratch = pads
        .as_array()
        .unwrap()
        .iter()
        .find(|pad| pad["name"] == "scratch")
        .unwrap();
    assert_eq!(
        scratch["owner"]["kind"], "user",
        "a Pad of a removed Subagent becomes the user's"
    );
    assert_eq!(scratch["home"], json!(door));
    model.assert_matched();
}

#[tokio::test]
async fn a32_message_list_spans_iterations() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let project = daemon::start_real("a32_message_history", &[]);
        let path = project.dir.path();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.name", "roundup"],
            vec!["config", "user.email", "roundup@example.com"],
        ] {
            assert!(
                std::process::Command::new("git")
                    .current_dir(path)
                    .args(args)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        std::fs::write(path.join("README"), "keep").unwrap();
        for args in [vec!["add", "README"], vec!["commit", "-qm", "base"]] {
            assert!(
                std::process::Command::new("git")
                    .current_dir(path)
                    .args(args)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let mut client = project.subscribed().await;
        client
            .request("project.setWorktrees", json!({"on": true, "check": null}))
            .await
            .unwrap();
        let door = client
            .request(
                "rail.createWorkstream",
                json!({"name": "team", "parent": null}),
            )
            .await
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        client
            .request("rail.startDoor", json!({"id": door}))
            .await
            .unwrap();
        let model = project.model.as_ref().unwrap();
        // The first turn is the Door's start-up: its Working → Idle is the only Status pair there. Every
        // later input follows the model's journal, because an Idle can arrive while the next turn begins.
        async fn stopped(client: &mut rpc::Client, door: &str, attempt: &str) {
            let mut working = false;
            loop {
                let status = next(client, |event| match event {
                    EventData::AgentStatus(status)
                        if status.id == door && status.attempt == attempt =>
                    {
                        Some(status)
                    }
                    _ => None,
                })
                .await;
                working |= status.status.kind == Kind::Working;
                if working && status.status.kind == Kind::Idle {
                    return;
                }
            }
        }
        stopped(&mut client, &door, "1").await;
        client
            .request(
                "message.send",
                json!({"to": door, "kind": "note", "body": "first ask"}),
            )
            .await
            .unwrap();
        turn_done(model, &client, &door, "first ask").await;
        assert_eq!(
            client
                .request(
                    "workstream.clear",
                    json!({"id": door, "reason": "history clear"})
                )
                .await
                .unwrap(),
            json!({"pending": true})
        );
        received(model, "history clear").await;
        let actor = project.client().await;
        actor
            .request(
                "daemon.identify",
                contracts::IdentifyParams {
                    actor: contracts::Actor {
                        kind: contracts::ActorKind::Agent,
                        id: door.clone(),
                        parent: None,
                    },
                },
            )
            .await
            .unwrap();
        actor
        .request(
            "workstream.handoff",
            json!({"id": door, "summary": "History survived", "keep_todos": [], "keep_pads": []}),
        )
        .await
        .unwrap();
        let cleared = next(&mut client, |event| match event {
            EventData::WorkstreamCleared(cleared) if cleared.id == door => Some(cleared),
            _ => None,
        })
        .await;
        assert_eq!(cleared.iteration, 2);
        turn_done(model, &client, &door, "History survived").await;
        client
            .request(
                "message.send",
                json!({"to": door, "kind": "note", "body": "second ask"}),
            )
            .await
            .unwrap();
        turn_done(model, &client, &door, "second ask").await;
        let listed = client
            .request("message.list", json!({"id": door}))
            .await
            .unwrap();
        let seen: Vec<(String, u64)> = listed
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| !m["body"].as_str().unwrap().starts_with("[digest]"))
            .map(|m| {
                (
                    m["body"].as_str().unwrap().to_owned(),
                    m["iteration"].as_u64().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            seen,
            [("first ask".to_owned(), 1), ("second ask".to_owned(), 2)]
        );
        let journal = model.journal();
        for expected in [
            "first ask",
            "history clear",
            "History survived",
            "second ask",
        ] {
            assert!(
                journal.iter().any(|entry| entry["body"]["tools"]
                    .as_array()
                    .is_some_and(|tools| !tools.is_empty())
                    && entry["body"]["messages"]
                        .as_array()
                        .is_some_and(|messages| messages.iter().any(|m| m["role"] == "user"
                            && m["content"]
                                .as_str()
                                .is_some_and(|text| text.contains(expected))))),
                "real Claude did not receive {expected}: {journal:#?}"
            );
        }
        model.assert_matched();
    })
    .await
    .expect("Message history across Clear did not complete");
}

#[tokio::test]
async fn a32_iteration_advances_once() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let (project, mut client, door) = clear_fixture("a32_iteration", &[]).await;
        let model = project.model.as_ref().unwrap();
        let before = rail_tree(&client).await;
        assert_eq!(
            before
                .iter()
                .find(|node| node.id == door)
                .unwrap()
                .iteration,
            Some(1)
        );
        assert!(
            before
                .iter()
                .filter(|node| node.id != door)
                .all(|node| node.iteration.is_none())
        );
        client
            .request(
                "workstream.clear",
                json!({"id": door, "reason": "advance iteration"}),
            )
            .await
            .unwrap();
        received(model, "advance iteration").await;
        let actor = project.client().await;
        actor
            .request(
                "daemon.identify",
                contracts::IdentifyParams {
                    actor: contracts::Actor {
                        kind: contracts::ActorKind::Agent,
                        id: door.clone(),
                        parent: None,
                    },
                },
            )
            .await
            .unwrap();
        actor
            .request(
                "workstream.handoff",
                json!({"id": door, "summary": "Iteration advanced", "keep_todos": [], "keep_pads": []}),
            )
            .await
            .unwrap();
        let cleared = next(&mut client, |event| match event {
            EventData::WorkstreamCleared(cleared) if cleared.id == door => Some(cleared),
            _ => None,
        })
        .await;
        assert_eq!((cleared.id.as_str(), cleared.iteration), (door.as_str(), 2));
        assert_eq!(
            rail_tree(&client)
                .await
                .iter()
                .find(|node| node.id == door)
                .unwrap()
                .iteration,
            Some(2)
        );
        // The retired Subagents' Digest would start a turn after the Handoff's; held, only one turn runs
        // before the next Clear.
        client
            .request("takeover.begin", json!({"agent": door}))
            .await
            .unwrap();
        turn_done(model, &client, &door, "Iteration advanced").await;
        client
            .request(
                "workstream.clear",
                json!({"id": door, "reason": "advance again"}),
            )
            .await
            .unwrap();
        received(model, "advance again").await;
        let journal = model.journal();
        for expected in ["advance iteration", "Iteration advanced", "advance again"] {
            assert!(
                journal.iter().any(|entry| entry["body"]["tools"].is_array()
                    && entry["body"]["messages"]
                        .as_array()
                        .is_some_and(|messages| messages.iter().any(|message| message["role"]
                            == "user"
                            && message["content"]
                                .as_str()
                                .is_some_and(|text| text.contains(expected)
                                    && (expected == "Iteration advanced" || text.contains("workstream_handoff")))))),
                "real Claude did not receive {expected}: {journal:#?}"
            );
        }
        assert_eq!(
            rail_tree(&client)
                .await
                .iter()
                .find(|node| node.id == door)
                .unwrap()
                .iteration,
            Some(2)
        );
        model.assert_matched();
    })
    .await
    .expect("Iteration observer did not complete");
}

#[tokio::test]
async fn a32_without_handoff_archives_everything() {
    let (project, mut client, door) = clear_fixture(
        "a32_without_handoff",
        &[("ROUNDUP_HANDOFF_BOUND_MS", "10000")],
    )
    .await;
    let model = project.model.as_ref().unwrap();
    let child = rail_tree(&client)
        .await
        .into_iter()
        .find(|node| node.parent.as_deref() == Some(&door))
        .unwrap()
        .id;
    assert_eq!(
        client
            .request("workstream.clear", json!({"id": door, "reason": "bloat"}))
            .await
            .unwrap(),
        json!({"pending": true})
    );
    let failed = next(&mut client, |event| match event {
        EventData::WorkstreamClearFailed(failed) if failed.id == door => Some(failed),
        _ => None,
    })
    .await;
    assert_eq!(failed.code, rpc::code::NOT_ACKED as i32);
    assert_eq!(
        client
            .request(
                "workstream.clear",
                json!({
                    "id": door, "reason": "bloat", "without_handoff": true
                })
            )
            .await
            .unwrap(),
        json!({"pending": true})
    );
    let cleared = next(&mut client, |event| match event {
        EventData::WorkstreamCleared(cleared) if cleared.id == door => Some(cleared),
        _ => None,
    })
    .await;
    assert_eq!(cleared.iteration, 2);
    let mut working = false;
    loop {
        let status = next(&mut client, |event| match event {
            EventData::AgentStatus(status) if status.id == door && status.attempt == "2" => {
                Some(status)
            }
            _ => None,
        })
        .await;
        assert_ne!(
            status.status.kind,
            Kind::Error,
            "real Door failed: {:#?}",
            model.journal()
        );
        working |= status.status.kind == Kind::Working;
        if working && status.status.kind == Kind::Idle {
            break;
        }
    }
    assert!(
        client
            .request("todo.list", Value::Null)
            .await
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
    let pads = client.request("pad.list", Value::Null).await.unwrap();
    let pads = pads.as_array().unwrap();
    assert_eq!(
        pads.iter()
            .map(|pad| pad["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["archive-1"]
    );
    let text = pads[0]["text"].as_str().unwrap();
    for lost in [
        "build the login page",
        "refactor everything",
        "## notes",
        "## scratch",
    ] {
        assert!(text.contains(lost), "{lost} is lost: {text}");
    }
    assert!(rail_tree(&client).await.iter().all(|node| node.id != child));
    for expected in [
        "workstream_handoff",
        "No Handoff was written, so every Todo and Pad was archived.",
    ] {
        assert!(
            model.journal().iter().any(|entry| entry["body"]["tools"]
                .as_array()
                .is_some_and(|tools| !tools.is_empty())
                && entry["body"]["messages"]
                    .as_array()
                    .is_some_and(|messages| messages.iter().any(|message| message["role"]
                        == "user"
                        && message["content"].as_str().is_some_and(|text| text
                            .contains("bloat")
                            && text.contains(expected))))),
            "real Claude must receive {expected}: {:#?}",
            model.journal()
        );
    }
    model.assert_matched();
}

#[tokio::test]
async fn a32_the_door_restarts_fresh_from_the_handoff() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let (project, mut client, door) = clear_fixture("a32_fresh_handoff", &[]).await;
        let model = project.model.as_ref().unwrap();
        let before = rail_tree(&client).await;
        let first = before.iter().find(|node| node.id == door).unwrap();
        client
            .request(
                "workstream.clear",
                json!({"id": door, "reason": "bloat"}),
            )
            .await
            .unwrap();
        let mut working = false;
        loop {
            let status = next(&mut client, |event| match event {
                EventData::AgentStatus(status) if status.id == door => Some(status),
                _ => None,
            })
            .await;
            working |= status.status.kind == Kind::Working;
            if working && status.status.kind == Kind::Idle {
                break;
            }
        }
        let actor = project.client().await;
        actor
            .request(
                "daemon.identify",
                contracts::IdentifyParams {
                    actor: contracts::Actor {
                        kind: contracts::ActorKind::Agent,
                        id: door.clone(),
                        parent: None,
                    },
                },
            )
            .await
            .unwrap();
        actor
            .request(
                "workstream.handoff",
                json!({"id": door, "summary": "Build the login page.", "keep_todos": [], "keep_pads": []}),
            )
            .await
            .unwrap();
        let cleared = next(&mut client, |event| match event {
            EventData::WorkstreamCleared(cleared) if cleared.id == door => Some(cleared),
            _ => None,
        })
        .await;
        assert_eq!((cleared.id.as_str(), cleared.iteration), (door.as_str(), 2));
        assert_eq!(
            rail_tree(&client)
                .await
                .iter()
                .find(|node| node.id == door)
                .unwrap()
                .iteration,
            Some(2)
        );
        let mut working = false;
        loop {
            let status = next(&mut client, |event| match event {
                EventData::AgentStatus(status) if status.id == door && status.attempt == "2" => {
                    Some(status)
                }
                _ => None,
            })
            .await;
            working |= status.status.kind == Kind::Working;
            if working && status.status.kind == Kind::Idle {
                break;
            }
        }
        let tree = rail_tree(&client).await;
        let after = tree.iter().find(|node| node.id == door).unwrap();
        assert_eq!(after.attempt.as_deref(), Some("2"));
        assert_ne!(after.terminal_id, first.terminal_id);
        let journal = model.journal();
        let restarted = journal.iter().find(|entry| entry["body"]["tools"].is_array()
            && entry["body"]["messages"].as_array().is_some_and(|messages|
                messages.iter().any(|message| message["role"] == "user"
                    && message["content"].as_str().is_some_and(|text|
                        text.contains("Follow this instruction: Build the login page.")
                        && text.contains("archived in the Pad archive-1")))))
            .unwrap_or_else(|| panic!("real Door receives the Handoff and archive pointer: {journal:#?}"));
        // A resumed conversation would retain the earlier Clear and child-spawn turns.
        let messages = restarted["body"]["messages"].as_array().unwrap();
        assert!(!messages.iter().any(|message| message["role"] == "tool"));
        assert!(!messages.iter().any(|message| message["role"] == "user" && message["content"].as_str()
            .is_some_and(|text| text.contains("workstream_handoff") || text.contains("What is this Workstream for?"))));
        assert!(journal.iter().any(|entry| entry["body"]["tools"].is_array()
            && entry["body"]["messages"].as_array().is_some_and(|messages|
                messages.iter().any(|message| message["role"] == "user"
                    && message["content"].as_str().is_some_and(|text|
                        text.contains("bloat") && text.contains("workstream_handoff"))))));
        model.assert_matched();
    })
    .await
    .expect("fresh Door Handoff observer did not complete");
}

#[tokio::test]
async fn a32_subagents_stop_deepest_first_and_leave_the_rail() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let (project, mut client, door) = clear_fixture("a32_stop_deepest", &[]).await;
        let model = project.model.as_ref().unwrap();
        let before = rail_tree(&client).await;
        let child = before.iter().find(|node| node.parent.as_deref() == Some(&door)).unwrap();
        let grandchild = before.iter().find(|node| node.parent.as_deref() == Some(&child.id)).unwrap();
        let children = [&child.id, &grandchild.id];
        client
            .request(
                "workstream.clear",
                json!({"id": door, "reason": "bloat"}),
            )
            .await
            .unwrap();
        let mut working = false;
        loop {
            let status = next(&mut client, |event| match event {
                EventData::AgentStatus(status) if status.id == door => Some(status),
                _ => None,
            })
            .await;
            working |= status.status.kind == Kind::Working;
            if working && status.status.kind == Kind::Idle {
                break;
            }
        }
        let actor = project.client().await;
        actor
            .request(
                "daemon.identify",
                contracts::IdentifyParams {
                    actor: contracts::Actor {
                        kind: contracts::ActorKind::Agent,
                        id: door.clone(),
                        parent: None,
                    },
                },
            )
            .await
            .unwrap();
        actor
            .request(
                "workstream.handoff",
                json!({"id": door, "summary": "Build the login page.", "keep_todos": [], "keep_pads": []}),
            )
            .await
            .unwrap();
        let cleared = next(&mut client, |event| match event {
            EventData::WorkstreamCleared(cleared) if cleared.id == door => Some(cleared),
            _ => None,
        })
        .await;
        assert_eq!((cleared.id.as_str(), cleared.iteration), (door.as_str(), 2));
        assert_eq!(
            rail_tree(&client)
                .await
                .iter()
                .find(|node| node.id == door)
                .unwrap()
                .iteration,
            Some(2)
        );
        let mut working = false;
        loop {
            let status = next(&mut client, |event| match event {
                EventData::AgentStatus(status) if status.id == door && status.attempt == "2" => {
                    Some(status)
                }
                _ => None,
            })
            .await;
            working |= status.status.kind == Kind::Working;
            if working && status.status.kind == Kind::Idle {
                break;
            }
        }
        let tree = rail_tree(&client).await;
        assert!(tree.iter().any(|node| node.id == door));
        assert!(tree.iter().all(|node| !children.contains(&&node.id)));
        let journal = model.journal();
        let restarted = journal.iter().find(|entry| entry["body"]["tools"].is_array()
            && entry["body"]["messages"].as_array().is_some_and(|messages|
                messages.iter().any(|message| message["role"] == "user"
                    && message["content"].as_str().is_some_and(|text|
                        text.contains("Follow this instruction: Build the login page.")
                        && text.contains("archived in the Pad archive-1")))))
            .unwrap_or_else(|| panic!("real Door receives the Handoff and archive pointer: {journal:#?}"));
        let messages = restarted["body"]["messages"].as_array().unwrap();
        let handoff = messages.iter().filter(|message| message["role"] == "user")
            .filter_map(|message| message["content"].as_str())
            .find(|text| text.contains("Follow this instruction: Build the login page."))
            .unwrap();
        let child_branch = format!("branch roundup/agent-{}", child.id);
        let grandchild_branch = format!("branch roundup/agent-{}", grandchild.id);
        let shallow = handoff.find(&child_branch).expect("child Worktree in Handoff");
        let deep = handoff.find(&grandchild_branch).expect("grandchild Worktree in Handoff");
        assert!(deep < shallow, "deepest Subagent is handed off first: {handoff}");
        model.assert_matched();
    })
    .await
    .expect("fresh Door Handoff observer did not complete");
}

/// A launch gate over real Claude: starts pass while `hold` exists; a start without it waits and creates `held`.
struct Gate {
    dir: tempfile::TempDir,
    hold: std::path::PathBuf,
    held: std::path::PathBuf,
    launches: std::path::PathBuf,
}

impl Gate {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let hold = dir.path().join("hold-start");
        std::fs::write(&hold, "").unwrap();
        Self {
            hold,
            held: dir.path().join("held-start"),
            launches: dir.path().join("launches.jsonl"),
            dir,
        }
    }

    fn wrapper(&self) -> String {
        aimock::launch_wrapper(self.dir.path(), &self.hold, &self.held, &self.launches)
            .to_str()
            .unwrap()
            .to_owned()
    }

    /// The Agent starts; the Daemon's own `claude plugin test` runs pass through the gate too.
    fn rows(&self) -> Vec<Value> {
        std::fs::read_to_string(&self.launches)
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|row| row["argv"][0] == "--settings")
            .collect()
    }
}

async fn gated_clear_fixture(fixture: &str, gate: &Gate) -> (daemon::Project, rpc::Client, String) {
    let wrapper = gate.wrapper();
    clear_fixture(
        fixture,
        &[
            ("ROUNDUP_CLAUDE_BIN", &wrapper),
            ("ROUNDUP_HANDOFF_BOUND_MS", "60000"),
            // These tests wait for a restart that fails, so it keeps the Daemon's own bound.
            ("ROUNDUP_RESTART_BOUND_MS", "8000"),
        ],
    )
    .await
}

async fn until<T>(what: &str, mut probe: impl FnMut() -> Option<T>) -> T {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Some(found) = probe() {
                return found;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("never: {what}"))
}

async fn idle(client: &rpc::Client, id: &str) {
    until_async(&format!("{id} idle"), || async {
        rail_tree(client)
            .await
            .into_iter()
            .find(|n| n.id == id)
            .and_then(|n| n.status)
            .is_some_and(|s| s.kind == Kind::Idle)
            .then_some(())
    })
    .await;
}

async fn until_async<T>(what: &str, mut probe: impl AsyncFnMut() -> Option<T>) -> T {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Some(found) = probe().await {
                return found;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("never: {what}"))
}

/// The model's journal holds a user message with `needle`: the prompt reached real Claude.
async fn received(model: &aimock::Model, needle: &str) {
    until(&format!("the model received {needle:?}"), || {
        model
            .journal()
            .iter()
            .any(|entry| {
                entry["body"]["messages"]
                    .as_array()
                    .is_some_and(|messages| {
                        messages.iter().any(|m| {
                            m["role"] == "user"
                                && m["content"].as_str().is_some_and(|t| t.contains(needle))
                        })
                    })
            })
            .then_some(())
    })
    .await;
}

/// The Door is asked to hand off once the model's journal holds the Clear prompt.
async fn asked(model: &aimock::Model) {
    received(model, "workstream_handoff").await;
}

/// The Door ended the turn that took a prompt holding `needle`. The model's journal proves the prompt
/// arrived, and an Idle read after that is the turn's end: a Status event is not, because an Idle
/// can arrive as the next turn begins.
async fn turn_done(model: &aimock::Model, client: &rpc::Client, door: &str, needle: &str) {
    received(model, needle).await;
    idle(client, door).await;
}

async fn as_door(project: &daemon::Project, door: &str) -> rpc::Client {
    let actor = project.client().await;
    actor
        .request(
            "daemon.identify",
            contracts::IdentifyParams {
                actor: contracts::Actor {
                    kind: contracts::ActorKind::Agent,
                    id: door.to_owned(),
                    parent: None,
                },
            },
        )
        .await
        .unwrap();
    actor
}

async fn child_of(client: &rpc::Client, parent: &str) -> String {
    rail_tree(client)
        .await
        .into_iter()
        .find(|node| node.parent.as_deref() == Some(parent))
        .unwrap()
        .id
}

/// Leave Subagent `a` mid-resume, which no stop can claim: it was stopped, and its start waits
/// before real Claude runs, so no SessionStart acknowledges the resume.
async fn hold_a_resume(
    client: &rpc::Client,
    project: &daemon::Project,
    gate: &Gate,
    a: &str,
) -> tokio::task::JoinHandle<Result<Value, rpc::RpcError>> {
    client
        .request("agent.stop", json!({"id": a}))
        .await
        .unwrap();
    std::fs::remove_file(&gate.hold).unwrap();
    let resumer = project.client().await;
    let id = a.to_owned();
    let resume =
        tokio::spawn(async move { resumer.request("agent.resume", json!({"id": id})).await });
    until("resume held before SessionStart", || {
        gate.held.is_file().then_some(())
    })
    .await;
    assert!(
        !resume.is_finished(),
        "resume completed without SessionStart"
    );
    resume
}

#[tokio::test]
async fn a32_resume_is_held_until_session_start() {
    let gate = Gate::new();
    let (project, client, door) = gated_clear_fixture("a32_resume_held", &gate).await;
    let a = child_of(&client, &door).await;
    let resume = hold_a_resume(&client, &project, &gate, &a).await;
    std::fs::write(&gate.hold, "").unwrap();
    tokio::time::timeout(Duration::from_secs(30), resume)
        .await
        .expect("resume did not receive SessionStart")
        .unwrap()
        .unwrap();
    idle(&client, &a).await;
    project.model.as_ref().unwrap().assert_matched();
}

#[tokio::test]
async fn a32_a_failed_stop_retries_without_double_archive() {
    let gate = Gate::new();
    let (project, mut client, door) = gated_clear_fixture("a32_failed_stop", &gate).await;
    let model = project.model.as_ref().unwrap();
    let a = child_of(&client, &door).await;
    let b = child_of(&client, &a).await;
    let sections = |pads: &Value| {
        let archive = pads
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == "archive-1")
            .unwrap();
        let text = archive["text"].as_str().unwrap();
        text.matches("\n## ").count() + usize::from(text.starts_with("## "))
    };
    assert_eq!(
        client
            .request("workstream.clear", json!({"id": door, "reason": "bloat"}))
            .await
            .unwrap(),
        json!({"pending": true})
    );
    asked(model).await;
    hold_a_resume(&client, &project, &gate, &a).await;
    as_door(&project, &door)
        .await
        .request(
            "workstream.handoff",
            json!({"id": door, "summary": "Retry the Handoff.", "keep_todos": [], "keep_pads": []}),
        )
        .await
        .unwrap();
    let failed = next(&mut client, |event| match event {
        EventData::WorkstreamClearFailed(failed) if failed.id == door => Some(failed),
        _ => None,
    })
    .await;
    assert_eq!(
        failed.code,
        rpc::code::CONFLICT as i32,
        "{}",
        failed.message
    );

    let ids: Vec<_> = rail_tree(&client).await.into_iter().map(|n| n.id).collect();
    assert!(!ids.contains(&b), "the deepest Subagent stopped first");
    assert!(ids.contains(&a), "{ids:?}");
    assert!(ids.contains(&door));
    assert_eq!(
        rail_tree(&client)
            .await
            .iter()
            .find(|n| n.id == door)
            .unwrap()
            .iteration,
        Some(1)
    );
    let pads = client.request("pad.list", Value::Null).await.unwrap();
    assert_eq!(sections(&pads), 4, "every item archived once");

    std::fs::write(&gate.hold, "").unwrap();
    idle(&client, &a).await;
    assert_eq!(
        client
            .request("workstream.clear", json!({"id": door, "reason": "bloat"}))
            .await
            .unwrap(),
        json!({"pending": true})
    );
    let cleared = next(&mut client, |event| match event {
        EventData::WorkstreamCleared(cleared) if cleared.id == door => Some(cleared),
        _ => None,
    })
    .await;
    assert_eq!(cleared.iteration, 2);
    let ids: Vec<_> = rail_tree(&client).await.into_iter().map(|n| n.id).collect();
    assert_eq!(ids.iter().filter(|id| **id == door).count(), 1);
    assert!(!ids.contains(&a));
    let pads = client.request("pad.list", Value::Null).await.unwrap();
    assert_eq!(sections(&pads), 4, "a retry archives nothing twice");
    until(
        "the restarted Door received the Handoff and archive pointer",
        || {
            model
                .journal()
                .iter()
                .any(|entry| {
                    entry["body"]["messages"]
                        .as_array()
                        .is_some_and(|messages| {
                            messages.iter().any(|m| {
                                m["role"] == "user"
                                    && m["content"].as_str().is_some_and(|t| {
                                        t.contains("Retry the Handoff.") && t.contains("archive-1")
                                    })
                            })
                        })
                })
                .then_some(())
        },
    )
    .await;
    idle(&client, &door).await;
    model.assert_matched();
}

#[tokio::test]
async fn a32_a_failed_restart_keeps_the_workstream() {
    let gate = Gate::new();
    let (project, mut client, door) = gated_clear_fixture("a32_failed_restart", &gate).await;
    let model = project.model.as_ref().unwrap();
    let a = child_of(&client, &door).await;
    let b = child_of(&client, &a).await;
    let before = rail_tree(&client)
        .await
        .into_iter()
        .find(|n| n.id == door)
        .unwrap();
    std::fs::remove_file(&gate.hold).unwrap();
    assert_eq!(
        client
            .request("workstream.clear", json!({"id": door, "reason": "bloat"}))
            .await
            .unwrap(),
        json!({"pending": true})
    );
    asked(model).await;
    as_door(&project, &door)
        .await
        .request(
            "workstream.handoff",
            json!({"id": door, "summary": "Build the login page.", "keep_todos": [], "keep_pads": []}),
        )
        .await
        .unwrap();
    let failed = next(&mut client, |event| match event {
        EventData::WorkstreamClearFailed(failed) if failed.id == door => Some(failed),
        _ => None,
    })
    .await;
    assert_eq!(
        failed.code,
        rpc::code::CONFLICT as i32,
        "{}",
        failed.message
    );

    let tree = rail_tree(&client).await;
    let after = tree.iter().find(|n| n.id == door).unwrap();
    assert_eq!(after.iteration, Some(1), "the Iteration stays");
    assert_eq!(after.terminal_id, None, "the Door stays stopped");
    assert_eq!(after.name, before.name);
    assert!(tree.iter().all(|n| n.id != a && n.id != b));

    std::fs::write(&gate.hold, "").unwrap();
    client
        .request("rail.startDoor", json!({"id": door}))
        .await
        .unwrap();
    let resumed = rail_tree(&client)
        .await
        .into_iter()
        .find(|n| n.id == door)
        .unwrap();
    assert!(resumed.terminal_id.is_some());
    assert_eq!(resumed.iteration, Some(1));
    idle(&client, &door).await;
    model.assert_matched();
}

#[tokio::test]
async fn q11_plugins_survive_real_daemon_launch_resume_and_workstream_clear() {
    let gate = Gate::new();
    let (project, mut client, door) = gated_clear_fixture("a32_plugins", &gate).await;
    let a = child_of(&client, &door).await;
    until("initial Door and children launched", || {
        (gate.rows().len() == 3).then_some(())
    })
    .await;
    for id in [&door, &a] {
        client
            .request("agent.stop", json!({"id": id}))
            .await
            .unwrap();
        client
            .request(
                if id == &door {
                    "rail.startDoor"
                } else {
                    "agent.resume"
                },
                json!({"id": id}),
            )
            .await
            .unwrap();
    }
    until("Door and child resumed", || {
        (gate.rows().len() == 5).then_some(())
    })
    .await;
    let before_clear = gate.rows();
    assert_eq!(
        client
            .request(
                "workstream.clear",
                json!({"id": door, "reason": "start a fresh iteration", "without_handoff": true})
            )
            .await
            .unwrap(),
        json!({"pending": true})
    );
    let cleared = next(&mut client, |event| match event {
        EventData::WorkstreamCleared(cleared) if cleared.id == door => Some(cleared),
        _ => None,
    })
    .await;
    assert_eq!(cleared.iteration, 2);
    until("fresh Door launched", || {
        (gate.rows().len() == 6).then_some(())
    })
    .await;
    let after_clear = gate.rows();
    let resumes = |row: &Value| {
        row["argv"]
            .as_array()
            .unwrap()
            .iter()
            .any(|arg| arg == "--resume")
    };
    assert!(resumes(&before_clear[3]) && resumes(&before_clear[4]));
    assert!(!resumes(&after_clear[5]));
    for row in &after_clear {
        let argv = row["argv"].as_array().unwrap();
        let paths: Vec<_> = argv
            .windows(2)
            .filter(|pair| pair[0] == "--plugin-dir")
            .map(|pair| std::path::Path::new(pair[1].as_str().unwrap()))
            .collect();
        // LC5: a Door is lean and gets the question plugin only; its children are workers.
        let bundled = [
            ("hope", "13.1.0"),
            ("hunch", "0.9.4"),
            ("hold", "0.0.9"),
            ("roundup-question", "0.0.1"),
        ];
        let wanted = if argv.iter().any(|arg| arg == "--tools") {
            &bundled[3..]
        } else {
            &bundled[..]
        };
        assert_eq!(paths.len(), wanted.len());
        for (path, &(name, version)) in paths.iter().zip(wanted) {
            let manifest: Value = serde_json::from_slice(
                &std::fs::read(path.join(".claude-plugin/plugin.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(manifest["name"], name);
            assert_eq!(manifest["version"], version);
        }
        assert_eq!(row["functionHooks"], "1");
    }
    idle(&client, &door).await;
    project.model.as_ref().unwrap().assert_matched();
}

#[tokio::test]
async fn h21_observer_a_door_allows_its_childs_permission_request_and_the_touch_names_it() {
    let project = daemon::start_real("h21_door_allows", &[]);
    let client = project.client().await;
    let door = workstream(&client, "team").await;
    client
        .request("rail.startDoor", json!({"id": door}))
        .await
        .unwrap();
    idle(&client, &door).await;
    client
        .request(
            "agent.spawn",
            json!({"cwd": project.dir.path(), "prompt": "H21 run ls", "parent": door}),
        )
        .await
        .unwrap();
    let model = project.model.as_ref().unwrap();
    // The Door answers from its Message, then the child's own Bash runs.
    model.tool_result("toolu_h21_allow").await;
    model.tool_result("toolu_h21_bash").await;
    assert!(project.dir.path().join("h21-made").is_dir());
    let touch = daemon::h21_poll("the Touch", async || {
        let history = client
            .request("provenance.history", json!({"item": "decision:d1"}))
            .await
            .unwrap();
        history.as_array().unwrap().first().cloned()
    })
    .await;
    assert_eq!(touch["verb"], "wrote");
    assert_eq!(touch["actor"]["kind"], "agent");
    assert_eq!(touch["actor"]["id"], door);
    let open = client.request("decision.list", Value::Null).await.unwrap();
    assert!(open.as_array().unwrap().iter().all(|d| d["id"] != "d1"));
    model.assert_matched();
}

async fn workstream(client: &rpc::Client, name: &str) -> String {
    client
        .request(
            "rail.createWorkstream",
            json!({"name": name, "parent": null}),
        )
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn h21_a_message_delivered_after_the_decision_cleared_is_still_typed() {
    let gate = Gate::new();
    std::fs::remove_file(&gate.hold).unwrap();
    let wrapper = gate.wrapper();
    let project = daemon::start_real("h21_typed_after_clear", &[("ROUNDUP_CLAUDE_BIN", &wrapper)]);
    let client = project.client().await;
    let door = workstream(&client, "team").await;
    client
        .request("rail.startDoor", json!({"id": door}))
        .await
        .unwrap();
    // B7 batches pending Messages. Put another note first so the Decision notice
    // cannot accidentally pass only because it heads the submitted prompt.
    // B50(e): a note from the user is typed on its own, so another sender's note is the one that
    // can be batched with the Decision notice.
    let sender = project.client().await;
    sender
        .request(
            "daemon.identify",
            json!({"actor": {"kind": "ext", "id": "h21"}}),
        )
        .await
        .unwrap();
    sender
        .request(
            "message.send",
            json!({"to": door, "kind": "note", "body": "queued before the Decision"}),
        )
        .await
        .unwrap();
    let child: contracts::agent::RailNode = serde_json::from_value(
        client
            .request(
                "agent.spawn",
                json!({"cwd": project.dir.path(), "prompt": null, "parent": door}),
            )
            .await
            .unwrap(),
    )
    .unwrap();
    let _open = daemon::h21_ask(&project, &child.id, "Bash", daemon::h21_bash()).await;
    let decision = daemon::h21_decision(&client, &child.id, None).await;
    let id = decision["id"].as_str().unwrap().to_owned();
    let notes = daemon::h21_notes(&client, &door, 1).await;
    let body = notes[0]["body"].as_str().unwrap().to_owned();
    assert_eq!(notes[0]["status"], "pending");

    daemon::h21_user_answers(&client, &id, "deny")
        .await
        .unwrap();
    let decisions = client.request("decision.list", Value::Null).await.unwrap();
    assert!(decisions.as_array().unwrap().iter().all(|d| d["id"] != id));
    assert!(gate.held.is_file(), "the Door's start is still held");
    std::fs::write(&gate.hold, "").unwrap();

    let model = project.model.as_ref().unwrap();
    let typed = until("the typed Decision notice", || {
        model.journal().iter().find_map(|entry| {
            entry["body"]["messages"]
                .as_array()?
                .iter()
                .filter(|m| m["role"] == "user")
                .filter_map(|m| m["content"].as_str())
                .find(|text| text.contains(&body))
                .map(str::to_owned)
        })
    })
    .await;
    let queued = typed.find("queued before the Decision").unwrap();
    assert!(queued < typed.find(&body).unwrap(), "{typed}");
}

#[tokio::test]
async fn h21_observer_the_childs_and_another_workstreams_agents_are_forbidden_and_the_user_wins() {
    // The child's own answer stays proved by `h21_an_agent_cannot_answer_its_own_decision` (rule 10).
    let project = daemon::start_real("h21_forbidden", &[]);
    let mut events = project.subscribed().await;
    let client = project.client().await;
    let door = workstream(&client, "team").await;
    client
        .request("rail.startDoor", json!({"id": door}))
        .await
        .unwrap();
    let elsewhere = workstream(&client, "elsewhere").await;
    let mut agents = Vec::new();
    for parent in [&door, &elsewhere] {
        let node: contracts::agent::RailNode = serde_json::from_value(
            client
                .request(
                    "agent.spawn",
                    json!({"cwd": project.dir.path(), "prompt": null, "parent": parent}),
                )
                .await
                .unwrap(),
        )
        .unwrap();
        agents.push(node.id);
    }
    let (child, other) = (agents[0].clone(), agents[1].clone());
    worked_then_idle(&mut events, &[&door, &child, &other]).await;
    let _open = daemon::h21_ask(&project, &child, "Bash", daemon::h21_bash()).await;
    assert_eq!(
        daemon::h21_decision(&client, &child, None).await["id"],
        "d1"
    );
    let say = |to: &str, body: &str| {
        client.request(
            "message.send",
            json!({"to": to, "kind": "note", "body": body}),
        )
    };
    let model = project.model.as_ref().unwrap();
    say(&other, "H21 other answers").await.unwrap();
    let refused = model.tool_result("toolu_h21_other").await;
    let refused: Value = serde_json::from_str(refused["content"].as_str().unwrap()).unwrap();
    assert_eq!(refused["code"], rpc::code::FORBIDDEN, "{refused}");
    let history = || client.request("provenance.history", json!({"item": "decision:d1"}));
    assert!(history().await.unwrap().as_array().unwrap().is_empty());
    assert_eq!(
        daemon::h21_decision(&client, &child, None).await["id"],
        "d1"
    );

    daemon::h21_user_answers(&client, "d1", "deny")
        .await
        .unwrap();
    say(&door, "H21 door answers late").await.unwrap();
    let late = model.tool_result("toolu_h21_late").await;
    let late: Value = serde_json::from_str(late["content"].as_str().unwrap()).unwrap();
    assert_eq!(late["code"], rpc::code::NOT_FOUND, "{late}");
    let touches = history().await.unwrap();
    assert_eq!(touches.as_array().unwrap().len(), 1);
    assert_eq!(touches[0]["actor"]["kind"], "user");
}

/// Each of `ids` has run a turn and stopped, so no later hook of its own can clear a Decision.
async fn worked_then_idle(events: &mut rpc::Client, ids: &[&String]) {
    let mut working = std::collections::BTreeSet::new();
    let mut idle = std::collections::BTreeSet::new();
    while idle.len() < ids.len() {
        let status = next(events, |event| match event {
            EventData::AgentStatus(status) if ids.contains(&&status.id) => Some(status),
            _ => None,
        })
        .await;
        if status.status.kind == Kind::Working {
            working.insert(status.id.clone());
        }
        if status.status.kind == Kind::Idle && working.contains(&status.id) {
            idle.insert(status.id);
        }
    }
}

/// U516: what the webview sends for a selected Turn: a fenced Excerpt and the Turn's facts.
const U516_BODY: &str = "why?\n\n```\n$ ls\nfile\n```\n\ncommand: ls\ncwd: /work\nexit: 0";

/// The model requests with a user message holding `needle`.
fn requests_holding(model: &aimock::Model, needle: &str) -> usize {
    model
        .journal()
        .iter()
        .filter(|entry| user_texts(entry).iter().any(|text| text.contains(needle)))
        .count()
}

/// The text of each user message a model request carries, whole or in content blocks.
fn user_texts(entry: &Value) -> Vec<String> {
    entry["body"]["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|message| message["role"] == "user")
        .map(|message| match message["content"].as_array() {
            Some(blocks) => blocks
                .iter()
                .filter_map(|block| block["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            None => message["content"].as_str().unwrap_or_default().to_owned(),
        })
        .collect()
}

async fn started_door(client: &rpc::Client) -> String {
    let door = client
        .request(
            "rail.createWorkstream",
            json!({"name": "team", "parent": null}),
        )
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    client
        .request("rail.startDoor", json!({"id": door}))
        .await
        .unwrap();
    door
}

#[tokio::test]
async fn u516_a_working_agent_is_sent_nothing_until_idle() {
    let gate = Gate::new();
    std::fs::remove_file(&gate.hold).unwrap();
    let project = daemon::start_real("u516_excerpt", &[("ROUNDUP_CLAUDE_BIN", &gate.wrapper())]);
    let client = project.subscribed().await;
    let door = started_door(&client).await;
    until("the Door's launch is held", || {
        gate.held.exists().then_some(())
    })
    .await;
    client
        .request(
            "message.send",
            json!({"to": door, "kind": "note", "body": U516_BODY}),
        )
        .await
        .unwrap();

    let listed = client
        .request("message.list", json!({"to": door}))
        .await
        .unwrap();
    assert_eq!(listed[0]["status"], "pending");
    let model = project.model.as_ref().unwrap();
    assert!(
        model.journal().is_empty(),
        "nothing reaches the model while the Agent has not reached idle"
    );

    std::fs::write(&gate.hold, "").unwrap();
    until_async("the Excerpt reaches the model", || async {
        (requests_holding(model, "```\n$ ls\nfile\n```") > 0).then_some(())
    })
    .await;
    assert!(
        requests_holding(model, U516_BODY) > 0,
        "the whole body is typed: {:#?}",
        model.journal()
    );
    idle(&client, &door).await;
    model.assert_matched();
}

/// B55: the unique word of an old ask, and the Door whose Iteration was given it.
const QUOKKA: &str = "QUOKKA";
const B55_FIRST_PROMPT: &str = "Follow this instruction: Build the";

struct Fresh {
    project: daemon::Project,
    client: rpc::Client,
    gate: Gate,
    door: String,
}

impl Fresh {
    async fn start() -> Self {
        let gate = Gate::new();
        let project = daemon::start_real(
            "b55_clear_starts_fresh",
            &[
                ("ROUNDUP_CLAUDE_BIN", &gate.wrapper()),
                ("ROUNDUP_HANDOFF_BOUND_MS", "60000"),
            ],
        );
        let client = project.subscribed().await;
        let door = started_door(&client).await;
        let fresh = Self {
            project,
            client,
            gate,
            door,
        };
        fresh.settled().await;
        fresh
    }

    fn model(&self) -> &aimock::Model {
        self.project.model.as_ref().unwrap()
    }

    /// The Door is idle and no model request has arrived for 300 ms.
    async fn settled(&self) {
        let seen = std::cell::Cell::new((usize::MAX, std::time::Instant::now()));
        until_async("the Door stays idle", || async {
            let count = self.model().journal().len();
            let door_idle = rail_tree(&self.client).await.iter().any(|n| {
                n.id == self.door && n.status.as_ref().is_some_and(|s| s.kind == Kind::Idle)
            });
            if !door_idle || count != seen.get().0 {
                seen.set((count, std::time::Instant::now()));
                return None;
            }
            (seen.get().1.elapsed() > Duration::from_millis(300)).then_some(())
        })
        .await;
    }

    /// The old Iteration's ask: the word in a Message, a Todo and a Pad.
    async fn ask(&self, delivered: bool) {
        self.client
            .request(
                "message.send",
                json!({"to": self.door, "kind": "note", "body": format!("{QUOKKA} plan")}),
            )
            .await
            .unwrap();
        self.client
            .request("todo.create", json!({"title": format!("{QUOKKA} todo")}))
            .await
            .unwrap();
        self.client
            .request(
                "pad.create",
                json!({"name": "plan", "text": format!("{QUOKKA} pad")}),
            )
            .await
            .unwrap();
        if delivered {
            until_async("the note reaches the Door", || async {
                (requests_holding(self.model(), &format!("{QUOKKA} plan")) > 0).then_some(())
            })
            .await;
        }
        self.settled().await;
    }

    /// Clear, answer the Handoff as the Door, and return once the next session has its first prompt.
    async fn clear(&mut self, summary: &str) {
        self.client
            .request(
                "workstream.clear",
                json!({"id": self.door, "reason": "start over"}),
            )
            .await
            .unwrap();
        until_async("the Door is asked for the Handoff", || async {
            (requests_holding(self.model(), "Write the Handoff") > 0).then_some(())
        })
        .await;
        self.settled().await;
        let actor = self.project.client().await;
        actor
            .request(
                "daemon.identify",
                contracts::IdentifyParams {
                    actor: contracts::Actor {
                        kind: contracts::ActorKind::Agent,
                        id: self.door.clone(),
                        parent: None,
                    },
                },
            )
            .await
            .unwrap();
        actor
            .request(
                "workstream.handoff",
                json!({"id": self.door, "summary": summary, "keep_todos": [], "keep_pads": []}),
            )
            .await
            .unwrap();
        until_async("the next session has its first prompt", || async {
            (requests_holding(self.model(), B55_FIRST_PROMPT) > 0).then_some(())
        })
        .await;
        self.settled().await;
    }

    fn launches(&self) -> Vec<Value> {
        self.gate.rows()
    }

    /// Everything the second session was given: argv, the files argv names, and every model
    /// request from its first prompt on.
    fn second_session(&self) -> String {
        let mut record = String::new();
        for launch in self.launches().iter().skip(1) {
            record.push_str(&launch.to_string());
            for arg in launch["argv"].as_array().unwrap() {
                if let Ok(text) = std::fs::read_to_string(arg.as_str().unwrap()) {
                    record.push_str(&text);
                }
            }
        }
        let journal = self.model().journal();
        let start = journal
            .iter()
            .position(|entry| {
                user_texts(entry)
                    .iter()
                    .any(|text| text.contains(B55_FIRST_PROMPT))
            })
            .expect("the next session sent a request");
        for entry in &journal[start..] {
            record.push_str(&entry["body"].to_string());
        }
        record
    }

    fn word_free(&self) {
        let record = self.second_session();
        assert!(!record.contains(QUOKKA), "{record}");
    }

    fn assert_fresh_launch(&self) {
        assert!(
            !self.launches()[1]["argv"].to_string().contains("--resume"),
            "{:?}",
            self.launches()
        );
    }
}

#[tokio::test]
async fn b55_the_next_session_holds_no_word_of_the_old_ask() {
    let mut w = Fresh::start().await;
    w.ask(true).await;
    w.clear("Build the login page.").await;
    w.assert_fresh_launch();
    w.word_free();
    w.model().assert_matched();
}

#[tokio::test]
async fn b55_the_word_appears_only_inside_the_handoff_when_the_summary_has_it() {
    let mut w = Fresh::start().await;
    w.ask(true).await;
    w.clear(&format!("Build the {QUOKKA} page.")).await;
    w.assert_fresh_launch();
    let handoff = format!("Build the {QUOKKA} page.");
    let record = w.second_session();
    assert!(record.contains(&handoff), "{record}");
    assert!(
        !record.replace(&handoff, "").contains(QUOKKA),
        "the word is in the Handoff and nowhere else: {record}"
    );
    w.model().assert_matched();
}

#[tokio::test]
async fn b55_a_restart_and_start_door_still_holds_no_word() {
    let mut w = Fresh::start().await;
    w.ask(true).await;
    w.clear("Build the login page.").await;
    let claude = w.gate.wrapper();
    let home = w.project.dir.path().join("home");
    w.project.restart(&[
        ("ROUNDUP_CLAUDE_BIN", &claude),
        ("HOME", home.to_str().unwrap()),
        ("CLAUDE_CODE_OAUTH_TOKEN", ""),
    ]);
    w.client = w.project.subscribed().await;
    w.client
        .request("rail.startDoor", json!({"id": w.door}))
        .await
        .unwrap();
    until("the Door starts a third time", || {
        (w.launches().len() >= 3).then_some(())
    })
    .await;
    w.settled().await;
    w.word_free();
    w.model().assert_matched();
}

#[tokio::test]
async fn b55_pending_held_and_waiting_messages_of_the_old_iteration_are_not_replayed() {
    let mut w = Fresh::start().await;
    w.client
        .request(
            "route.set",
            json!({"from": "you", "to": w.door, "delivery": "ask-first"}),
        )
        .await
        .unwrap();
    w.ask(false).await;
    w.clear("Build the login page.").await;
    w.word_free();
    w.model().assert_matched();
}

#[tokio::test]
async fn b55_thirty_idle_edges_of_the_new_session_type_none_of_it() {
    let mut w = Fresh::start().await;
    w.ask(true).await;
    w.clear("Build the login page.").await;
    for _ in 0..30 {
        let attempt = rail_tree(&w.client)
            .await
            .into_iter()
            .find(|n| n.id == w.door)
            .unwrap()
            .attempt;
        w.client
            .request(
                "agent.signal",
                json!({"id": w.door, "attempt": attempt, "payload": {"hook_event_name": "Stop"}}),
            )
            .await
            .unwrap();
    }
    w.settled().await;
    w.word_free();
    w.model().assert_matched();
}
