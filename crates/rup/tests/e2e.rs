//! D2 to D4, and H15: the whole Daemon, run as the App runs it, with a fake `claude` for every
//! Agent.

#[path = "e2e/daemon.rs"]
mod daemon;

use std::process::Command;
use std::time::{Duration, Instant};

use contracts::agent::RailNode;
use contracts::pad::Pad;
use contracts::todo::Todo;
use contracts::{Actor, ActorKind, EventData, Kind, Touch};
use daemon::{Project, TEN_IDLE_BOUND, next, next_kind, rail_tree, signal, start, wait_until_idle};
use serde_json::json;

/// Recorded Claude Code hook payloads, one of each event D2 plays.
const RECORDED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../spikes/hooks-state/log.run1.jsonl"
);

const D2_EVENTS: &str = "SessionStart,UserPromptSubmit,PermissionRequest,PostToolUse,Stop";

fn d2_project() -> Project {
    start(&[
        ("FAKE_CLAUDE_PAYLOADS", RECORDED),
        ("FAKE_CLAUDE_EVENTS", D2_EVENTS),
        ("FAKE_CLAUDE_PROMPT", "fix the refresh race"),
    ])
}

#[tokio::test]
async fn d2_status_follows_the_hooks_in_order() {
    let project = d2_project();
    let mut client = project.subscribed().await;

    project.spawn_agent(&client).await;

    let mut kinds = Vec::new();
    for _ in 0..5 {
        kinds.push(next_kind(&mut client).await);
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
}

#[tokio::test]
async fn d2_the_first_prompt_names_the_agent() {
    let project = d2_project();
    let mut client = project.subscribed().await;
    let agent = project.spawn_agent(&client).await;
    for _ in 0..5 {
        next_kind(&mut client).await;
    }

    let tree = rail_tree(&client).await;

    let named: Vec<_> = tree.iter().filter(|node| node.id == agent.id).collect();
    assert_eq!(named.len(), 1);
    assert_eq!(named[0].name, "fix-refresh-race");
}

/// A Daemon whose one Agent has called `todo_create`; the Todo exists once this returns.
async fn agent_created_a_todo() -> (Project, RailNode, Todo) {
    let tool = json!({ "name": "todo_create", "arguments": { "title": "split checkout" } });
    let project = start(&[("FAKE_CLAUDE_TOOL", &tool.to_string())]);
    let mut client = project.subscribed().await;
    let agent = project.spawn_agent(&client).await;
    let todo = next(&mut client, |data| match data {
        EventData::TodoCreated(todo) => Some(todo),
        _ => None,
    })
    .await;
    (project, agent, todo)
}

#[tokio::test]
async fn d3_the_todo_the_agent_created_is_listed() {
    let (project, _, todo) = agent_created_a_todo().await;

    let listed: Vec<Todo> = serde_json::from_value(
        project
            .client()
            .await
            .request("todo.list", json!(null))
            .await
            .unwrap(),
    )
    .unwrap();

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, todo.id);
    assert_eq!(listed[0].title, "split checkout");
}

#[tokio::test]
async fn d3_provenance_names_the_agent_that_created_the_todo() {
    let (project, agent, todo) = agent_created_a_todo().await;

    let history: Vec<Touch> = serde_json::from_value(
        project
            .client()
            .await
            .request(
                "provenance.history",
                json!({ "item": format!("todo:{}", todo.id) }),
            )
            .await
            .unwrap(),
    )
    .unwrap();

    assert_eq!(history.len(), 1);
    assert_eq!(history[0].actor.kind, ActorKind::Agent);
    assert_eq!(history[0].actor.id, agent.id);
}

