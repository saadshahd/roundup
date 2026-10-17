//! B41 to B44 through a real Daemon: an Agent asks to be woken when named Agents finish, is told
//! once, and a restart neither loses nor repeats the note. Statuses are driven by Signals.

use std::time::Duration;

use contracts::ActorKind;
use contracts::agent::RailNode;
use rpc::code;
use serde_json::{Value, json};

use super::daemon::{Project, h21_ask, h21_bash, h21_poll, rail_tree, signal, start};
use super::{actor, agent_in_as, client_as, start_door};

pub(super) const STOP: &str = "Stop";
pub(super) const WORK: &str = "UserPromptSubmit";
pub(super) const FAIL: &str = "StopFailure";

pub(super) async fn drive(user: &rpc::Client, id: &str, event: &str) {
    signal(user, id, json!({ "hook_event_name": event })).await;
}

pub(super) async fn ask(
    who: &rpc::Client,
    ids: &[&str],
    any: bool,
) -> Result<Value, rpc::RpcError> {
    who.request("agent.wake", json!({ "ids": ids, "any": any }))
        .await
}

/// Every Message to `to` whose body starts with `prefix`, as the user sees them.
pub(super) async fn messages(user: &rpc::Client, to: &str, prefix: &str) -> Vec<Value> {
    let listed = user
        .request("message.list", json!({ "to": to }))
        .await
        .unwrap();
    listed
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["body"].as_str().unwrap().starts_with(prefix))
        .cloned()
        .collect()
}

/// The notes of `to` once at least `least` are there.
pub(super) async fn notes(user: &rpc::Client, to: &str, prefix: &str, least: usize) -> Vec<Value> {
    h21_poll("the notes", async || {
        let found = messages(user, to, prefix).await;
        (found.len() >= least).then_some(found)
    })
    .await
}

/// Nothing more arrives: the count stays `count` after the Daemon has had every chance.
pub(super) async fn still(user: &rpc::Client, to: &str, prefix: &str, count: usize) {
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(messages(user, to, prefix).await.len(), count);
}

async fn name_of(user: &rpc::Client, id: &str) -> String {
    rail_tree(user)
        .await
        .into_iter()
        .find(|node| node.id == id)
        .unwrap()
        .name
}

pub(super) async fn named(
    project: &Project,
    user: &rpc::Client,
    name: &str,
    parent: Option<&str>,
) -> String {
    let node = user
        .request(
            "agent.spawn",
            json!({ "cwd": project.dir.path(), "prompt": null, "parent": parent, "name": name }),
        )
        .await
        .unwrap();
    serde_json::from_value::<RailNode>(node).unwrap().id
}

/// A started Door `d` with two working children `a` and `b`, and Agent `w` (a plain Agent) as
/// the waiter.
pub(super) struct Scene {
    pub(super) project: Project,
    pub(super) user: rpc::Client,
    pub(super) door: String,
    pub(super) a: String,
    pub(super) b: String,
    pub(super) w: String,
    pub(super) as_w: rpc::Client,
}

pub(super) async fn scene(fake: &[(&str, &str)]) -> Scene {
    let project = start(fake);
    let user = project.client().await;
    let door = start_door(&user).await;
    let a = named(&project, &user, "alpha", Some(&door)).await;
    let b = named(&project, &user, "beta", Some(&door)).await;
    let w = named(&project, &user, "waiter", None).await;
    for id in [&a, &b] {
        drive(&user, id, WORK).await;
    }
    let as_w = client_as(&project, actor(ActorKind::Agent, &w)).await;
    Scene {
        project,
        user,
        door,
        a,
        b,
        w,
        as_w,
    }
}

#[tokio::test]
async fn b41_a_request_returns_at_once_and_holds_nothing_open() {
    let s = scene(&[]).await;
    let reply = tokio::time::timeout(Duration::from_secs(5), ask(&s.as_w, &[&s.a, &s.b], false))
        .await
        .expect("the call returned while both children work")
        .unwrap();
    assert_eq!(reply["fired"], false);
    assert!(reply["id"].as_u64().is_some());
    still(&s.user, &s.w, "[wake]", 0).await;
}

