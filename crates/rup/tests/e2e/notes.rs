//! B46 to B49 through a real Daemon with the fake `claude`: a note is delivered once, a status flip
//! makes none, a refused wake says why, and an end note carries only the child's own summary.

use std::path::Path;
use std::time::Duration;

use contracts::ActorKind;
use serde_json::{Value, json};

use super::daemon::{h21_ask, h21_bash, h21_poll, start};
use super::wake::{
    FAIL, STOP, WORK, ask, drive, named, notes, scene, start_again, status_of, typed,
};
use super::{actor, client_as};

fn env(prompts: &Path) -> [(&str, &str); 1] {
    [("FAKE_CLAUDE_PROMPT_LOG", prompts.to_str().unwrap())]
}

/// How many times the fake `claude` of the run took up the inbox entry of Message `id` (B50).
fn inbox_takes_of(prompts: &Path, id: u64) -> usize {
    let entry = format!("rup-{id}");
    std::fs::read_to_string(prompts)
        .unwrap_or_default()
        .lines()
        .filter(|l| serde_json::from_str::<Value>(l).unwrap()["inbox"] == entry)
        .count()
}

/// How many `message.delivered` Events for `id` the client received, after the Daemon had every
/// chance to send more.
async fn delivered_events(events: &mut rpc::Client, id: u64) -> usize {
    let mut count = 0;
    while let Ok(Some(event)) =
        tokio::time::timeout(Duration::from_millis(700), events.next_event()).await
    {
        if let contracts::EventData::MessageDelivered(m) = event.data
            && u64::from(m.id) == id
        {
            count += 1;
        }
    }
    count
}

async fn delivered_messages(user: &rpc::Client, to: &str, id: u64) -> usize {
    let listed = user
        .request("message.list", json!({ "to": to }))
        .await
        .unwrap();
    listed
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["id"] == id && m["status"] == "delivered")
        .count()
}

#[tokio::test]
async fn b46_thirty_idle_edges_deliver_a_pending_note_once() {
    let logs = tempfile::tempdir().unwrap();
    let prompts = logs.path().join("prompts.jsonl");
    let s = scene(&env(&prompts)).await;
    let mut events = s.project.subscribed().await;
    drive(&s.user, &s.a, STOP).await;
    ask(&s.as_w, &[&s.a], false).await.unwrap();
    let id = notes(&s.user, &s.w, "[wake]", 1).await[0]["id"]
        .as_u64()
        .unwrap();
    h21_poll("the note to be delivered", async || {
        (status_of(&s.user, id).await == "delivered").then_some(())
    })
    .await;
    for _ in 0..30 {
        drive(&s.user, &s.w, WORK).await;
        drive(&s.user, &s.w, STOP).await;
    }
    assert_eq!(delivered_events(&mut events, id).await, 1);
    assert_eq!(delivered_messages(&s.user, &s.w, id).await, 1);
    assert_eq!(inbox_takes_of(&prompts, id), 1);
}

#[tokio::test]
async fn b46_a_restart_with_a_note_pending_delivers_it_once() {
    let logs = tempfile::tempdir().unwrap();
    let prompts = logs.path().join("prompts.jsonl");
    let mut project = start(&env(&prompts));
    let user0 = project.client().await;
    let door = super::start_door(&user0).await;
    // The Door's own first turn ends before it is made to work.
    h21_poll("the Door's first prompt", async || {
        std::fs::read_to_string(&prompts)
            .ok()
            .filter(|log| !log.is_empty())
    })
    .await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    // A working Door takes nothing, so the note waits for its next idle.
    drive(&user0, &door, WORK).await;
    let lone = named(&project, &user0, "lone", None).await;
    drive(&user0, &lone, WORK).await;
    let as_door = client_as(&project, actor(ActorKind::Agent, &door)).await;
    ask(&as_door, &[&lone], false).await.unwrap();
    drive(&user0, &lone, STOP).await;
    let id = notes(&user0, &door, "[wake]", 1).await[0]["id"]
        .as_u64()
        .unwrap();
    assert_eq!(status_of(&user0, id).await, "pending");
    assert_eq!(inbox_takes_of(&prompts, id), 0);

    project.restart(&env(&prompts));
    let user = project.client().await;
    let mut events = project.subscribed().await;
    start_again(&user, &door).await;
    h21_poll("the note to be delivered", async || {
        (status_of(&user, id).await == "delivered").then_some(())
    })
    .await;
    assert_eq!(delivered_events(&mut events, id).await, 1);
    assert_eq!(delivered_messages(&user, &door, id).await, 1);
    assert_eq!(inbox_takes_of(&prompts, id), 1);
}

