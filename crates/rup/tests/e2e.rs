//! D2 to D4, and H15: the whole Daemon, run as the App runs it, with a fake `claude` for every
//! Agent.

#[path = "e2e/daemon.rs"]
mod daemon;

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use contracts::agent::{Channel, Context, ParentNode, RailNode};
use contracts::pad::Pad;
use contracts::todo::Todo;
use contracts::{Actor, ActorKind, EventData, IdentifyParams, Kind, Touch};
use daemon::{Project, TEN_IDLE_BOUND, next, next_kind, rail_tree, signal, start, wait_until_idle};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Recorded Claude Code hook payloads, one of each event D2 plays.
const RECORDED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../spikes/hooks-state/log.run1.jsonl"
);

const D2_EVENTS: &str = "SessionStart,UserPromptSubmit,PermissionRequest,PostToolUse,Stop";

/// The fake `claude` leaves its `rup permission` open and waits for this file, which the test
/// writes once the Agent is `needs-you` (H7a: the next Signal clears the Decision).
fn d2_project() -> (Project, std::path::PathBuf) {
    let gate = std::env::temp_dir().join(format!(
        "d2-gate-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_file(&gate);
    let project = start(&[
        ("FAKE_CLAUDE_PAYLOADS", RECORDED),
        ("FAKE_CLAUDE_EVENTS", D2_EVENTS),
        ("FAKE_CLAUDE_PROMPT", "fix the refresh race"),
        ("FAKE_CLAUDE_GATE", gate.to_str().unwrap()),
    ]);
    (project, gate)
}

/// The next five Kinds, opening the gate once the third (`needs-you`) is in.
async fn d2_kinds(client: &mut rpc::Client, gate: &std::path::Path) -> Vec<Kind> {
    let mut kinds = Vec::new();
    for _ in 0..5 {
        kinds.push(next_kind(client).await);
        if kinds.len() == 3 {
            std::fs::write(gate, "").unwrap();
        }
    }
    kinds
}

#[tokio::test]
async fn d2_status_follows_the_hooks_in_order() {
    let (project, gate) = d2_project();
    let mut client = project.subscribed().await;

    project.spawn_agent(&client).await;

    let kinds = d2_kinds(&mut client, &gate).await;
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
    let (project, gate) = d2_project();
    let mut client = project.subscribed().await;
    let agent = project.spawn_agent(&client).await;
    d2_kinds(&mut client, &gate).await;

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

/// T8: `rail.remove` (A16) does not know `creator`, since `crates/agents` has no `todos` dependency;
/// the Todo the removed Agent made must still read with that Agent as its creator.
#[tokio::test]
async fn t8_creator_survives_removing_the_agent_that_made_the_todo() {
    let (project, agent, todo) = agent_created_a_todo().await;
    let client = project.client().await;
    let before: Todo = serde_json::from_value(
        client
            .request("todo.get", json!({"id": todo.id}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(before.creator.kind, ActorKind::Agent);
    assert_eq!(before.creator.id, agent.id);

    client
        .request("rail.remove", json!({"id": agent.id}))
        .await
        .unwrap();

    let after: Todo = serde_json::from_value(
        client
            .request("todo.get", json!({"id": todo.id}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(after.creator, before.creator);
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

#[tokio::test]
async fn g1_the_worktrees_setting_is_set_and_read_through_the_daemon() {
    let project = start(&[]);
    let client = project.client().await;

    client
        .request(
            "project.setWorktrees",
            json!({"on": true, "check": "just check"}),
        )
        .await
        .unwrap();
    let settings = client.request("project.get", json!(null)).await.unwrap();

    assert_eq!(
        settings,
        json!({"worktrees": {"on": true, "check": "just check"}})
    );
}

/// H15: a loop of 100 tool uses, with the real `rup` hook command registered in the settings file
/// `agent.spawn` writes, opened by a `UserPromptSubmit`, trying the hook for `PreToolUse` and
/// `PostToolUse` around each use, then a final `Stop`. The fake `claude` skips an event with no
/// entry in the settings, exactly as Claude Code itself would, and fails the loop if any hook
/// command exits non-zero, so a finished loop means every `rup signal` answered. That proves only
/// that the RPC call answered: a Daemon whose `agent.signal` handler dropped every Signal would
/// still make `rup signal` exit 0, so a subscribed client must also see the Kind go `working` and
/// then `idle`, and `rail.tree`, read straight from the Daemon's own Runs, must say `idle`.
#[tokio::test]
async fn h15_every_hook_call_of_a_hundred_tool_use_loop_is_answered_and_the_final_stop_leaves_the_agent_idle()
 {
    let report = tempfile::NamedTempFile::new().unwrap();
    let report_path = report.path().to_string_lossy().into_owned();
    let project = start(&[
        ("FAKE_CLAUDE_LOOP", "100"),
        ("FAKE_CLAUDE_LOOP_REPORT", &report_path),
    ]);
    let mut client = project.subscribed().await;
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

    let mut seen_working = false;
    loop {
        match next_kind(&mut client).await {
            Kind::Working => seen_working = true,
            Kind::Idle if seen_working => break,
            _ => {}
        }
    }

    let tree = rail_tree(&client).await;
    let node = tree.iter().find(|node| node.id == agent.id).unwrap();
    assert_eq!(
        node.status.as_ref().map(|status| status.label.as_str()),
        Some("idle"),
        "the Daemon's own Status for the Agent is not `idle` after the loop's final Stop, so it \
         did not act on the Signals: {:?}",
        node.status
    );
}

/// H15's last clause: a Signal that changes the Kind reaches a subscribed client as a Status event,
/// within the bound every e2e wait uses. The keystroke-to-render budget itself is rule 7's
/// `just perf-keystroke`; a wall-clock assertion of it here would only measure the CI runner.
#[tokio::test]
async fn h15_a_signal_reaches_a_subscribed_client_as_a_status_event() {
    let project = start(&[]);
    let mut client = project.subscribed().await;
    let agent = project.spawn_agent(&client).await;

    signal(&client, &agent.id, json!({"hook_event_name": "Stop"})).await;

    assert_eq!(next_kind(&mut client).await, Kind::Idle);
}

fn git_at(project: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(project)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

async fn interrupted_worktree(workstream: bool) {
    use std::os::unix::fs::PermissionsExt;
    let wrapper = tempfile::tempdir().unwrap();
    let real_git = String::from_utf8(
        Command::new("sh")
            .args(["-c", "command -v git"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let marker = wrapper.path().join("wrapper-pid");
    let fifo = wrapper.path().join("hold");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let script = format!(
        "#!/bin/sh\nREAL='{}'\nif [ \"$1\" = worktree ] && [ \"$2\" = add ]; then\n \"$REAL\" \"$@\" || exit $?\n echo $$ > '{}'\n read release < '{}'\nelse exec \"$REAL\" \"$@\"; fi\n",
        real_git.trim(),
        marker.display(),
        fifo.display()
    );
    let executable = wrapper.path().join("git");
    std::fs::write(&executable, script).unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        wrapper.path().display(),
        std::env::var("PATH").unwrap()
    );
    let mut project = start(&[("PATH", &path)]);
    let root = project.dir.path();
    git_at(root, &["init", "-q", "-b", "main"]);
    git_at(root, &["config", "user.name", "roundup"]);
    git_at(root, &["config", "user.email", "roundup@example.com"]);
    std::fs::write(root.join("README"), "keep").unwrap();
    git_at(root, &["add", "README"]);
    git_at(root, &["commit", "-qm", "base"]);
    let unrelated = wrapper.path().join("user-worktree");
    git_at(
        root,
        &[
            "worktree",
            "add",
            "-b",
            "user-owned",
            unrelated.to_str().unwrap(),
        ],
    );
    let before = (
        git_at(root, &["branch", "--list"]),
        git_at(root, &["worktree", "list"]),
    );
    let client = project.client().await;
    let ids = if workstream {
        let parent = client
            .request(
                "rail.createWorkstream",
                json!({"name":"workstream","parent":null}),
            )
            .await
            .unwrap();
        let child = client
            .request(
                "agent.spawn",
                json!({"cwd": project.dir.path(), "prompt": null, "parent": parent["id"]}),
            )
            .await
            .unwrap();
        Some((parent["id"].clone(), child["id"].clone()))
    } else {
        None
    };
    client
        .request("project.setWorktrees", json!({"on":true,"check":null}))
        .await
        .unwrap();
    let pending = if let Some((id, _)) = &ids {
        let id = id.clone();
        tokio::spawn(async move { client.request("rail.startDoor", json!({"id":id})).await })
    } else {
        let cwd = root.to_str().unwrap().to_owned();
        tokio::spawn(async move {
            client
                .request(
                    "agent.spawn",
                    json!({"cwd":cwd,"parent":null,"prompt":null}),
                )
                .await
        })
    };
    let pid = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(pid) = std::fs::read_to_string(&marker) {
                break pid.trim().parse::<u32>().unwrap();
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    project.crash();
    assert!(
        Command::new("kill")
            .args(["-9", &pid.to_string()])
            .status()
            .unwrap()
            .success()
    );
    let _ = pending.await;
    project.restart(&[]);
    let root = project.dir.path();
    assert_eq!(
        (
            git_at(root, &["branch", "--list"]),
            git_at(root, &["worktree", "list"])
        ),
        before
    );
    assert!(git_at(root, &["for-each-ref", "refs/roundup/provisioning/"]).is_empty());
    let client = project.client().await;
    let nodes = rail_tree(&client).await;
    if let Some((parent, child)) = ids {
        assert_eq!(nodes.len(), 2);
        assert!(
            nodes
                .iter()
                .all(|n| n.terminal_id.is_none() && n.worktree.is_none())
        );
        assert_eq!(
            nodes
                .iter()
                .find(|n| json!(n.id) == child)
                .unwrap()
                .parent
                .as_deref(),
            parent.as_str()
        );
    } else {
        assert!(nodes.is_empty());
    }
    assert!(unrelated.join("README").exists());
}

#[tokio::test]
async fn g6_crashed_agent_provision_is_recovered_without_touching_user_worktree() {
    interrupted_worktree(false).await;
}

#[tokio::test]
async fn g6_crashed_door_provision_retains_workstream_and_children() {
    interrupted_worktree(true).await;
}

/// What the fake `claude` read from its argv and its `rup mcp`: the Brief file and the tool list.
struct Seen {
    brief: String,
    tools: Vec<String>,
}

async fn seen_at(report: &std::path::Path) -> Seen {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Ok(text) = std::fs::read_to_string(report)
                && let Ok(seen) = serde_json::from_str::<serde_json::Value>(&text)
            {
                return Seen {
                    brief: seen["brief"].as_str().unwrap().to_owned(),
                    tools: seen["tools"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|name| name.as_str().unwrap().to_owned())
                        .collect(),
                };
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the fake claude reported")
}

/// The tool names a Brief puts in backticks.
fn advertised(brief: &str) -> Vec<String> {
    let mut names: Vec<String> = brief
        .lines()
        .filter_map(|line| line.strip_prefix("- `")?.split('`').next())
        .map(str::to_owned)
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn e1_an_agents_brief_advertises_exactly_the_tools_its_rup_mcp_lists() {
    let report = tempfile::tempdir().unwrap().keep().join("seen.json");
    let project = start(&[("FAKE_CLAUDE_BRIEF_REPORT", report.to_str().unwrap())]);
    let client = project.client().await;
    let agent = project.spawn_agent(&client).await;

    let seen = seen_at(&report).await;

    let mut listed = seen.tools.clone();
    listed.sort();
    assert_eq!(advertised(&seen.brief), listed);
    assert!(
        seen.brief
            .contains(&format!("Your Agent id is {}.", agent.id))
    );
    assert!(!seen.brief.contains("coordinating Door"));
}

#[tokio::test]
async fn b24_a_door_reads_its_brief_then_creates_and_updates_a_todo_over_mcp() {
    let report = tempfile::tempdir().unwrap().keep().join("seen.json");
    let tools = json!([
        { "name": "todo_create", "arguments": { "title": "plan the workstream" } },
        { "name": "todo_update", "arguments": { "id": "$id", "body": "owned files: none yet" } },
    ]);
    let project = start(&[
        ("FAKE_CLAUDE_BRIEF_REPORT", report.to_str().unwrap()),
        ("FAKE_CLAUDE_TOOLS", &tools.to_string()),
    ]);
    let mut client = project.subscribed().await;
    let workstream = client
        .request(
            "rail.createWorkstream",
            json!({"name": "workstream", "parent": null}),
        )
        .await
        .unwrap();
    let door = workstream["id"].as_str().unwrap().to_owned();
    client
        .request("rail.startDoor", json!({ "id": door }))
        .await
        .unwrap();

    let updated = next(&mut client, |data| match data {
        EventData::TodoUpdated(todo) if !todo.body.is_empty() => Some(todo),
        _ => None,
    })
    .await;
    let seen = seen_at(&report).await;

    assert!(seen.brief.contains("coordinating Door"));
    assert!(seen.brief.contains(&format!("Your Agent id is {door}.")));
    let mut listed = seen.tools;
    listed.sort();
    assert_eq!(advertised(&seen.brief), listed);
    assert_eq!(updated.title, "plan the workstream");
    assert_eq!(updated.body, "owned files: none yet");
    assert_eq!(
        updated.creator,
        Actor {
            kind: ActorKind::Agent,
            id: door,
            parent: None
        }
    );
    let stored: Vec<Todo> = serde_json::from_value(
        project
            .client()
            .await
            .request("todo.list", json!(null))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(
        (&stored[0].title, &stored[0].body),
        (&updated.title, &updated.body)
    );
}

// E2 to E6: an Agent is told where it stands and asks again.

const DEADLINE: (&str, &str) = ("ROUNDUP_CHANNEL_DEADLINE_MS", "600");

fn socket(project: &Project) -> PathBuf {
    project.dir.path().join("rupd.sock")
}

fn actor(kind: ActorKind, id: &str) -> Actor {
    Actor {
        kind,
        id: id.into(),
        parent: None,
    }
}

async fn client_as(project: &Project, who: Actor) -> rpc::Client {
    let client = project.client().await;
    client
        .request("daemon.identify", IdentifyParams { actor: who })
        .await
        .unwrap();
    client
}

async fn workstream(client: &rpc::Client, name: &str) -> String {
    let workstream = client
        .request(
            "rail.createWorkstream",
            json!({ "name": name, "parent": null }),
        )
        .await
        .unwrap();
    workstream["id"].as_str().unwrap().to_owned()
}

async fn agent_in(project: &Project, client: &rpc::Client, parent: Option<&str>) -> String {
    let node = client
        .request(
            "agent.spawn",
            json!({ "cwd": project.dir.path(), "prompt": null, "parent": parent }),
        )
        .await
        .unwrap();
    serde_json::from_value::<RailNode>(node).unwrap().id
}

/// The id of the Agent that Agent `caller` starts (F3, A26).
async fn agent_in_as(project: &Project, caller: &str) -> String {
    let as_caller = client_as(project, actor(ActorKind::Agent, caller)).await;
    let node = as_caller
        .request(
            "agent.spawn",
            json!({ "cwd": project.dir.path(), "prompt": null, "parent": null }),
        )
        .await
        .unwrap();
    serde_json::from_value::<RailNode>(node).unwrap().id
}

/// `a` and `b` under the Door `m`, and `c` under the plain Group `g`.
struct Rail {
    m: String,
    a: String,
    b: String,
    g: String,
    c: String,
}

async fn rail(project: &Project, client: &rpc::Client) -> Rail {
    let m = workstream(client, "m").await;
    client
        .request("rail.startDoor", json!({ "id": m }))
        .await
        .unwrap();
    let a = agent_in(project, client, Some(&m)).await;
    let b = agent_in(project, client, Some(&m)).await;
    let g = workstream(client, "g").await;
    let c = agent_in(project, client, Some(&g)).await;
    Rail { m, a, b, g, c }
}

async fn context_of(client: &rpc::Client, id: &str) -> Result<Context, rpc::RpcError> {
    let reply = client.request("agent.context", json!({ "id": id })).await?;
    Ok(serde_json::from_value(reply).unwrap())
}

async fn digest_of(client: &rpc::Client, id: &str) -> Result<Value, rpc::RpcError> {
    client.request("agent.digest", json!({ "id": id })).await
}

#[tokio::test]
async fn b20_a_door_and_the_user_ask_for_its_children_and_no_one_else_does() {
    let project = start(&[]);
    let user = project.client().await;
    let rail = rail(&project, &user).await;
    let as_m = client_as(&project, actor(ActorKind::Agent, &rail.m)).await;
    let as_a = client_as(&project, actor(ActorKind::Agent, &rail.a)).await;

    let asked = digest_of(&as_m, &rail.m).await.unwrap();
    assert_eq!(digest_of(&user, &rail.m).await.unwrap(), asked);
    let children = asked["children"].as_array().unwrap();
    assert_eq!(children.len(), 2);
    for child in children {
        let mut keys: Vec<_> = child.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["kind", "last", "name", "pads", "todos"]);
    }

    let code = |result: Result<Value, rpc::RpcError>| result.unwrap_err().code;
    assert_eq!(code(digest_of(&as_a, &rail.m).await), rpc::code::FORBIDDEN);
    assert_eq!(code(digest_of(&user, "nobody").await), rpc::code::NOT_FOUND);
    assert_eq!(code(digest_of(&user, &rail.a).await), rpc::code::CONFLICT);
}

#[tokio::test]
async fn b19_a_childs_pads_reach_the_door_once_and_terminal_output_never() {
    let project = start(&[]);
    let user = project.client().await;
    let rail = rail(&project, &user).await;
    let as_a = client_as(&project, actor(ActorKind::Agent, &rail.a)).await;
    as_a.request("pad.create", json!({ "name": "notes", "text": "ZEBRA" }))
        .await
        .unwrap();
    let as_m = client_as(&project, actor(ActorKind::Agent, &rail.m)).await;

    // The change is pushed to `m` (B21), which is an envelope about the child, so it names the
    // Pad and no later envelope does.
    let (_, pushed) = push_where(&user, &rail.m, |_, e| e["pads"] == json!(["notes"])).await;
    let by_user = digest_of(&user, &rail.m).await.unwrap();
    let first = digest_of(&as_m, &rail.m).await.unwrap();

    assert_eq!(pushed["name"], by_user["children"][0]["name"]);
    assert_eq!(by_user["children"][0]["pads"], json!([]));
    assert_eq!(first["children"][0]["pads"], json!([]));
    assert!(!pushed.to_string().contains("ZEBRA"));
    assert!(!first.to_string().contains("ZEBRA"));
}

/// The Messages the Daemon sent to `to`, each with its pushed entry.
async fn digest_pushes(client: &rpc::Client, to: &str) -> Vec<(Value, Value)> {
    let listed = client
        .request("message.list", json!({ "to": to }))
        .await
        .unwrap();
    listed
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| message["from"]["id"] == "rupd")
        .map(|message| {
            let body = message["body"].as_str().unwrap();
            let entry = body
                .strip_prefix("[digest] ")
                .expect("body is [digest] and an entry");
            (message.clone(), serde_json::from_str(entry).unwrap())
        })
        .collect()
}

/// The pushes to `to` once one satisfies `want`; fails after five seconds.
async fn push_where(
    client: &rpc::Client,
    to: &str,
    want: impl Fn(&Value, &Value) -> bool,
) -> (Value, Value) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let pushes = digest_pushes(client, to).await;
        if let Some(found) = pushes.iter().rev().find(|(m, e)| want(m, e)) {
            return found.clone();
        }
        assert!(Instant::now() < deadline, "no such push yet: {pushes:?}");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

/// Everything the Daemon has to say about the Rail so far has been said.
async fn settled(client: &rpc::Client, to: &str) -> usize {
    let mut seen = usize::MAX;
    loop {
        tokio::time::sleep(Duration::from_millis(300)).await;
        let now = digest_pushes(client, to).await.len();
        if now == seen {
            return now;
        }
        seen = now;
    }
}

#[tokio::test]
async fn b21_a_childs_kind_pad_and_todo_changes_each_push_one_entry_to_its_door() {
    let project = start(&[]);
    let user = project.client().await;
    let rail = rail(&project, &user).await;
    let a_name = rail_tree(&user)
        .await
        .into_iter()
        .find(|node| node.id == rail.a)
        .unwrap()
        .name;
    settled(&user, &rail.m).await;

    // A Kind change.
    signal(&user, &rail.a, json!({"hook_event_name": "Stop"})).await;
    let (message, entry) = push_where(&user, &rail.m, |_, e| {
        e["name"] == a_name.as_str() && e["kind"] == "idle"
    })
    .await;
    assert_eq!(message["kind"], "note");
    assert_eq!(message["from"]["id"], "rupd");
    let mut keys: Vec<_> = entry.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["kind", "last", "name", "pads", "todos"]);

    // A Pad.
    let as_a = client_as(&project, actor(ActorKind::Agent, &rail.a)).await;
    as_a.request("pad.create", json!({ "name": "notes", "text": "ZEBRA" }))
        .await
        .unwrap();
    let (_, entry) = push_where(&user, &rail.m, |_, e| e["pads"] == json!(["notes"])).await;
    assert_eq!(entry["name"], a_name.as_str());
    assert!(!entry.to_string().contains("ZEBRA"));

    // An open Todo whose Home is a child Door, then its completion.
    let sub = user
        .request(
            "rail.createWorkstream",
            json!({ "name": "sub", "parent": rail.m }),
        )
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    user.request("rail.startDoor", json!({ "id": sub }))
        .await
        .unwrap();
    let todo = todo_at(&user, "chase the ticket").await;
    user.request("todo.move", json!({ "id": todo, "home": sub }))
        .await
        .unwrap();
    push_where(&user, &rail.m, |_, e| e["name"] == "sub" && e["todos"] == 1).await;
    user.request("todo.complete", json!({ "id": todo }))
        .await
        .unwrap();
    push_where(&user, &rail.m, |_, e| e["name"] == "sub" && e["todos"] == 0).await;
}

#[tokio::test]
async fn b21_a_change_that_leaves_the_entry_as_it_was_pushes_nothing() {
    let project = start(&[]);
    let user = project.client().await;
    let rail = rail(&project, &user).await;
    settled(&user, &rail.m).await;
    signal(&user, &rail.a, json!({"hook_event_name": "Stop"})).await;
    let before = settled(&user, &rail.m).await;

    signal(&user, &rail.a, json!({"hook_event_name": "Stop"})).await;
    let todo = todo_at(&user, "at the root").await;
    user.request("todo.update", json!({ "id": todo, "title": "renamed" }))
        .await
        .unwrap();

    assert_eq!(settled(&user, &rail.m).await, before);
}

#[tokio::test]
async fn b21_a_child_with_no_door_above_it_pushes_nothing() {
    let project = start(&[]);
    let user = project.client().await;
    let rail = rail(&project, &user).await;
    settled(&user, &rail.m).await;

    signal(&user, &rail.c, json!({"hook_event_name": "Stop"})).await;
    let as_c = client_as(&project, actor(ActorKind::Agent, &rail.c)).await;
    as_c.request("pad.create", json!({ "name": "c-notes", "text": "x" }))
        .await
        .unwrap();
    settled(&user, &rail.m).await;

    for to in [&rail.g, &rail.c, &rail.m] {
        let pushes = digest_pushes(&user, to).await;
        assert!(
            pushes.iter().all(|(_, e)| e["pads"] != json!(["c-notes"])),
            "{to}: {pushes:?}"
        );
    }
    assert!(digest_pushes(&user, &rail.g).await.is_empty());
}

#[tokio::test]
async fn b21_under_a_takeover_of_the_door_the_push_is_held() {
    let project = start(&[]);
    let user = project.client().await;
    let rail = rail(&project, &user).await;
    settled(&user, &rail.m).await;
    user.request("takeover.begin", json!({ "agent": rail.m }))
        .await
        .unwrap();

    signal(&user, &rail.b, json!({"hook_event_name": "Stop"})).await;

    let (message, _) = push_where(&user, &rail.m, |_, e| e["kind"] == "idle").await;
    assert_eq!(message["status"], "held");
    assert_eq!(message["reason"], "takeover");
}

#[tokio::test]
async fn e2_an_agent_reads_its_own_context_and_no_one_elses() {
    let project = start(&[]);
    let user = project.client().await;
    let rail = rail(&project, &user).await;
    let as_a = client_as(&project, actor(ActorKind::Agent, &rail.a)).await;

    let a = context_of(&as_a, &rail.a).await.unwrap();
    let parent = a.parent.unwrap();
    assert_eq!(
        (parent.id, parent.node),
        (rail.m.clone(), ParentNode::Agent)
    );
    assert_eq!(a.ask.to, rail.m);
    assert_eq!(
        a.peers.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
        [rail.b.as_str()]
    );
    let c = context_of(&user, &rail.c).await.unwrap();
    assert_eq!(c.ask.to, "you");
    assert!(c.peers.is_empty());
    assert_eq!(c.parent.unwrap().node, ParentNode::Group);

    let other = context_of(&as_a, &rail.b).await.unwrap_err();
    assert_eq!(other.code, rpc::code::FORBIDDEN);
    let missing = context_of(&user, "nobody").await.unwrap_err();
    assert_eq!(missing.code, rpc::code::NOT_FOUND);
    let touches: Vec<Touch> = serde_json::from_value(
        user.request("provenance.touched", json!({ "actor_id": rail.a }))
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(touches.is_empty(), "{touches:?}");
}

#[tokio::test]
async fn e4_after_a_move_the_next_call_returns_the_new_parent() {
    let project = start(&[]);
    let user = project.client().await;
    let rail = rail(&project, &user).await;

    user.request(
        "rail.move",
        json!({ "id": rail.a, "parent": rail.g, "index": 0 }),
    )
    .await
    .unwrap();
    let a = context_of(&user, &rail.a).await.unwrap();

    assert_eq!(a.parent.unwrap().id, rail.g);
    assert_eq!(a.ask.to, "you");
    assert_eq!(a.peers.len(), 1);
    assert_eq!(a.peers[0].id, rail.c);
}

/// A raw MCP client of `rup mcp <id>`: the tool's text, or its error text.
struct Shim {
    child: tokio::process::Child,
    lines: tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    next: u32,
}

impl Shim {
    async fn start(project: &Project, id: &str) -> Shim {
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_rup"))
            .args(["mcp", id])
            .env("RUPD_SOCKET", socket(project))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let mut shim = Shim {
            child,
            lines,
            next: 0,
        };
        shim.request(
            "initialize",
            json!({ "protocolVersion": "2025-06-18", "capabilities": {},
                    "clientInfo": { "name": "t", "version": "0" } }),
        )
        .await;
        let stdin = shim.child.stdin.as_mut().unwrap();
        let note = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        stdin
            .write_all(format!("{note}\n").as_bytes())
            .await
            .unwrap();
        shim
    }

    async fn request(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let line = json!({ "jsonrpc": "2.0", "id": self.next, "method": method, "params": params });
        let stdin = self.child.stdin.as_mut().unwrap();
        stdin
            .write_all(format!("{line}\n").as_bytes())
            .await
            .unwrap();
        let reply = tokio::time::timeout(Duration::from_secs(30), self.lines.next_line())
            .await
            .expect("the shim answered")
            .unwrap()
            .expect("the shim is open");
        serde_json::from_str::<Value>(&reply).unwrap()["result"].clone()
    }

    async fn call(&mut self, name: &str, arguments: Value) -> Value {
        let result = self
            .request(
                "tools/call",
                json!({ "name": name, "arguments": arguments }),
            )
            .await;
        assert_ne!(result["isError"], true, "{result}");
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
    }
}

#[tokio::test]
async fn e4_agent_context_returns_the_callers_context_after_a_move() {
    let project = start(&[]);
    let user = project.client().await;
    let rail = rail(&project, &user).await;
    let mut shim = Shim::start(&project, &rail.a).await;

    let before: Context =
        serde_json::from_value(shim.call("agent_context", json!({})).await).unwrap();
    user.request(
        "rail.move",
        json!({ "id": rail.a, "parent": rail.g, "index": 0 }),
    )
    .await
    .unwrap();
    let after: Context =
        serde_json::from_value(shim.call("agent_context", json!({})).await).unwrap();

    assert_eq!(before.this.id, rail.a);
    assert_eq!(before.ask.to, rail.m);
    assert_eq!(after.parent.unwrap().id, rail.g);
    assert_eq!(after.ask.to, "you");
}

async fn tool_names(shim: &mut Shim) -> Vec<String> {
    let listed = shim.request("tools/list", json!({})).await;
    listed["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn m1_offers_agent_digest_to_a_door_and_to_no_other_agent() {
    let project = start(&[]);
    let user = project.client().await;
    let rail = rail(&project, &user).await;

    let mut door = Shim::start(&project, &rail.m).await;
    let mut plain = Shim::start(&project, &rail.a).await;

    assert!(
        tool_names(&mut door)
            .await
            .contains(&"agent_digest".to_owned())
    );
    assert!(
        !tool_names(&mut plain)
            .await
            .contains(&"agent_digest".to_owned())
    );
    let digest = door.call("agent_digest", json!({})).await;
    assert_eq!(digest["children"].as_array().unwrap().len(), 2);
}

async fn question_to(shim: &mut Shim, project: &Project, to: &str) -> Value {
    let sent = shim
        .call(
            "message_send",
            json!({ "to": to, "kind": "question", "body": "which file?" }),
        )
        .await;
    let user = project.client().await;
    let listed = user
        .request("message.list", json!({ "to": to }))
        .await
        .unwrap();
    assert!(
        listed
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["id"] == sent["id"] && m["body"] == "which file?"),
        "{listed}"
    );
    sent
}

#[tokio::test]
async fn e5_ask_to_is_a_valid_recipient_for_a_door_and_for_the_user() {
    let project = start(&[]);
    let user = project.client().await;
    let rail = rail(&project, &user).await;

    for id in [&rail.a, &rail.c] {
        let mut shim = Shim::start(&project, id).await;
        let context: Context =
            serde_json::from_value(shim.call("agent_context", json!({})).await).unwrap();
        question_to(&mut shim, &project, &context.ask.to).await;
    }
    let mut shim = Shim::start(&project, &rail.a).await;
    question_to(&mut shim, &project, &rail.b).await;
}

#[tokio::test]
async fn e3_rup_context_prints_the_brief_verbatim_and_exits_0() {
    let project = start(&[]);
    let user = project.client().await;
    let rail = rail(&project, &user).await;
    let brief = user
        .request("agent.brief", json!({ "id": rail.a }))
        .await
        .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_rup"))
        .args(["context", &rail.a])
        .env("RUPD_SOCKET", socket(&project))
        .output()
        .unwrap();

    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        brief["stdout"].as_str().unwrap()
    );
    let as_a = client_as(&project, actor(ActorKind::Agent, &rail.a)).await;
    let other = as_a
        .request("agent.brief", json!({ "id": rail.b }))
        .await
        .unwrap_err();
    assert_eq!(other.code, rpc::code::FORBIDDEN);
    let missing = user
        .request("agent.brief", json!({ "id": "nobody" }))
        .await
        .unwrap_err();
    assert_eq!(missing.code, rpc::code::NOT_FOUND);
}

#[tokio::test]
async fn e3_a_dead_daemon_gives_one_stderr_line_no_stdout_and_exit_0() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("none.sock");

    let out = Command::new(env!("CARGO_BIN_EXE_rup"))
        .args(["context", "a1"])
        .env("RUPD_SOCKET", &socket)
        .output()
        .unwrap();

    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.contains(socket.to_str().unwrap()), "{stderr}");
}

#[tokio::test]
async fn e3_a_daemon_that_never_answers_is_given_up_on_within_five_seconds() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("hung.sock");
    let listener = tokio::net::UnixListener::bind(&socket).unwrap();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((stream, _)) = listener.accept().await {
            held.push(stream);
        }
    });

    let started = std::time::Instant::now();
    let out = tokio::process::Command::new(env!("CARGO_BIN_EXE_rup"))
        .args(["context", "a1"])
        .env("RUPD_SOCKET", &socket)
        .output()
        .await
        .unwrap();

    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty());
    assert_eq!(String::from_utf8_lossy(&out.stderr).lines().count(), 1);
}

#[tokio::test]
async fn e3_the_fake_claude_runs_both_session_start_commands_and_reads_the_context() {
    let report = tempfile::tempdir().unwrap().keep().join("context.txt");
    let project = start(&[
        ("FAKE_CLAUDE_PAYLOADS", RECORDED),
        ("FAKE_CLAUDE_EVENTS", "SessionStart"),
        ("FAKE_CLAUDE_CONTEXT_REPORT", report.to_str().unwrap()),
    ]);
    let user = project.client().await;
    let id = agent_in(&project, &user, None).await;

    let told = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            match std::fs::read_to_string(&report) {
                Ok(text) if !text.is_empty() => return text,
                _ => tokio::time::sleep(Duration::from_millis(20)).await,
            }
        }
    })
    .await
    .expect("the context was printed");

    let told: Value = serde_json::from_str(&told).unwrap();
    assert_eq!(told["hookSpecificOutput"]["hookEventName"], "SessionStart");
    assert!(
        told["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains("Parent: none")
    );
    assert_eq!(
        rail_tree(&user)
            .await
            .iter()
            .find(|n| n.id == id)
            .unwrap()
            .id,
        id
    );
}

async fn channel_of(client: &rpc::Client, id: &str) -> Option<Channel> {
    rail_tree(client)
        .await
        .into_iter()
        .find(|n| n.id == id)
        .unwrap()
        .channel
}

async fn next_channel(client: &mut rpc::Client, id: &str) -> Channel {
    next(client, |data| match data {
        EventData::AgentChannel(event) if event.id == id => Some(event.channel),
        _ => None,
    })
    .await
}

#[tokio::test]
async fn e6_a_shim_started_at_once_goes_pending_to_up_and_never_missing() {
    let project = start(&[
        ("FAKE_CLAUDE_TOOLS", "[]"),
        ("ROUNDUP_CHANNEL_DEADLINE_MS", "3000"),
    ]);
    let mut events = project.subscribed().await;

    let node = project.spawn_agent(&events).await;
    assert_eq!(node.channel, Some(Channel::Pending));

    assert_eq!(next_channel(&mut events, &node.id).await, Channel::Up);
    tokio::time::sleep(Duration::from_millis(3500)).await;
    assert_eq!(channel_of(&events, &node.id).await, Some(Channel::Up));
}

#[tokio::test]
async fn e6_no_shim_is_missing_with_the_status_unchanged_and_hooks_do_not_count() {
    let project = start(&[
        ("FAKE_CLAUDE_PAYLOADS", RECORDED),
        ("FAKE_CLAUDE_EVENTS", "SessionStart"),
        DEADLINE,
    ]);
    let mut events = project.subscribed().await;

    let node = project.spawn_agent(&events).await;
    let status = next(&mut events, |data| match data {
        EventData::AgentStatus(s) if s.id == node.id => Some(s.status),
        _ => None,
    })
    .await;

    assert_eq!(next_channel(&mut events, &node.id).await, Channel::Missing);
    let now = rail_tree(&events)
        .await
        .into_iter()
        .find(|n| n.id == node.id)
        .unwrap();
    assert_eq!(now.channel, Some(Channel::Missing));
    assert_eq!(now.status.unwrap().kind, status.kind);
}

#[tokio::test]
async fn e6_a_late_shim_goes_missing_then_up() {
    let project = start(&[
        ("FAKE_CLAUDE_TOOLS", "[]"),
        ("FAKE_CLAUDE_SHIM_DELAY_MS", "2000"),
        DEADLINE,
    ]);
    let mut events = project.subscribed().await;

    let node = project.spawn_agent(&events).await;

    assert_eq!(next_channel(&mut events, &node.id).await, Channel::Missing);
    assert_eq!(next_channel(&mut events, &node.id).await, Channel::Up);
}

#[tokio::test]
async fn e6_only_the_agent_reports_its_channel_and_a_second_report_emits_nothing() {
    let project = start(&[("ROUNDUP_CHANNEL_DEADLINE_MS", "60000")]);
    let mut user = project.subscribed().await;
    let rail = rail(&project, &user).await;
    let as_a = client_as(&project, actor(ActorKind::Agent, &rail.a)).await;
    let id = json!({ "id": rail.a });

    assert_eq!(
        user.request("agent.channelUp", id.clone())
            .await
            .unwrap_err()
            .code,
        rpc::code::FORBIDDEN
    );
    let as_b = client_as(&project, actor(ActorKind::Agent, &rail.b)).await;
    assert_eq!(
        as_b.request("agent.channelUp", id.clone())
            .await
            .unwrap_err()
            .code,
        rpc::code::FORBIDDEN
    );
    assert_eq!(
        as_a.request("agent.channelUp", json!({ "id": "nobody" }))
            .await
            .unwrap_err()
            .code,
        rpc::code::NOT_FOUND
    );
    as_a.request("agent.channelUp", id.clone()).await.unwrap();
    as_a.request("agent.channelUp", id).await.unwrap();

    assert_eq!(next_channel(&mut user, &rail.a).await, Channel::Up);
    as_a.request("daemon.ping", Value::Null).await.unwrap();
    let again =
        tokio::time::timeout(Duration::from_millis(300), next_channel(&mut user, &rail.a)).await;
    assert!(again.is_err(), "a second report emitted an event");
}

#[tokio::test]
async fn e6_only_an_agent_with_a_live_terminal_has_a_channel() {
    let project = start(&[("ROUNDUP_CHANNEL_DEADLINE_MS", "60000")]);
    let user = project.client().await;
    let rail = rail(&project, &user).await;

    let tree = rail_tree(&user).await;
    let channel = |id: &str| tree.iter().find(|n| n.id == id).unwrap().channel;

    assert_eq!(channel(&rail.a), Some(Channel::Pending));
    assert_eq!(channel(&rail.g), None);
    for terminal in tree
        .iter()
        .filter(|n| n.kind == contracts::agent::NodeKind::Terminal)
    {
        assert_eq!(terminal.channel, None);
    }
}

async fn todo_at(client: &rpc::Client, title: &str) -> u32 {
    let todo = client
        .request("todo.create", json!({ "title": title }))
        .await
        .unwrap();
    serde_json::from_value::<Todo>(todo).unwrap().id
}

async fn home_of(client: &rpc::Client, id: u32) -> Value {
    client
        .request("todo.get", json!({ "id": id }))
        .await
        .unwrap()["home"]
        .clone()
}

/// T12, T13: a Todo moves to a Workstream, the Daemon checks the Rail, and the Home survives a restart.
#[tokio::test]
async fn t13_move_checks_the_rail_and_survives_a_restart() {
    let mut project = start(&[]);
    let mut events = project.subscribed().await;
    let client = project.client().await;
    let r1 = workstream(&client, "r1").await;
    let agent = agent_in(&project, &client, None).await;
    let id = todo_at(&client, "a").await;
    assert_eq!(home_of(&client, id).await, Value::Null);
    next(&mut events, |data| {
        matches!(data, EventData::TodoCreated(_)).then_some(())
    })
    .await;

    let history = || async {
        client
            .request(
                "provenance.history",
                json!({ "item": format!("todo:{id}") }),
            )
            .await
            .unwrap()
    };
    let touched = history().await;
    let missing = client
        .request("todo.move", json!({ "id": id, "home": "nope" }))
        .await
        .unwrap_err();
    assert_eq!(missing.code, rpc::code::NOT_FOUND);
    let not_a_workstream = client
        .request("todo.move", json!({ "id": id, "home": agent }))
        .await
        .unwrap_err();
    assert_eq!(not_a_workstream.code, rpc::code::INVALID_PARAMS);
    assert_eq!(history().await, touched);
    assert_eq!(home_of(&client, id).await, Value::Null);

    client
        .request("todo.move", json!({ "id": id, "home": r1 }))
        .await
        .unwrap();
    // The first Todo event after the rejections is the move that succeeded.
    let first = next(&mut events, |data| match data {
        EventData::TodoUpdated(todo) => Some(todo.home.clone()),
        _ => None,
    })
    .await;
    assert_eq!(first, Some(r1.clone()));
    assert_eq!(home_of(&client, id).await, json!(r1));

    project.restart(&[]);
    assert_eq!(home_of(&project.client().await, id).await, json!(r1));
}

#[tokio::test]
async fn t13_workstream_removal_resets_homes_and_emits_in_id_order() {
    let project = start(&[]);
    let mut events = project.subscribed().await;
    let client = project.client().await;
    let r1 = workstream(&client, "r1").await;
    let r2 = workstream(&client, "r2").await;
    let (a, b, c, d) = (
        todo_at(&client, "a").await,
        todo_at(&client, "b").await,
        todo_at(&client, "c").await,
        todo_at(&client, "d").await,
    );
    for (id, home) in [(d, &r1), (b, &r1), (c, &r2)] {
        client
            .request("todo.move", json!({ "id": id, "home": home }))
            .await
            .unwrap();
    }
    // Drain the events of the setup.
    for _ in 0..7 {
        next(&mut events, |data| {
            matches!(data, EventData::TodoUpdated(_) | EventData::TodoCreated(_)).then_some(())
        })
        .await;
    }

    client
        .request("rail.remove", json!({ "id": r1 }))
        .await
        .unwrap();

    let first = next(&mut events, |data| match data {
        EventData::TodoUpdated(todo) => Some(todo.id),
        _ => None,
    })
    .await;
    let second = next(&mut events, |data| match data {
        EventData::TodoUpdated(todo) => Some(todo.id),
        _ => None,
    })
    .await;
    assert_eq!([first, second], [b, d]);
    assert!(rail_tree(&client).await.iter().all(|node| node.id != r1));
    assert_eq!(home_of(&client, b).await, Value::Null);
    assert_eq!(home_of(&client, d).await, Value::Null);
    assert_eq!(home_of(&client, c).await, json!(r2));
    assert_eq!(home_of(&client, a).await, Value::Null);
    let listed = client.request("todo.list", json!(null)).await.unwrap();
    assert_eq!(listed.as_array().unwrap().len(), 4);
}

#[tokio::test]
async fn t13_a_failed_rail_remove_leaves_every_home() {
    let project = start(&[]);
    let client = project.client().await;
    let r1 = workstream(&client, "r1").await;
    let id = todo_at(&client, "a").await;
    client
        .request("todo.move", json!({ "id": id, "home": r1 }))
        .await
        .unwrap();
    let mut events = project.subscribed().await;

    let err = client
        .request("rail.remove", json!({ "id": "nope" }))
        .await
        .unwrap_err();
    assert_eq!(err.code, rpc::code::NOT_FOUND);
    assert_eq!(home_of(&client, id).await, json!(r1));
    let none = tokio::time::timeout(
        Duration::from_millis(300),
        next(&mut events, |data| {
            matches!(data, EventData::TodoUpdated(_)).then_some(())
        }),
    )
    .await;
    assert!(none.is_err());
}

/// T12: a Todo made by an Agent or an Extension is at the Project root, whatever its params say.
#[tokio::test]
async fn t12_every_caller_creates_at_the_project_root_on_a_real_daemon() {
    let project = start(&[]);
    for who in [actor(ActorKind::Agent, "a1"), actor(ActorKind::Ext, "x")] {
        let client = client_as(&project, who).await;
        let made = client
            .request("todo.create", json!({ "title": "t", "home": "r1" }))
            .await
            .unwrap();
        assert_eq!(made["home"], Value::Null);
    }
}

#[tokio::test]
async fn t14_an_agent_under_a_workstream_creates_todos_with_the_nearest_workstream_as_home() {
    let project = start(&[]);
    let client = project.client().await;
    let outer = workstream(&client, "outer").await;
    let nested = agent_in(&project, &client, Some(&outer)).await;
    let deeper = agent_in_as(&project, &nested).await;
    let bare = agent_in(&project, &client, None).await;

    let create = |who: rpc::Client| async move {
        let todo = who
            .request("todo.create", json!({ "title": "t", "home": "ignored" }))
            .await
            .unwrap();
        todo["home"].clone()
    };
    for agent in [&nested, &deeper] {
        let by_nested = client_as(&project, actor(ActorKind::Agent, agent)).await;
        assert_eq!(create(by_nested).await, json!(outer));
    }
    let by_bare = client_as(&project, actor(ActorKind::Agent, &bare)).await;
    assert_eq!(create(by_bare).await, Value::Null);
    assert_eq!(create(project.client().await).await, Value::Null);
    let by_ext = client_as(&project, actor(ActorKind::Ext, "x")).await;
    assert_eq!(create(by_ext).await, Value::Null);
    let listed = client.request("todo.list", json!(null)).await.unwrap();
    assert_eq!(listed[0]["home"], json!(outer));
}

/// T14: a Door's Rail id is its Workstream's id, so its Todo's Home is that Workstream.
#[tokio::test]
async fn t14_a_door_creates_todos_at_its_own_workstream() {
    let project = start(&[]);
    let client = project.client().await;
    for name in ["outer", "other"] {
        let door = workstream(&client, name).await;
        let who = client_as(&project, actor(ActorKind::Agent, &door)).await;
        let todo = who
            .request("todo.create", json!({ "title": "ship j4" }))
            .await
            .unwrap();
        assert_eq!(todo["home"], json!(door));
    }
}

async fn ids_listed(client: &rpc::Client) -> Vec<u32> {
    let listed: Vec<Todo> =
        serde_json::from_value(client.request("todo.list", json!(null)).await.unwrap()).unwrap();
    listed.into_iter().map(|todo| todo.id).collect()
}

/// T11: four Todos, `todo.reorder`, and the ids `todo.list` prints.
#[tokio::test]
async fn t11_reorder_changes_the_list_on_a_real_daemon() {
    let project = start(&[]);
    let client = project.client().await;
    for title in ["a", "b", "c", "d"] {
        todo_at(&client, title).await;
    }
    let mut events = project.subscribed().await;
    let moved = client
        .request("todo.reorder", json!({ "id": 3, "before": 1 }))
        .await
        .unwrap();
    assert_eq!(moved["id"], 3);
    let updated = next(&mut events, |data| match data {
        EventData::TodoUpdated(todo) => Some(todo.id),
        _ => None,
    })
    .await;
    assert_eq!(updated, 3);
    assert_eq!(ids_listed(&client).await, [3, 1, 2, 4]);
    let missing = client
        .request("todo.reorder", json!({ "id": 3, "before": 9 }))
        .await
        .unwrap_err();
    assert_eq!(missing.code, rpc::code::NOT_FOUND);
    assert_eq!(ids_listed(&client).await, [3, 1, 2, 4]);
    let more = tokio::time::timeout(
        Duration::from_millis(300),
        next(&mut events, |data| {
            matches!(data, EventData::TodoUpdated(_)).then_some(())
        }),
    )
    .await;
    assert!(more.is_err());
}

/// T10, U155: the order the user dropped a Todo into is the order `todo.list` gives after `rupd` restarts.
#[tokio::test]
async fn t11_reordered_list_survives_a_restart() {
    let mut project = start(&[]);
    let client = project.client().await;
    for title in ["a", "b", "c"] {
        todo_at(&client, title).await;
    }
    client
        .request("todo.reorder", json!({ "id": 3, "before": 1 }))
        .await
        .unwrap();
    assert_eq!(ids_listed(&client).await, [3, 1, 2]);

    project.restart(&[]);
    assert_eq!(ids_listed(&project.client().await).await, [3, 1, 2]);
}

/// T10: an Agent's `todo_list`, `todo_create` and `agent.context` all read the user's order.
#[tokio::test]
async fn t10_the_mcp_server_and_the_context_read_the_reordered_list() {
    let project = start(&[]);
    let user = project.client().await;
    let rail = rail(&project, &user).await;
    let mut shim = Shim::start(&project, &rail.a).await;
    for title in ["a", "b", "c"] {
        shim.call("todo_create", json!({ "title": title })).await;
    }
    user.request("todo.reorder", json!({ "id": 3, "before": 1 }))
        .await
        .unwrap();
    let ids = |list: Value| -> Vec<u64> {
        list.as_array()
            .unwrap()
            .iter()
            .map(|todo| todo["id"].as_u64().unwrap())
            .collect()
    };
    assert_eq!(ids(shim.call("todo_list", json!({})).await), [3, 1, 2]);
    shim.call("todo_create", json!({ "title": "d" })).await;
    assert_eq!(ids(shim.call("todo_list", json!({})).await), [3, 1, 2, 4]);
    let context: Context =
        serde_json::from_value(shim.call("agent_context", json!({})).await).unwrap();
    let in_context: Vec<u32> = context.todos.iter().map(|todo| todo.id).collect();
    assert_eq!(in_context, [3, 1, 2, 4]);
}

// F2 to F5: a Door has no shell, starts children through `agent_spawn`, finds its `claude` shimmed,
// and has a stray vendor program flagged. `spikes/spawn-boundary/REPORT.md` is what the real
// Claude Code did (F1).

fn executable(path: &std::path::Path, text: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A directory holding a `claude` that prints `9.9.9`, ahead of the test's own `PATH`.
fn path_with_fake_claude(root: &std::path::Path) -> String {
    let bin = root.join("path-bin");
    std::fs::create_dir_all(&bin).unwrap();
    executable(&bin.join("claude"), "#!/bin/sh\necho 9.9.9\n");
    format!("{}:{}", bin.display(), std::env::var("PATH").unwrap())
}

async fn json_file(path: &std::path::Path) -> Value {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Ok(text) = std::fs::read_to_string(path)
                && let Ok(value) = serde_json::from_str(&text)
            {
                return value;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the fake claude wrote its report")
}

async fn start_door(client: &rpc::Client) -> String {
    let id = workstream(client, "team").await;
    client
        .request("rail.startDoor", json!({ "id": id }))
        .await
        .unwrap();
    id
}

fn settings_of(project: &Project, id: &str) -> Value {
    let path = project
        .dir
        .path()
        .join(format!(".roundup/agents/{id}.settings.json"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[tokio::test]
async fn f2_an_ordinary_agent_runs_claude_directly_and_through_sh() {
    let root = tempfile::tempdir().unwrap();
    let report = root.path().join("run.json");
    let runs = json!([
        "claude --version",
        "sh -c 'claude --version'",
        ["claude", "--version"]
    ]);
    let project = start(&[
        ("PATH", &path_with_fake_claude(root.path())),
        ("FAKE_CLAUDE_RUN", &runs.to_string()),
        ("FAKE_CLAUDE_RUN_REPORT", report.to_str().unwrap()),
    ]);
    let client = project.client().await;

    let agent = project.spawn_agent(&client).await;
    let done = json_file(&report).await;

    for run in done.as_array().unwrap() {
        assert_eq!(
            (run["code"].as_i64(), run["stdout"].as_str()),
            (Some(0), Some("9.9.9\n"))
        );
    }
    assert!(settings_of(&project, &agent.id).get("env").is_none());
}

#[tokio::test]
async fn f4_a_door_finds_a_shim_for_claude_directly_and_through_sh_and_other_programs_run() {
    let root = tempfile::tempdir().unwrap();
    let report = root.path().join("run.json");
    let runs = json!([
        "claude --version",
        "sh -c 'claude --version'",
        ["claude", "--version"],
        "/bin/echo hi",
        "git --version"
    ]);
    let project = start(&[
        ("PATH", &path_with_fake_claude(root.path())),
        ("FAKE_CLAUDE_RUN", &runs.to_string()),
        ("FAKE_CLAUDE_RUN_REPORT", report.to_str().unwrap()),
    ]);
    let client = project.client().await;

    start_door(&client).await;
    let done = json_file(&report).await;
    let done = done.as_array().unwrap();

    let message = "roundup: start an agent with agent_spawn (a Door) or ask the user\n";
    for run in &done[..3] {
        assert_eq!(run["code"], 1);
        assert_eq!(run["stderr"], message);
        assert_eq!(run["stdout"], "");
    }
    assert_eq!(
        (done[3]["code"].as_i64(), done[3]["stdout"].as_str()),
        (Some(0), Some("hi\n"))
    );
    assert_eq!(done[4]["code"], 0);
    assert!(
        done[4]["stdout"]
            .as_str()
            .unwrap()
            .starts_with("git version")
    );
}

#[tokio::test]
async fn f4_a_doors_path_in_its_settings_is_its_shim_directory_then_the_daemons_path() {
    let root = tempfile::tempdir().unwrap();
    let path = path_with_fake_claude(root.path());
    let project = start(&[("PATH", &path)]);
    let client = project.client().await;

    let door = start_door(&client).await;

    let shims = project
        .dir
        .path()
        .canonicalize()
        .unwrap()
        .join(format!(".roundup/agents/{door}.bin"));
    assert_eq!(
        settings_of(&project, &door)["env"]["PATH"],
        format!("{}:{path}", shims.display())
    );
    assert!(shims.join("claude").is_file());
}

#[tokio::test]
async fn f2_a_doors_brief_and_tools_hold_agent_spawn_and_an_agents_hold_none() {
    let root = tempfile::tempdir().unwrap();
    let report = root.path().join("seen.json");
    let project = start(&[("FAKE_CLAUDE_BRIEF_REPORT", report.to_str().unwrap())]);
    let client = project.client().await;

    start_door(&client).await;
    let seen = json_file(&report).await;

    let listed: Vec<_> = seen["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap())
        .collect();
    assert!(listed.contains(&"agent_spawn"));
    assert_eq!(listed.len(), 22);
    assert!(seen["brief"].as_str().unwrap().contains("no shell"));
}

#[tokio::test]
async fn f3_a_door_starts_a_child_with_agent_spawn_and_the_child_is_the_doors_own() {
    let root = tempfile::tempdir().unwrap();
    let report = root.path().join("result.json");
    let cwd = root.path().join("never-used");
    let project = start(&[]);
    let cwd_in_project = project.dir.path().to_string_lossy().into_owned();
    drop(cwd);
    let tools = json!([{"name": "agent_spawn", "arguments": {
        "cwd": cwd_in_project, "prompt": "do it", "parent": "999"
    }}]);
    let mut project = project;
    project.restart(&[
        ("FAKE_CLAUDE_DOOR_TOOLS", &tools.to_string()),
        ("FAKE_CLAUDE_TOOL_REPORT", report.to_str().unwrap()),
    ]);
    let client = project.client().await;

    let door = start_door(&client).await;
    let spawned: RailNode = serde_json::from_value(json_file(&report).await[0].clone()).unwrap();

    assert_eq!(spawned.parent.as_deref(), Some(door.as_str()));
    assert!(spawned.worktree.is_none());
    let tree = rail_tree(&client).await;
    let child = tree.iter().find(|n| n.id == spawned.id).unwrap();
    assert_eq!(child.parent.as_deref(), Some(door.as_str()));
    assert!(tree.iter().all(|n| n.parent.as_deref() != Some("999")));
}

#[tokio::test]
async fn f3_a_door_spawns_into_a_worktree_when_the_setting_is_on() {
    let root = tempfile::tempdir().unwrap();
    let report = root.path().join("result.json");
    let project = start(&[]);
    let path = project.dir.path().to_owned();
    git_at(&path, &["init", "-q", "-b", "main"]);
    git_at(&path, &["config", "user.name", "roundup"]);
    git_at(&path, &["config", "user.email", "roundup@example.com"]);
    std::fs::write(path.join("README"), "keep").unwrap();
    git_at(&path, &["add", "README"]);
    git_at(&path, &["commit", "-qm", "base"]);
    let tools = json!([{"name": "agent_spawn", "arguments": {"cwd": path.to_string_lossy()}}]);
    let mut project = project;
    project.restart(&[
        ("FAKE_CLAUDE_DOOR_TOOLS", &tools.to_string()),
        ("FAKE_CLAUDE_TOOL_REPORT", report.to_str().unwrap()),
    ]);
    let client = project.client().await;
    client
        .request("project.setWorktrees", json!({"on": true, "check": null}))
        .await
        .unwrap();

    let door = start_door(&client).await;
    let spawned: RailNode = serde_json::from_value(json_file(&report).await[0].clone()).unwrap();

    assert_eq!(spawned.parent.as_deref(), Some(door.as_str()));
    let worktree = spawned.worktree.expect("the child has a Worktree");
    assert_eq!(worktree.branch, format!("roundup/agent-{}", spawned.id));
    let listed = git_at(&path, &["worktree", "list"]);
    assert!(listed.contains(&worktree.path), "{listed}");
}

#[tokio::test]
async fn f3_any_agent_starts_a_subagent_under_itself_and_its_parent_param_is_ignored() {
    let project = start(&[]);
    let client = project.client().await;
    let agent = project.spawn_agent(&client).await;
    let other = project.spawn_agent(&client).await;

    let sub = agent_in_as(&project, &agent.id).await;
    let tree = rail_tree(&client).await;
    let node = tree.iter().find(|node| node.id == sub).unwrap();
    assert_eq!(node.parent.as_deref(), Some(agent.id.as_str()));
    assert_ne!(node.parent.as_deref(), Some(other.id.as_str()));
}

#[tokio::test]
async fn f3_a_terminal_is_refused_and_nothing_changes() {
    let project = start(&[]);
    let client = project.client().await;
    let terminal = client
        .request(
            "rail.spawnTerminal",
            json!({"cwd": project.dir.path(), "parent": null}),
        )
        .await
        .unwrap();
    let id = terminal["id"].as_str().unwrap().to_owned();
    let before = rail_tree(&client).await;
    let as_terminal = client_as(&project, actor(ActorKind::Agent, &id)).await;

    let refused = as_terminal
        .request(
            "agent.spawn",
            json!({"cwd": project.dir.path(), "prompt": null, "parent": null}),
        )
        .await
        .unwrap_err();

    assert_eq!(refused.code, rpc::code::FORBIDDEN);
    assert_eq!(rail_tree(&client).await, before);
}

/// A script named `claude` that leaves its pid in `pid` and sleeps.
fn stray_script(root: &std::path::Path) -> (PathBuf, PathBuf) {
    let folder = root.join("stray");
    std::fs::create_dir_all(&folder).unwrap();
    let script = folder.join("claude");
    let pid = root.join("stray.pid");
    executable(
        &script,
        &format!(
            "#!/bin/sh\necho $$ > '{}'\nsleep 60 & wait\n",
            pid.display()
        ),
    );
    (script, pid)
}

async fn next_stray(client: &mut rpc::Client) -> contracts::agent::StrayEvent {
    next(client, |data| match data {
        EventData::AgentStray(event) => Some(event),
        _ => None,
    })
    .await
}

#[tokio::test]
async fn f5_a_stray_claude_under_a_door_is_flagged_once_and_cleared_once_when_it_exits() {
    let root = tempfile::tempdir().unwrap();
    let (script, pid) = stray_script(root.path());
    let project = start(&[
        ("ROUNDUP_SCAN_MS", "100"),
        ("FAKE_CLAUDE_STRAY", script.to_str().unwrap()),
    ]);
    let mut client = project.subscribed().await;
    let door = start_door(&client).await;
    let status = rail_tree(&client)
        .await
        .into_iter()
        .find(|n| n.id == door)
        .unwrap()
        .status;

    let flagged = next_stray(&mut client).await;
    let pid_found: u32 = json_file_text(&pid).await.trim().parse().unwrap();

    assert_eq!(flagged.id, door);
    assert_eq!(flagged.stray.len(), 1);
    assert_eq!(flagged.stray[0].pid, pid_found);
    assert!(flagged.stray[0].command.contains("claude"), "{flagged:?}");
    let node = rail_tree(&client)
        .await
        .into_iter()
        .find(|n| n.id == door)
        .unwrap();
    assert_eq!(node.stray, flagged.stray);
    assert_eq!(node.status, status);
    assert_eq!(node.kind, contracts::agent::NodeKind::Workstream);

    assert!(
        Command::new("kill")
            .arg(pid_found.to_string())
            .status()
            .unwrap()
            .success()
    );
    let cleared = next_stray(&mut client).await;
    assert_eq!((cleared.id, cleared.stray), (door.clone(), vec![]));
    let node = rail_tree(&client)
        .await
        .into_iter()
        .find(|n| n.id == door)
        .unwrap();
    assert!(node.stray.is_empty());
}

async fn json_file_text(path: &std::path::Path) -> String {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            match std::fs::read_to_string(path) {
                Ok(text) if text.ends_with('\n') => return text,
                _ => tokio::time::sleep(Duration::from_millis(20)).await,
            }
        }
    })
    .await
    .expect("the file was written")
}

#[tokio::test]
async fn f5_a_program_a_users_mcp_server_starts_is_flagged() {
    let root = tempfile::tempdir().unwrap();
    let (script, _) = stray_script(root.path());
    let project = start(&[
        ("ROUNDUP_SCAN_MS", "100"),
        ("FAKE_CLAUDE_MCP_STRAY", script.to_str().unwrap()),
    ]);
    let mut client = project.subscribed().await;
    let door = start_door(&client).await;

    let flagged = next_stray(&mut client).await;

    assert_eq!((flagged.id, flagged.stray.len()), (door, 1));
}

#[tokio::test]
async fn f5_only_a_doors_tree_is_scanned_and_a_registered_terminal_is_not_flagged() {
    let root = tempfile::tempdir().unwrap();
    let (script, pid) = stray_script(root.path());
    let shell = root.path().join("shell");
    std::fs::create_dir_all(&shell).unwrap();
    let named_claude = shell.join("claude");
    executable(&named_claude, "#!/bin/sh\nsleep 60 & wait\n");
    let project = start(&[
        ("ROUNDUP_SCAN_MS", "100"),
        ("SHELL", named_claude.to_str().unwrap()),
        ("FAKE_CLAUDE_STRAY", script.to_str().unwrap()),
    ]);
    let client = project.client().await;
    let agent = project.spawn_agent(&client).await;
    client
        .request(
            "rail.spawnTerminal",
            json!({"cwd": project.dir.path(), "parent": null}),
        )
        .await
        .unwrap();
    json_file_text(&pid).await;

    tokio::time::sleep(Duration::from_millis(600)).await;

    let tree = rail_tree(&client).await;
    assert!(tree.iter().any(|n| n.id == agent.id));
    assert!(tree.iter().all(|n| n.stray.is_empty()), "{tree:?}");
}