#[tokio::test]
async fn b41_an_already_finished_agent_fires_at_once() {
    let s = scene(&[]).await;
    drive(&s.user, &s.a, STOP).await;
    let reply = ask(&s.as_w, &[&s.a], false).await.unwrap();
    assert_eq!(reply["fired"], true);
    assert_eq!(notes(&s.user, &s.w, "[wake]", 1).await.len(), 1);
}

#[tokio::test]
async fn b41_all_waits_for_every_named_agent_and_keeps_a_flip_finished() {
    let s = scene(&[]).await;
    ask(&s.as_w, &[&s.a, &s.b], false).await.unwrap();
    drive(&s.user, &s.a, STOP).await;
    // `a` resumes: it stays finished for the request.
    drive(&s.user, &s.a, WORK).await;
    still(&s.user, &s.w, "[wake]", 0).await;
    drive(&s.user, &s.b, STOP).await;
    let note = notes(&s.user, &s.w, "[wake]", 1).await;
    let body = note[0]["body"].as_str().unwrap();
    let (a, b) = (name_of(&s.user, &s.a).await, name_of(&s.user, &s.b).await);
    assert!(
        body.contains(&format!("{a} is working")) && body.contains(&format!("{b} is idle")),
        "{body}"
    );
}

#[tokio::test]
async fn b41_any_fires_on_the_first() {
    let s = scene(&[]).await;
    ask(&s.as_w, &[&s.a, &s.b], true).await.unwrap();
    still(&s.user, &s.w, "[wake]", 0).await;
    drive(&s.user, &s.b, STOP).await;
    let note = notes(&s.user, &s.w, "[wake]", 1).await;
    let body = note[0]["body"].as_str().unwrap();
    let a = name_of(&s.user, &s.a).await;
    assert!(body.contains(&format!("{a} is working")), "{body}");
}

#[tokio::test]
async fn b41_a_removed_agent_counts_as_done() {
    let s = scene(&[]).await;
    ask(&s.as_w, &[&s.a, &s.b], false).await.unwrap();
    drive(&s.user, &s.a, STOP).await;
    s.user
        .request("rail.remove", json!({ "id": s.b }))
        .await
        .unwrap();
    let note = notes(&s.user, &s.w, "[wake]", 1).await;
    assert!(
        {
            let body = note[0]["body"].as_str().unwrap();
            body.contains("beta is done") || body.contains(&format!("{} is done", s.b))
        },
        "{note:?}"
    );
}

#[tokio::test]
async fn b41_refusals_save_nothing() {
    let s = scene(&[]).await;
    let code_of = |result: Result<Value, rpc::RpcError>| result.unwrap_err().code;
    let ids: Vec<String> = (0..21).map(|n| format!("x{n}")).collect();
    let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
    assert_eq!(code_of(ask(&s.user, &[&s.a], false).await), code::FORBIDDEN);
    assert_eq!(
        code_of(ask(&s.as_w, &["no-such"], false).await),
        code::NOT_FOUND
    );
    for refused in [
        vec![],
        vec![s.w.as_str()],
        vec![s.a.as_str(), s.a.as_str()],
        ids,
    ] {
        assert_eq!(
            code_of(ask(&s.as_w, &refused, false).await),
            code::INVALID_PARAMS,
            "{refused:?}"
        );
    }
    // None of those was saved: all eight allowed requests still fit, and the ninth does not.
    for _ in 0..8 {
        ask(&s.as_w, &[&s.a], false).await.unwrap();
    }
    assert_eq!(code_of(ask(&s.as_w, &[&s.a], false).await), code::CONFLICT);
}