#[tokio::test]
async fn b46_a_restart_with_a_note_in_flight_delivers_it_once() {
    let logs = tempfile::tempdir().unwrap();
    let hold = logs.path().join("signal");
    let prompts = logs.path().join("prompts.jsonl");
    let mut project = start(&[
        ("FAKE_CLAUDE_PROMPT_LOG", prompts.to_str().unwrap()),
        ("FAKE_CLAUDE_HOLD_PROMPT", hold.to_str().unwrap()),
    ]);
    let user0 = project.client().await;
    let door = super::start_door(&user0).await;
    let lone = named(&project, &user0, "lone", None).await;
    drive(&user0, &lone, WORK).await;
    let as_door = client_as(&project, actor(ActorKind::Agent, &door)).await;
    ask(&as_door, &[&lone], false).await.unwrap();
    drive(&user0, &lone, STOP).await;
    let id = notes(&user0, &door, "[wake]", 1).await[0]["id"]
        .as_u64()
        .unwrap();
    typed(&prompts, 1).await;
    assert_eq!(status_of(&user0, id).await, "pending");
    let before = inbox_takes_of(&prompts, id);

    project.restart(&env(&prompts));
    let user = project.client().await;
    let mut events = project.subscribed().await;
    start_again(&user, &door).await;
    h21_poll("the note to be delivered", async || {
        (status_of(&user, id).await == "delivered").then_some(())
    })
    .await;
    assert_eq!(delivered_events(&mut events, id).await, 1);
    assert_eq!(delivered_messages(&user, &door, id).await, 1);
    assert_eq!(inbox_takes_of(&prompts, id) - before, 1);
}

#[tokio::test]
async fn b46_every_note_has_its_own_id() {
    let s = scene(&[]).await;
    drive(&s.user, &s.a, STOP).await;
    for _ in 0..3 {
        ask(&s.as_w, &[&s.a], false).await.unwrap();
    }
    let found = notes(&s.user, &s.w, "[wake]", 3).await;
    let mut ids: Vec<u64> = found.iter().map(|m| m["id"].as_u64().unwrap()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 3);
}

/// Every Message to `to` the Daemon made, as bodies.
async fn made(user: &rpc::Client, to: &str) -> Vec<String> {
    let listed = user
        .request("message.list", json!({ "to": to }))
        .await
        .unwrap();
    listed
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["from"]["id"] == "rupd")
        .map(|m| m["body"].as_str().unwrap().to_owned())
        .collect()
}

fn count(bodies: &[String], head: &str) -> usize {
    bodies.iter().filter(|b| b.starts_with(head)).count()
}

/// The name `id` has on the Rail.
async fn name_of(user: &rpc::Client, id: &str) -> String {
    super::daemon::rail_tree(user)
        .await
        .into_iter()
        .find(|node| node.id == id)
        .unwrap()
        .name
}

#[tokio::test]
async fn b47_a_replay_of_status_flips_starts_no_door_turn() {
    let logs = tempfile::tempdir().unwrap();
    let prompts = logs.path().join("prompts.jsonl");
    let s = scene(&env(&prompts)).await;
    // The Door's own first turn is over before the flips are counted.
    h21_poll("the Door's first prompt", async || {
        std::fs::read_to_string(&prompts)
            .ok()
            .filter(|log| !log.is_empty())
    })
    .await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    // With a `last` of its own (B35), the child's flips change no entry.
    let as_a = client_as(&s.project, actor(ActorKind::Agent, &s.a)).await;
    as_a.request(
        "message.send",
        json!({ "to": s.door, "kind": "note", "body": "found it" }),
    )
    .await
    .unwrap();
    drive(&s.user, &s.a, STOP).await;
    drive(&s.user, &s.a, WORK).await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let before = (made(&s.user, &s.door).await, typed(&prompts, 0).await.len());
    for _ in 0..5 {
        drive(&s.user, &s.a, STOP).await;
        drive(&s.user, &s.a, WORK).await;
    }
    tokio::time::sleep(Duration::from_secs(1)).await;
    let after = (made(&s.user, &s.door).await, typed(&prompts, 0).await.len());
    assert_eq!(after.0, before.0);
    assert_eq!(after.1, before.1);
}