/// A16: `rail.remove` has no `pads` or `todos` dependency in `crates/agents`, so this proves it
/// against a real Daemon instead. Removing an Agent that owns a Pad and has made a Todo must
/// leave both exactly as they were; the user still reclaims the Pad by `pad.setOwner` (U18).
#[tokio::test]
async fn a16_removing_an_agent_leaves_its_pads_and_todos_untouched() {
    let (project, agent, _todo) = agent_created_a_todo().await;
    let client = project.client().await;
    client
        .request("pad.create", json!({"name": "notes", "text": null}))
        .await
        .unwrap();
    let owner = Actor {
        kind: ActorKind::Agent,
        id: agent.id.clone(),
        parent: None,
    };
    client
        .request("pad.setOwner", json!({"name": "notes", "owner": owner}))
        .await
        .unwrap();
    let todos_before: Vec<Todo> =
        serde_json::from_value(client.request("todo.list", json!(null)).await.unwrap()).unwrap();
    let pads_before: Vec<Pad> =
        serde_json::from_value(client.request("pad.list", json!(null)).await.unwrap()).unwrap();

    client
        .request("rail.remove", json!({"id": agent.id}))
        .await
        .unwrap();

    let todos_after: Vec<Todo> =
        serde_json::from_value(client.request("todo.list", json!(null)).await.unwrap()).unwrap();
    assert_eq!(todos_after, todos_before);
    let pads_after: Vec<Pad> =
        serde_json::from_value(client.request("pad.list", json!(null)).await.unwrap()).unwrap();
    assert_eq!(pads_after, pads_before);

    let reclaimed: Pad = serde_json::from_value(
        client
            .request(
                "pad.setOwner",
                json!({"name": "notes", "owner": Actor::user()}),
            )
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(reclaimed.owner, Actor::user());
}

/// How much memory the Daemon allows its own share of ten idle Agents (rule 7).
const BUDGET_MB: u64 = 150;

fn resident_kb(pid: u32) -> u64 {
    let ps = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .unwrap();
    String::from_utf8(ps.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap()
}

#[tokio::test]
async fn d4_ten_idle_agents_cost_the_daemon_less_than_150_mb() {
    let project = start(&[
        ("FAKE_CLAUDE_PAYLOADS", RECORDED),
        ("FAKE_CLAUDE_EVENTS", "SessionStart"),
    ]);
    let mut client = project.subscribed().await;
    let before = resident_kb(project.pid());

    // One deadline for all ten Agents together, from the first spawn (scenario D4): moving this
    // inside the loop would reset it on every spawn and let a slow first Agent dodge the bound.
    let deadline = Instant::now() + TEN_IDLE_BOUND;
    let mut ids = Vec::new();
    for _ in 0..10 {
        ids.push(project.spawn_agent(&client).await.id);
    }
    wait_until_idle(&mut client, &ids, deadline)
        .await
        .unwrap_or_else(|report| panic!("{report}"));

    let growth_kb = resident_kb(project.pid()).saturating_sub(before);
    println!("d4: the Daemon's resident memory grew by {growth_kb} kB with 10 idle Agents");
    assert!(growth_kb < BUDGET_MB * 1024);
}

/// A test that passes by waiting longer is not a pass (scenario D4): the wait must return once
/// every watched Agent is Idle, not sit until the deadline.
#[tokio::test]
async fn d4_wait_returns_once_every_watched_agent_is_idle_not_at_the_deadline() {
    let project = start(&[]);
    let mut client = project.subscribed().await;
    let mut ids = Vec::new();
    for _ in 0..3 {
        let agent = project.spawn_agent(&client).await;
        signal(
            &client,
            &agent.id,
            json!({ "hook_event_name": "SessionStart" }),
        )
        .await;
        ids.push(agent.id);
    }

    let started = Instant::now();
    wait_until_idle(&mut client, &ids, started + TEN_IDLE_BOUND)
        .await
        .unwrap();

    let elapsed = started.elapsed();
    assert!(
        elapsed < TEN_IDLE_BOUND / 3,
        "wait_until_idle took {elapsed:?} to return once every Agent was Idle, \
         with a deadline of {TEN_IDLE_BOUND:?} still ahead of it"
    );
}

/// Scenario D4's failure output: a miss names each Agent not yet Idle, the status last seen on
/// the event stream and the Daemon's own `rail.tree` view of it.
#[tokio::test]
async fn d4_a_miss_names_the_not_idle_agents_their_last_status_and_the_rail_tree() {
    let project = start(&[]);
    let mut client = project.subscribed().await;
    let agent = project.spawn_agent(&client).await;
    // A Signal that does arrive, so "last seen" is never trivially None; it never reaches Idle.
    signal(
        &client,
        &agent.id,
        json!({ "hook_event_name": "UserPromptSubmit" }),
    )
    .await;

    let report = wait_until_idle(
        &mut client,
        std::slice::from_ref(&agent.id),
        Instant::now() + Duration::from_millis(200),
    )
    .await
    .unwrap_err();

    let line = report
        .lines()
        .find(|line| line.starts_with(&format!("  {}:", agent.id)))
        .unwrap_or_else(|| panic!("no line for {}: {report}", agent.id));
    assert!(line.contains("last saw Some("), "{line}");
    assert!(
        line.contains("rail.tree shows Some(Status { kind: Working"),
        "{line}"
    );
    assert!(
        !line.contains("(Idle in the tree but not seen on the stream)"),
        "{line}"
    );
}

/// The `ids` guard: a Status for an Agent nobody is waiting on must not count toward the wait.
#[tokio::test]
async fn d4_an_agent_not_in_ids_does_not_count_toward_idle() {
    let project = start(&[]);
    let mut client = project.subscribed().await;
    let watched = project.spawn_agent(&client).await;
    let other = project.spawn_agent(&client).await;
    // `other` reaches Idle quickly; only `watched` is in the `ids` this wait names.
    signal(
        &client,
        &other.id,
        json!({ "hook_event_name": "SessionStart" }),
    )
    .await;

    let report = wait_until_idle(
        &mut client,
        std::slice::from_ref(&watched.id),
        Instant::now() + Duration::from_millis(300),
    )
    .await
    .unwrap_err();

    assert!(
        report.starts_with("1 of 1 Agents were not Idle"),
        "{report}"
    );
    assert!(report.contains(&format!("  {}:", watched.id)), "{report}");
}

/// The not-Idle filter: an Agent that already reached Idle must not get a line of its own.
#[tokio::test]
async fn d4_a_miss_report_omits_agents_already_idle() {
    let project = start(&[]);
    let mut client = project.subscribed().await;
    let pending = project.spawn_agent(&client).await;
    let done = project.spawn_agent(&client).await;
    signal(
        &client,
        &done.id,
        json!({ "hook_event_name": "SessionStart" }),
    )
    .await;

    let report = wait_until_idle(
        &mut client,
        &[pending.id.clone(), done.id.clone()],
        Instant::now() + Duration::from_millis(300),
    )
    .await
    .unwrap_err();

    assert!(
        report.starts_with("1 of 2 Agents were not Idle"),
        "{report}"
    );
    assert!(report.contains(&format!("  {}:", pending.id)), "{report}");
    assert!(!report.contains(&format!("  {}:", done.id)), "{report}");
}

/// A Status the Daemon already holds but this client never saw (it never subscribed) is told
/// apart from a Daemon that is merely slow to reach the watched Agent.
#[tokio::test]
async fn d4_an_event_not_seen_on_the_stream_is_told_from_a_slow_daemon() {
    let project = start(&[]);
    let client = project.client().await;
    let agent = project.spawn_agent(&client).await;
    signal(
        &client,
        &agent.id,
        json!({ "hook_event_name": "SessionStart" }),
    )
    .await;

    // Never subscribed, so it can never see the Idle event, however long it waits.
    let mut unsubscribed = project.client().await;
    let report = wait_until_idle(
        &mut unsubscribed,
        std::slice::from_ref(&agent.id),
        Instant::now() + Duration::from_millis(200),
    )
    .await
    .unwrap_err();

    assert!(report.contains("last saw None"), "{report}");
    assert!(
        report.contains("rail.tree shows Some(Status { kind: Idle"),
        "{report}"
    );
    assert!(
        report.contains("(Idle in the tree but not seen on the stream)"),
        "{report}"
    );
}

/// A connection that closes mid-wait is reported as closed, not folded into the usual report
/// (which would need `rail.tree` on a connection that can no longer answer).
#[tokio::test]
async fn d4_a_miss_tells_a_closed_connection_from_a_slow_daemon() {
    let project = start(&[]);
    let mut client = project.subscribed().await;
    let agent = project.spawn_agent(&client).await;
    drop(project);

    let report = wait_until_idle(
        &mut client,
        std::slice::from_ref(&agent.id),
        Instant::now() + Duration::from_secs(5),
    )
    .await
    .unwrap_err();

    assert!(report.contains("connection closed"), "{report}");
}

/// H15: a loop of 100 tool uses, with the real `rup` hook command registered in the settings file
/// `agent.spawn` writes, trying the hook for `PreToolUse` and `PostToolUse` around each use. The
/// fake `claude` skips an event with no entry in the settings, exactly as Claude Code itself would;
/// each hook command that does run is `rup signal`, so a command that exits 0 proves only that
/// the RPC call answered, not that the Daemon acted on it (a Daemon whose `agent.signal` handler
/// dropped every Signal on the floor would still make `rup signal` exit 0). The Daemon must see
/// exactly the `STATE_EVENTS` Signals, both `PreToolUse` and `PostToolUse` around every tool use,
/// and must actually fold at least one into the Agent's Status: `rail.tree` is read straight from
/// the Daemon's own Runs, not from anything the fake `claude` reports about itself.
#[tokio::test]
async fn h15_the_daemon_sees_exactly_the_signals_of_state_events_from_a_hundred_tool_use_loop() {
    let report = tempfile::NamedTempFile::new().unwrap();
    let report_path = report.path().to_string_lossy().into_owned();
    let project = start(&[
        ("FAKE_CLAUDE_LOOP", "100"),
        ("FAKE_CLAUDE_LOOP_REPORT", &report_path),
    ]);
    let client = project.client().await;
    let agent = project.spawn_agent(&client).await;

    let deadline = Instant::now() + Duration::from_secs(30);
    let ran: Vec<String> = loop {
        if let Ok(text) = std::fs::read_to_string(report.path())
            && let Ok(ran) = serde_json::from_str::<Vec<String>>(&text)
        {
            break ran;
        }
        assert!(Instant::now() < deadline, "the loop never finished");
        tokio::time::sleep(Duration::from_millis(20)).await;
    };

    assert_eq!(ran.len(), 200, "{ran:?}");
    assert!(
        ran.chunks(2)
            .all(|pair| pair == ["PreToolUse", "PostToolUse"]),
        "{ran:?}"
    );

    let tree = rail_tree(&client).await;
    let node = tree.iter().find(|node| node.id == agent.id).unwrap();
    assert_eq!(
        node.status.as_ref().map(|status| status.label.as_str()),
        Some("working"),
        "the Daemon's own Status for the Agent never left `starting`, so it cannot have observed \
         a Signal from the loop: {:?}",
        node.status
    );
}

/// H15's last clause: once a Signal reaches the Daemon, the Kind it folds to must be visible to a
/// subscribed client within rule 7's keystroke-to-render budget, the same local round trip a
/// Signal and its resulting Status event both make.
#[tokio::test]
async fn h15_the_kind_flips_within_the_keystroke_to_render_budget_after_the_signal_arrives() {
    let project = start(&[]);
    let mut client = project.subscribed().await;
    let agent = project.spawn_agent(&client).await;

    let started = Instant::now();
    signal(&client, &agent.id, json!({"hook_event_name": "Stop"})).await;
    let kind = next_kind(&mut client).await;
    let elapsed = started.elapsed();

    assert_eq!(kind, Kind::Idle);
    assert!(elapsed < Duration::from_millis(16), "{elapsed:?}");
}