#[tokio::test]
async fn b41_no_timer_is_set() {
    let clock = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(clock.path(), "0").unwrap();
    let s = scene(&[("RUPD_MESSAGES_CLOCK", clock.path().to_str().unwrap())]).await;
    let before = rail_tree(&s.user).await;
    ask(&s.as_w, &[&s.a, &s.b], false).await.unwrap();
    std::fs::write(clock.path(), (24 * 3600 * 1000).to_string()).unwrap();
    s.user
        .request("todo.create", json!({ "title": "wakes the Daemon up" }))
        .await
        .unwrap();
    still(&s.user, &s.w, "[wake]", 0).await;
    let after = rail_tree(&s.user).await;
    let kinds = |nodes: &[RailNode]| -> Vec<String> {
        nodes
            .iter()
            .map(|n| format!("{} {:?}", n.id, n.status.as_ref().map(|s| s.kind)))
            .collect()
    };
    assert_eq!(kinds(&before), kinds(&after));
}

#[tokio::test]
async fn b41_a_request_ends_with_its_agent() {
    let s = scene(&[]).await;
    ask(&s.as_w, &[&s.a], false).await.unwrap();
    s.user
        .request("rail.remove", json!({ "id": s.w }))
        .await
        .unwrap();
    drive(&s.user, &s.a, STOP).await;
    still(&s.user, &s.w, "[wake]", 0).await;
    let all = s.user.request("message.list", json!({})).await.unwrap();
    assert!(
        all.as_array()
            .unwrap()
            .iter()
            .all(|m| !m["body"].as_str().unwrap().starts_with("[wake]")),
        "{all}"
    );
}

#[tokio::test]
async fn b42_a_firing_creates_one_note_with_each_agents_kind_and_last() {
    let s = scene(&[]).await;
    ask(&s.as_w, &[&s.b, &s.a], false).await.unwrap();
    drive(&s.user, &s.a, STOP).await;
    drive(&s.user, &s.b, FAIL).await;
    let note = &notes(&s.user, &s.w, "[wake]", 1).await[0];
    assert_eq!(note["kind"], "note");
    assert_eq!(note["from"]["id"], "rupd");
    let body = note["body"].as_str().unwrap();
    let (a, b) = (name_of(&s.user, &s.a).await, name_of(&s.user, &s.b).await);
    let at = |name: &str| body.find(&format!("{name} is ")).unwrap();
    assert!(at(&b) < at(&a), "in the order named: {body}");
    assert!(body.contains(&format!("{b} is error\n  last: ")), "{body}");
    assert!(body.lines().all(|line| line.len() <= 512));
}

#[tokio::test]
async fn b42_a_fired_request_never_fires_again() {
    let s = scene(&[]).await;
    ask(&s.as_w, &[&s.a, &s.b], false).await.unwrap();
    drive(&s.user, &s.a, STOP).await;
    drive(&s.user, &s.b, STOP).await;
    notes(&s.user, &s.w, "[wake]", 1).await;
    for id in [&s.a, &s.b] {
        drive(&s.user, id, WORK).await;
        drive(&s.user, id, STOP).await;
    }
    still(&s.user, &s.w, "[wake]", 1).await;
}

#[tokio::test]
async fn b42_the_note_and_the_removal_are_one_write() {
    let fault = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(fault.path(), "wake").unwrap();
    let s = scene(&[("RUPD_MESSAGES_FAULT_FILE", fault.path().to_str().unwrap())]).await;
    ask(&s.as_w, &[&s.a], false).await.unwrap();
    drive(&s.user, &s.a, STOP).await;
    // The fault between the note and the removal leaves neither.
    still(&s.user, &s.w, "[wake]", 0).await;
    std::fs::write(fault.path(), "").unwrap();
    // Any later Event decides the request again: the note is made once, with the request gone.
    drive(&s.user, &s.a, WORK).await;
    drive(&s.user, &s.a, STOP).await;
    notes(&s.user, &s.w, "[wake]", 1).await;
    still(&s.user, &s.w, "[wake]", 1).await;
}