#[tokio::test]
async fn b47_a_note_is_made_for_a_wake_needs_you_error_or_end_and_a_status_flip_makes_none() {
    let s = scene(&[]).await;
    let as_door = client_as(&s.project, actor(ActorKind::Agent, &s.door)).await;
    let err = named(&s.project, &s.user, "erring", Some(&s.door)).await;
    let need = named(&s.project, &s.user, "needing", Some(&s.door)).await;
    let end = named(&s.project, &s.user, "ending", Some(&s.door)).await;
    ask(&as_door, &[&s.a], true).await.unwrap();
    drive(&s.user, &s.a, STOP).await;
    drive(&s.user, &err, FAIL).await;
    let _ask = h21_ask(&s.project, &need, "Bash", h21_bash()).await;
    s.user
        .request("agent.stop", json!({ "id": end }))
        .await
        .unwrap();
    let (e, n, d) = (
        name_of(&s.user, &err).await,
        name_of(&s.user, &need).await,
        name_of(&s.user, &end).await,
    );
    h21_poll("the four notes", async || {
        let bodies = made(&s.user, &s.door).await;
        (count(&bodies, "[wake]") == 1
            && count(&bodies, &format!("[digest] {e} is error")) == 1
            && count(&bodies, &format!("[digest] {n} is needs-you")) == 1
            && count(&bodies, &format!("[digest] {d} is done")) == 1)
            .then_some(())
    })
    .await;
    // The first flip of `b` changes its `last` (B35); the replay after it adds none.
    drive(&s.user, &s.b, STOP).await;
    drive(&s.user, &s.b, WORK).await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    let bodies = made(&s.user, &s.door).await;
    for _ in 0..5 {
        drive(&s.user, &s.b, STOP).await;
        drive(&s.user, &s.b, WORK).await;
    }
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert_eq!(made(&s.user, &s.door).await.len(), bodies.len());
}

#[tokio::test]
async fn b47_the_same_agent_and_kind_twice_is_one_note() {
    let s = scene(&[]).await;
    let a = name_of(&s.user, &s.a).await;
    let head = format!("[digest] {a} is error");
    drive(&s.user, &s.a, FAIL).await;
    h21_poll("the first note", async || {
        (count(&made(&s.user, &s.door).await, &head) == 1).then_some(())
    })
    .await;
    drive(&s.user, &s.a, WORK).await;
    drive(&s.user, &s.a, FAIL).await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert_eq!(count(&made(&s.user, &s.door).await, &head), 1);
}

#[tokio::test]
async fn b47_the_same_kind_after_another_kind_is_a_new_note() {
    let s = scene(&[]).await;
    let a = name_of(&s.user, &s.a).await;
    let error = format!("[digest] {a} is error");
    let needs = format!("[digest] {a} is needs-you");
    drive(&s.user, &s.a, FAIL).await;
    h21_poll("the error note", async || {
        (count(&made(&s.user, &s.door).await, &error) == 1).then_some(())
    })
    .await;
    let ask = h21_ask(&s.project, &s.a, "Bash", h21_bash()).await;
    h21_poll("the needs-you note", async || {
        (count(&made(&s.user, &s.door).await, &needs) == 1).then_some(())
    })
    .await;
    let decision = super::daemon::h21_decision(&s.user, &s.a, None).await;
    super::daemon::h21_user_answers(&s.user, decision["id"].as_str().unwrap(), "allow")
        .await
        .unwrap();
    let _ = ask.await;
    drive(&s.user, &s.a, FAIL).await;
    h21_poll("the second error note", async || {
        (count(&made(&s.user, &s.door).await, &error) == 2).then_some(())
    })
    .await;
}

#[tokio::test]
async fn b48_a_wake_that_fires_nothing_says_already_working() {
    let s = scene(&[]).await;
    // An `all` request with one finished and one working Agent.
    drive(&s.user, &s.a, STOP).await;
    let all = ask(&s.as_w, &[&s.a, &s.b], false).await.unwrap();
    assert_eq!(all["fired"], false);
    assert_eq!(all["why"], "already working");
    // An `any` request with only working Agents.
    drive(&s.user, &s.a, WORK).await;
    let any = ask(&s.as_w, &[&s.a, &s.b], true).await.unwrap();
    assert_eq!(any["fired"], false);
    assert_eq!(any["why"], "already working");
    // A request that fires says nothing of why.
    drive(&s.user, &s.a, STOP).await;
    let fired = ask(&s.as_w, &[&s.a], false).await.unwrap();
    assert_eq!(fired["fired"], true);
    assert!(fired.get("why").is_none(), "{fired}");
}

#[tokio::test]
async fn b48_the_mcp_tool_returns_why() {
    let s = scene(&[]).await;
    let mut shim = super::Shim::start(&s.project, &s.w).await;
    let reply = shim
        .call("agent_wake", json!({ "ids": [s.a, s.b], "any": false }))
        .await;
    assert_eq!(reply["fired"], false);
    assert_eq!(reply["why"], "already working");
}

