//! D2 to D4: the whole Daemon, run as the App runs it, with a fake `claude` for every Agent.

#[path = "e2e/daemon.rs"]
mod daemon;

use std::process::Command;

use contracts::agent::RailNode;
use contracts::todo::Todo;
use contracts::{ActorKind, EventData, Kind, Touch};
use daemon::{Project, next, next_kind, next_status, start};
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

    let tree: Vec<RailNode> =
        serde_json::from_value(client.request("rail.tree", json!(null)).await.unwrap()).unwrap();

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

    for _ in 0..10 {
        project.spawn_agent(&client).await;
    }
    let mut idle = std::collections::HashSet::new();
    while idle.len() < 10 {
        let status = next_status(&mut client).await;
        if status.status.kind == Kind::Idle {
            idle.insert(status.id);
        }
    }

    let growth_kb = resident_kb(project.pid()).saturating_sub(before);
    println!("d4: the Daemon's resident memory grew by {growth_kb} kB with 10 idle Agents");
    assert!(growth_kb < BUDGET_MB * 1024);
}