#[tokio::test]
async fn b42_a_flood_of_firings_is_not_capped_by_b22() {
    let s = scene(&[]).await;
    drive(&s.user, &s.a, STOP).await;
    for _ in 0..10 {
        assert_eq!(ask(&s.as_w, &[&s.a], false).await.unwrap()["fired"], true);
    }
    assert_eq!(notes(&s.user, &s.w, "[wake]", 10).await.len(), 10);
}

#[tokio::test]
async fn b43_error_and_needs_you_reach_a_non_door_parent() {
    let s = scene(&[]).await;
    let c = agent_in_as(&s.project, &s.w).await;
    let n = name_of(&s.user, &c).await;
    drive(&s.user, &c, FAIL).await;
    let note = &notes(&s.user, &s.w, "[digest]", 1).await[0];
    assert!(
        note["body"]
            .as_str()
            .unwrap()
            .contains(&format!("{n} is error")),
        "{note}"
    );
    let d = agent_in_as(&s.project, &s.w).await;
    let _ask = h21_ask(&s.project, &d, "Bash", h21_bash()).await;
    let n = name_of(&s.user, &d).await;
    h21_poll("the needs-you note", async || {
        let found = messages(&s.user, &s.w, "[digest]").await;
        found
            .iter()
            .any(|m| {
                m["body"]
                    .as_str()
                    .unwrap()
                    .contains(&format!("{n} is needs-you"))
            })
            .then_some(())
    })
    .await;
}

#[tokio::test]
async fn b43_the_note_is_not_held_behind_an_all_request() {
    let s = scene(&[]).await;
    let c1 = agent_in_as(&s.project, &s.w).await;
    let c2 = agent_in_as(&s.project, &s.w).await;
    drive(&s.user, &c2, WORK).await;
    ask(&s.as_w, &[&c1, &c2], false).await.unwrap();
    drive(&s.user, &c1, FAIL).await;
    notes(&s.user, &s.w, "[digest]", 1).await;
    still(&s.user, &s.w, "[wake]", 0).await;
}

#[tokio::test]
async fn b43_one_note_not_two_for_a_door() {
    let s = scene(&[]).await;
    drive(&s.user, &s.a, FAIL).await;
    let a = name_of(&s.user, &s.a).await;
    notes(&s.user, &s.door, &format!("[digest] {a} is error"), 1).await;
    // Let any second push come.
    tokio::time::sleep(Duration::from_millis(600)).await;
    let pushes = messages(&s.user, &s.door, "[digest]").await;
    let errors = pushes
        .iter()
        .filter(|m| {
            m["body"]
                .as_str()
                .unwrap()
                .contains(&format!("{a} is error"))
        })
        .count();
    assert_eq!(errors, 1, "{pushes:?}");
}

#[tokio::test]
async fn b43_a_change_that_also_fires_a_wake_sends_only_the_wake_note() {
    let s = scene(&[]).await;
    let c = agent_in_as(&s.project, &s.w).await;
    drive(&s.user, &c, WORK).await;
    ask(&s.as_w, &[&c], true).await.unwrap();
    drive(&s.user, &c, FAIL).await;
    notes(&s.user, &s.w, "[wake]", 1).await;
    still(&s.user, &s.w, "[digest]", 0).await;
}

#[tokio::test]
async fn b43_the_note_is_outside_b22s_caps() {
    let s = scene(&[]).await;
    let mut children = Vec::new();
    for _ in 0..10 {
        children.push(agent_in_as(&s.project, &s.w).await);
    }
    for c in &children {
        drive(&s.user, c, FAIL).await;
    }
    assert_eq!(notes(&s.user, &s.w, "[digest]", 10).await.len(), 10);
}

/// The lines of the fake `claude`'s prompt log that took up an inbox entry (B50), once `least` are there.
pub(super) async fn typed(log: &std::path::Path, least: usize) -> Vec<Value> {
    h21_poll("the typed prompts", async || {
        let lines: Vec<Value> = std::fs::read_to_string(log)
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|line| line["inbox"].is_string())
            .collect();
        (lines.len() >= least).then_some(lines)
    })
    .await
}