#[tokio::test]
async fn b48_the_message_list_and_the_digest_return_equal_state_after_the_same_events() {
    let s = scene(&[]).await;
    let as_a = client_as(&s.project, actor(ActorKind::Agent, &s.a)).await;
    as_a.request(
        "message.send",
        json!({ "to": s.door, "kind": "note", "body": "found it" }),
    )
    .await
    .unwrap();
    as_a.request("todo.create", json!({ "title": "check" }))
        .await
        .unwrap();
    let name = super::daemon::rail_tree(&s.user)
        .await
        .into_iter()
        .find(|node| node.id == s.a)
        .unwrap()
        .name;
    let pushed = h21_poll("the push", async || {
        let listed = s
            .user
            .request("message.list", json!({ "to": s.door }))
            .await
            .unwrap();
        listed
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["from"]["id"] == "rupd")
            .filter_map(|m| {
                Some(super::entry_of(
                    m["body"].as_str().unwrap().strip_prefix("[digest] ")?,
                ))
            })
            .rfind(|e| e["name"] == name.as_str() && e["kind"] == "working" && e["todos"] == 1)
    })
    .await;
    let digest = super::digest_of(&s.user, &s.door).await.unwrap();
    let read = digest["children"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name.as_str())
        .unwrap();
    for field in ["kind", "last", "todos", "pads"] {
        assert_eq!(read[field], pushed[field], "{field}");
    }
    assert_eq!(read["last"], "found it");
}

/// A child `finder` of the Door that sends `summary` to it (when there is one), and ends. Returns
/// the body of the Door's end note about it.
async fn end_note(
    s: &super::wake::Scene,
    summary: Option<&str>,
    terminal_text: Option<&str>,
) -> String {
    let child = named(&s.project, &s.user, "finder", Some(&s.door)).await;
    drive(&s.user, &child, WORK).await;
    if let Some(text) = terminal_text {
        let terminal = super::daemon::rail_tree(&s.user)
            .await
            .into_iter()
            .find(|node| node.id == child)
            .unwrap()
            .terminal_id
            .unwrap();
        use base64::Engine;
        s.user
            .request(
                "terminal.write",
                json!({ "id": terminal, "data": base64::engine::general_purpose::STANDARD.encode(format!("{text}\n")) }),
            )
            .await
            .unwrap();
    }
    if let Some(text) = summary {
        let as_child = client_as(&s.project, actor(ActorKind::Agent, &child)).await;
        as_child
            .request(
                "message.send",
                json!({ "to": s.door, "kind": "note", "body": text }),
            )
            .await
            .unwrap();
    }
    s.user
        .request("agent.stop", json!({ "id": child }))
        .await
        .unwrap();
    h21_poll("the end note", async || {
        made(&s.user, &s.door)
            .await
            .into_iter()
            .find(|body| body.starts_with("[digest] finder is done"))
    })
    .await
}

/// The entry of a pushed digest without the `(n changes)` line of a folded one.
fn entry_text_of(body: &str) -> String {
    let entry = body.strip_prefix("[digest] ").unwrap();
    entry
        .lines()
        .filter(|line| !line.starts_with("  ("))
        .collect::<Vec<_>>()
        .join("\n")
}

fn last_of(body: &str) -> String {
    super::entry_of(&entry_text_of(body))["last"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn b49_a_5000_byte_summary_gives_a_note_of_at_most_600_bytes() {
    let s = scene(&[]).await;
    let note = end_note(&s, Some(&"a".repeat(5000)), None).await;
    let last = last_of(&note);
    assert!(
        last.len() <= 600 && last.ends_with("[cut]"),
        "{}",
        last.len()
    );
    assert!(last.len() > 500, "{}", last.len());
    assert!(entry_text_of(&note).len() <= 1024, "{}", note.len());
}

#[tokio::test]
async fn b49_a_short_summary_is_carried_whole() {
    let s = scene(&[]).await;
    let note = end_note(&s, Some("fixed it, see the pad notes"), None).await;
    assert_eq!(last_of(&note), "fixed it, see the pad notes");
}

#[tokio::test]
async fn b49_a_child_that_ends_with_no_summary_gives_ended_with_no_finding() {
    let s = scene(&[]).await;
    let note = end_note(&s, None, None).await;
    assert_eq!(last_of(&note), "ended with no finding");
}

#[tokio::test]
async fn b49_the_note_never_holds_the_terminal_reply() {
    let s = scene(&[]).await;
    let note = end_note(&s, None, Some("ZEBRA")).await;
    assert!(!note.contains("ZEBRA"), "{note}");
    assert_eq!(last_of(&note), "ended with no finding");
}