pub(super) async fn status_of(user: &rpc::Client, id: u64) -> String {
    let message = user
        .request("message.get", json!({ "id": id }))
        .await
        .unwrap();
    message["status"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn b42_the_note_goes_through_the_inbox_at_the_next_idle() {
    let logs = tempfile::tempdir().unwrap();
    let prompts = logs.path().join("prompts.jsonl");
    let s = scene(&[("FAKE_CLAUDE_PROMPT_LOG", prompts.to_str().unwrap())]).await;
    drive(&s.user, &s.a, STOP).await;
    ask(&s.as_w, &[&s.a], false).await.unwrap();
    let note = &notes(&s.user, &s.w, "[wake]", 1).await[0];
    let id = note["id"].as_u64().unwrap();
    let prompt = h21_poll("the wake prompt", async || {
        typed(&prompts, 1)
            .await
            .iter()
            .find(|line| line["prompt"].as_str().unwrap().contains("[wake]"))
            .map(|line| line["prompt"].as_str().unwrap().to_owned())
    })
    .await;
    assert!(
        prompt.contains("[wake]") && prompt.contains("alpha is idle"),
        "{prompt}"
    );
    h21_poll("the note to be delivered", async || {
        (status_of(&s.user, id).await == "delivered").then_some(())
    })
    .await;
}

pub(super) fn project_db(project: &Project) -> rusqlite::Connection {
    rusqlite::Connection::open(project.data_dir().join("project.db")).unwrap()
}

/// An `agent.status` Event as the log saves it: `id` became `kind`.
pub(super) fn finished_event(id: &str, kind: contracts::Kind) -> String {
    serde_json::to_string(&contracts::Event {
        seq: None,
        actor: contracts::Actor::daemon(),
        data: contracts::EventData::AgentStatus(Box::new(contracts::agent::StatusEvent {
            id: id.into(),
            attempt: "1".into(),
            status_revision: "99".into(),
            status: contracts::Status {
                kind,
                label: "finished".into(),
                since: 0,
            },
        })),
    })
    .unwrap()
}

/// Waiter `w` has an open request on `ids`, saved as the Daemon saves it, decided up to `seen`.
pub(super) fn save_request(project: &Project, waiter: &str, ids: &[&str], seen: u64) {
    project_db(project)
        .execute(
            "INSERT INTO wakes (waiter, any_of, ids, finished, seen) VALUES (?1, 0, ?2, '[]', ?3)",
            rusqlite::params![waiter, json!(ids).to_string(), seen as i64],
        )
        .unwrap();
}

pub(super) fn head(project: &Project) -> u64 {
    let top: i64 = project_db(project)
        .query_row("SELECT MAX(id) FROM events", [], |row| row.get(0))
        .unwrap();
    top as u64
}

/// The Door comes back after a restart, as the user starts it.
pub(super) async fn start_again(user: &rpc::Client, door: &str) {
    user.request("rail.startDoor", json!({ "id": door }))
        .await
        .unwrap();
}

#[tokio::test]
async fn b44_a_finish_while_the_daemon_was_down_fires_once_after_the_restart() {
    let mut s = scene(&[]).await;
    let seen = head(&s.project);
    s.project.crash();
    // The finishing Event was saved, and the Daemon went down before it decided the request.
    let db = project_db(&s.project);
    db.execute(
        "INSERT INTO events (event) VALUES (?1)",
        [finished_event(&s.a, contracts::Kind::Idle)],
    )
    .unwrap();
    save_request(&s.project, &s.door, &[&s.a], seen);
    drop(db);
    s.project.restart(&[]);
    let user = s.project.client().await;
    start_again(&user, &s.door).await;
    assert_eq!(notes(&user, &s.door, "[wake]", 1).await.len(), 1);
    s.project.restart(&[]);
    let user = s.project.client().await;
    still(&user, &s.door, "[wake]", 1).await;
}

#[tokio::test]
async fn b44_a_fired_request_does_not_fire_again_after_a_kill() {
    let mut s = scene(&[]).await;
    let as_door = client_as(&s.project, actor(ActorKind::Agent, &s.door)).await;
    ask(&as_door, &[&s.a, &s.b], false).await.unwrap();
    drive(&s.user, &s.a, STOP).await;
    drive(&s.user, &s.b, STOP).await;
    notes(&s.user, &s.door, "[wake]", 1).await;
    s.project.restart(&[]);
    let user = s.project.client().await;
    start_again(&user, &s.door).await;
    still(&user, &s.door, "[wake]", 1).await;
}

#[tokio::test]
async fn b44_a_kill_between_the_finish_and_the_decision_fires_once() {
    let mut s = scene(&[]).await;
    drive(&s.user, &s.a, STOP).await;
    let finish = head(&s.project);
    s.project.crash();
    // The request was saved decided up to just before the finish the log holds.
    save_request(&s.project, &s.door, &[&s.a], finish - 1);
    s.project.restart(&[]);
    let user = s.project.client().await;
    start_again(&user, &s.door).await;
    assert_eq!(notes(&user, &s.door, "[wake]", 1).await.len(), 1);
    still(&user, &s.door, "[wake]", 1).await;
}

#[tokio::test]
async fn b44_an_expired_log_falls_back_to_the_present_kinds() {
    let mut s = scene(&[]).await;
    s.project.crash();
    // The log no longer keeps the Event after number 0, but `a` is finished now: late, never lost.
    project_db(&s.project)
        .execute("DELETE FROM events", [])
        .unwrap();
    save_request(&s.project, &s.door, &[&s.a], 0);
    s.project.restart(&[]);
    let user = s.project.client().await;
    start_again(&user, &s.door).await;
    assert_eq!(notes(&user, &s.door, "[wake]", 1).await.len(), 1);
    still(&user, &s.door, "[wake]", 1).await;
}

#[tokio::test]
async fn b44_a_note_in_flight_at_the_kill_is_written_again_not_made_again() {
    let logs = tempfile::tempdir().unwrap();
    let hold = logs.path().join("signal");
    let prompts = logs.path().join("prompts.jsonl");
    let log = prompts.to_str().unwrap();
    let mut project = start(&[
        ("FAKE_CLAUDE_PROMPT_LOG", log),
        ("FAKE_CLAUDE_HOLD_PROMPT", hold.to_str().unwrap()),
    ]);
    let user0 = project.client().await;
    let door = start_door(&user0).await;
    // Not the Door's child, so no digest competes with the note for the one typing.
    let lone = named(&project, &user0, "lone", None).await;
    drive(&user0, &lone, WORK).await;
    let as_door = client_as(&project, actor(ActorKind::Agent, &door)).await;
    ask(&as_door, &[&lone], false).await.unwrap();
    drive(&user0, &lone, STOP).await;
    let id = notes(&user0, &door, "[wake]", 1).await[0]["id"]
        .as_u64()
        .unwrap();
    let first = typed(&prompts, 1).await;
    assert_eq!(status_of(&user0, id).await, "pending");

    project.restart(&[("FAKE_CLAUDE_PROMPT_LOG", log)]);
    let user = project.client().await;
    let m = user
        .request("message.get", json!({ "id": id }))
        .await
        .unwrap();
    assert_eq!(m["status"], "pending", "{m}");
    start_again(&user, &door).await;
    h21_poll("the note to be delivered", async || {
        (status_of(&user, id).await == "delivered").then_some(())
    })
    .await;
    let again = typed(&prompts, 2).await;
    assert_eq!(again[1]["prompt"], first[0]["prompt"]);
    still(&user, &door, "[wake]", 1).await;
}
