//! H2, H3, H4, H5, H6, H7, H9 and H10: the Decision store, driven through the calls `rup
//! permission` and the webview make. A dropped `agent.permission` future is the hook's connection
//! closing. Written by an architect before the store; the Builder makes these pass and never
//! edits them.
//!
//! Pinned handle (the one name these tests need that is not in the contract): `Agents::with_proof
//! (self, proof: &str) -> Agents`, the Daemon's one `proof` value (H4). Without it every
//! `decision.answer` is `FORBIDDEN`.

use std::sync::Arc;
use std::time::Duration;

use contracts::decision::{ClearedEvent, Decision, Outcome};
use contracts::{Actor, ActorKind, Event, EventData, Kind};
use rpc::{Ctx, Module, RpcError, code};
use serde_json::{Value, json};
use tokio::sync::broadcast::Receiver;
use tokio::task::JoinHandle;

use crate::common::{Fixture, status_of};

const PROOF: &str = "proof-held-by-the-app";
const PATIENCE: Duration = Duration::from_secs(10);

const ALLOW_REPLY: &str = r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}"#;

fn proven(mut f: Fixture) -> Arc<Fixture> {
    f.agents = f.agents.with_proof(PROOF);
    Arc::new(f)
}

fn payload(tool: &str, input: Value) -> Value {
    json!({"hook_event_name": "PermissionRequest", "tool_name": tool, "tool_input": input})
}

fn bash(command: &str) -> Value {
    payload("Bash", json!({"command": command}))
}

/// `rup permission` for `agent`: the call stays open until the Decision is answered or cleared.
fn hook(f: &Arc<Fixture>, agent: &str, payload: Value) -> JoinHandle<Result<Value, RpcError>> {
    let (f, agent) = (Arc::clone(f), agent.to_owned());
    tokio::spawn(async move {
        f.call("agent.permission", json!({"id": agent, "payload": payload}))
            .await
    })
}

async fn finished(call: JoinHandle<Result<Value, RpcError>>) -> Result<Value, RpcError> {
    tokio::time::timeout(PATIENCE, call)
        .await
        .expect("the call returns in time")
        .unwrap()
}

async fn open(f: &Fixture) -> Vec<Decision> {
    serde_json::from_value(f.call("decision.list", Value::Null).await.unwrap()).unwrap()
}

/// The open Decisions, once there are `n` of them.
async fn until_open(f: &Fixture, n: usize) -> Vec<Decision> {
    tokio::time::timeout(PATIENCE, async {
        loop {
            let found = open(f).await;
            if found.len() == n {
                return found;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{n} Decisions open in time"))
}

async fn answer(f: &Fixture, id: &str, word: &str, proof: Option<&str>) -> Result<Value, RpcError> {
    f.call(
        "decision.answer",
        json!({"id": id, "answer": word, "proof": proof}),
    )
    .await
}

fn cleared(events: &mut Receiver<Event>) -> Vec<ClearedEvent> {
    drained(events)
        .into_iter()
        .filter_map(|event| match event.data {
            EventData::DecisionCleared(done) => Some(done),
            _ => None,
        })
        .collect()
}

fn drained(events: &mut Receiver<Event>) -> Vec<Event> {
    std::iter::from_fn(|| events.try_recv().ok()).collect()
}

async fn kind_of(f: &Fixture, agent: &str) -> Kind {
    status_of(&f.tree().await, agent).kind
}

async fn until_kind(f: &Fixture, agent: &str, kind: Kind) {
    f.until(|tree| {
        tree.iter()
            .any(|n| n.id == agent && status_of(tree, agent).kind == kind)
    })
    .await;
}

#[tokio::test]
async fn h2_a_permission_request_opens_one_decision() {
    let f = proven(Fixture::running("sleep 30"));
    let mut events = f.bus.subscribe();
    let node = f.spawn(None, None).await.unwrap();
    let call = hook(&f, &node.id, bash("touch a.txt"));

    let decisions = until_open(&f, 1).await;

    let opened: Vec<Decision> = drained(&mut events)
        .into_iter()
        .filter_map(|e| match e.data {
            EventData::DecisionOpened(d) => Some(d),
            _ => None,
        })
        .collect();
    assert_eq!(opened, decisions);
    let d = &decisions[0];
    assert_eq!(
        (d.agent.as_str(), d.tool.as_str()),
        (node.id.as_str(), "Bash")
    );
    assert!(d.args.contains("touch a.txt") && d.answerable);
    until_kind(&f, &node.id, Kind::NeedsYou).await;
    assert_eq!(
        status_of(&f.tree().await, &node.id).label,
        "Bash: touch a.txt"
    );
    call.abort();
}

#[tokio::test]
async fn h2_args_are_cut_at_2000_characters_with_a_final_ellipsis() {
    let f = proven(Fixture::running("sleep 30"));
    let node = f.spawn(None, None).await.unwrap();
    let call = hook(&f, &node.id, bash(&"x".repeat(2500)));

    let d = until_open(&f, 1).await.remove(0);

    assert_eq!(d.args.chars().count(), 2000);
    assert!(d.args.ends_with('…'));
    call.abort();
}

#[tokio::test]
async fn h2_ask_user_question_opens_an_unanswerable_decision_and_returns_at_once() {
    let f = proven(Fixture::running("sleep 30"));
    let node = f.spawn(None, None).await.unwrap();
    let input = json!({"questions": [{"question": "Which one?", "options": []}]});

    let reply = finished(hook(&f, &node.id, payload("AskUserQuestion", input))).await;

    assert_eq!(reply.unwrap(), json!({"output": ""}));
    let d = until_open(&f, 1).await.remove(0);
    assert_eq!((d.tool.as_str(), d.answerable), ("AskUserQuestion", false));
    assert_eq!(status_of(&f.tree().await, &node.id).label, "Which one?");
    let err = answer(&f, &d.id, "allow", Some(PROOF)).await.unwrap_err();
    assert_eq!(err.code, code::INVALID_PARAMS);
}

#[tokio::test]
async fn h2_a_second_request_replaces_the_first() {
    let f = proven(Fixture::running("sleep 30"));
    let mut events = f.bus.subscribe();
    let node = f.spawn(None, None).await.unwrap();
    let first = hook(&f, &node.id, bash("one"));
    let old = until_open(&f, 1).await.remove(0);
    let second = hook(&f, &node.id, bash("two"));

    let err = finished(first).await.unwrap_err();

    assert!(err.message.contains("superseded"), "{}", err.message);
    let now = until_open(&f, 1).await.remove(0);
    assert_ne!(now.id, old.id);
    assert!(now.args.contains("two"));
    let gone = cleared(&mut events);
    assert_eq!(gone.len(), 1);
    assert_eq!(
        (gone[0].id.as_str(), gone[0].outcome),
        (old.id.as_str(), Outcome::Replaced)
    );
    assert_eq!(kind_of(&f, &node.id).await, Kind::NeedsYou);
    second.abort();
}

#[tokio::test]
async fn h3_an_allow_prints_the_recorded_reply_and_the_agent_works_again() {
    let f = proven(Fixture::running("sleep 30"));
    let mut events = f.bus.subscribe();
    let node = f.spawn(None, None).await.unwrap();
    let call = hook(&f, &node.id, bash("touch a.txt"));
    let d = until_open(&f, 1).await.remove(0);

    let reply = answer(&f, &d.id, "allow", Some(PROOF)).await.unwrap();

    assert_eq!(reply, Value::Null);
    assert_eq!(
        finished(call).await.unwrap(),
        json!({"output": ALLOW_REPLY})
    );
    assert!(open(&f).await.is_empty());
    let gone = cleared(&mut events);
    assert_eq!((gone.len(), gone[0].outcome), (1, Outcome::Allow));
    assert_eq!(kind_of(&f, &node.id).await, Kind::Working);
}

#[tokio::test]
async fn h3_a_deny_prints_a_deny_reply() {
    let f = proven(Fixture::running("sleep 30"));
    let mut events = f.bus.subscribe();
    let node = f.spawn(None, None).await.unwrap();
    let call = hook(&f, &node.id, bash("rm -rf x"));
    let d = until_open(&f, 1).await.remove(0);

    answer(&f, &d.id, "deny", Some(PROOF)).await.unwrap();

    let out = finished(call).await.unwrap()["output"]
        .as_str()
        .unwrap()
        .to_owned();
    let reply: Value = serde_json::from_str(&out).unwrap();
    let decision = &reply["hookSpecificOutput"];
    assert_eq!(decision["hookEventName"], "PermissionRequest");
    assert_eq!(decision["decision"]["behavior"], "deny");
    let gone = cleared(&mut events);
    assert_eq!((gone.len(), gone[0].outcome), (1, Outcome::Deny));
}

#[tokio::test]
async fn h3_a_bad_id_or_a_bad_word_is_refused_and_leaves_the_decision_open() {
    let f = proven(Fixture::running("sleep 30"));
    let node = f.spawn(None, None).await.unwrap();
    let call = hook(&f, &node.id, bash("a"));
    let d = until_open(&f, 1).await.remove(0);

    let unknown = answer(&f, "nope", "allow", Some(PROOF)).await.unwrap_err();
    let word = answer(&f, &d.id, "ask", Some(PROOF)).await.unwrap_err();

    assert_eq!(unknown.code, code::NOT_FOUND);
    assert_eq!(word.code, code::INVALID_PARAMS);
    assert_eq!(open(&f).await.len(), 1);
    call.abort();
}

#[tokio::test]
async fn h3_a_cleared_decision_is_not_found() {
    let f = proven(Fixture::running("sleep 30"));
    let node = f.spawn(None, None).await.unwrap();
    let call = hook(&f, &node.id, bash("a"));
    let d = until_open(&f, 1).await.remove(0);
    answer(&f, &d.id, "allow", Some(PROOF)).await.unwrap();
    finished(call).await.unwrap();

    let again = answer(&f, &d.id, "deny", Some(PROOF)).await.unwrap_err();

    assert_eq!(again.code, code::NOT_FOUND);
}

#[tokio::test]
async fn h4_a_missing_or_wrong_proof_is_forbidden_and_leaves_the_decision_open() {
    let f = proven(Fixture::running("sleep 30"));
    let node = f.spawn(None, None).await.unwrap();
    let call = hook(&f, &node.id, bash("a"));
    let d = until_open(&f, 1).await.remove(0);

    for proof in [None, Some("wrong"), Some("")] {
        let err = answer(&f, &d.id, "allow", proof).await.unwrap_err();
        assert_eq!(err.code, code::FORBIDDEN, "proof {proof:?}");
    }

    assert_eq!(open(&f).await.len(), 1);
    assert!(!call.is_finished());
    call.abort();
}

#[tokio::test]
async fn h4_an_agent_or_ext_actor_is_forbidden_even_with_the_proof() {
    let f = proven(Fixture::running("sleep 30"));
    let node = f.spawn(None, None).await.unwrap();
    let call = hook(&f, &node.id, bash("a"));
    let d = until_open(&f, 1).await.remove(0);

    for kind in [ActorKind::Agent, ActorKind::Ext] {
        let ctx = Ctx {
            actor: Actor {
                kind,
                id: node.id.clone(),
                parent: None,
            },
            ..f.ctx()
        };
        let params = json!({"id": d.id, "answer": "allow", "proof": PROOF});
        let err = f
            .agents
            .call(&ctx, "decision.answer", params)
            .await
            .unwrap_err();
        assert_eq!(err.code, code::FORBIDDEN, "{kind:?}");
    }

    assert_eq!(open(&f).await.len(), 1);
    call.abort();
}

#[tokio::test]
async fn h4_a_daemon_with_no_proof_forbids_every_answer() {
    let f = Arc::new(Fixture::running("sleep 30"));
    let node = f.spawn(None, None).await.unwrap();
    let call = hook(&f, &node.id, bash("a"));
    let d = until_open(&f, 1).await.remove(0);

    let err = answer(&f, &d.id, "allow", Some(PROOF)).await.unwrap_err();

    assert_eq!(err.code, code::FORBIDDEN);
    call.abort();
}

#[tokio::test]
async fn h4_neither_a_signal_nor_the_clock_nor_a_terminal_write_answers() {
    let f = proven(Fixture::running("cat"));
    let node = f.spawn(None, None).await.unwrap();
    let call = hook(&f, &node.id, bash("a"));
    until_open(&f, 1).await;

    f.call(
        "agent.signal",
        json!({"id": node.id, "payload": {"hook_event_name": "Notification"}}),
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    assert_eq!(open(&f).await.len(), 1);
    assert!(!call.is_finished());
    call.abort();
}

#[tokio::test]
async fn h5_a_second_client_sees_the_same_decision_and_its_answer_reaches_the_hook() {
    let f = proven(Fixture::running("sleep 30"));
    let node = f.spawn(None, None).await.unwrap();
    let call = hook(&f, &node.id, bash("a"));
    let before = until_open(&f, 1).await;

    let other = Ctx {
        bus: rpc::Bus::new(),
        ..f.ctx()
    };
    let seen = f
        .agents
        .call(&other, "decision.list", Value::Null)
        .await
        .unwrap();
    let seen: Vec<Decision> = serde_json::from_value(seen).unwrap();
    let params = json!({"id": seen[0].id, "answer": "allow", "proof": PROOF});
    f.agents
        .call(&other, "decision.answer", params)
        .await
        .unwrap();

    assert_eq!(seen, before);
    assert_eq!(
        finished(call).await.unwrap(),
        json!({"output": ALLOW_REPLY})
    );
}

#[tokio::test]
async fn h6_stopping_the_agent_clears_its_decision_and_fails_the_hook() {
    let f = proven(Fixture::running("sleep 30"));
    let mut events = f.bus.subscribe();
    let node = f.spawn(None, None).await.unwrap();
    let call = hook(&f, &node.id, bash("a"));
    let d = until_open(&f, 1).await.remove(0);

    f.call("agent.stop", json!({"id": node.id})).await.unwrap();

    let err = finished(call).await.unwrap_err();
    assert!(err.message.contains("the Agent ended"), "{}", err.message);
    assert!(open(&f).await.is_empty());
    let gone = cleared(&mut events);
    assert_eq!(gone.len(), 1);
    assert_eq!(
        (gone[0].id.as_str(), gone[0].outcome),
        (d.id.as_str(), Outcome::AgentGone)
    );
}

#[tokio::test]
async fn h7_a_later_signal_clears_the_decision_once_as_a_terminal_answer() {
    for (event, kind) in [("PostToolUse", Kind::Working), ("Stop", Kind::Idle)] {
        let f = proven(Fixture::running("sleep 30"));
        let mut events = f.bus.subscribe();
        let node = f.spawn(None, None).await.unwrap();
        let call = hook(&f, &node.id, bash("a"));
        let d = until_open(&f, 1).await.remove(0);

        f.call(
            "agent.signal",
            json!({"id": node.id, "payload": {"hook_event_name": event}}),
        )
        .await
        .unwrap();

        assert!(open(&f).await.is_empty(), "{event}");
        let gone = cleared(&mut events);
        assert_eq!(gone.len(), 1, "{event}");
        assert_eq!(
            (gone[0].id.as_str(), gone[0].outcome),
            (d.id.as_str(), Outcome::Terminal)
        );
        assert_eq!(kind_of(&f, &node.id).await, kind, "{event}");
        let late = answer(&f, &d.id, "allow", Some(PROOF)).await.unwrap_err();
        assert_eq!(late.code, code::NOT_FOUND);
        finished(call).await.ok();
    }
}

#[tokio::test]
async fn h7_the_hook_closing_its_own_connection_is_a_dismissal_and_the_agent_is_idle() {
    let f = proven(Fixture::running("sleep 30"));
    let mut events = f.bus.subscribe();
    let node = f.spawn(None, None).await.unwrap();
    let call = hook(&f, &node.id, bash("a"));
    let d = until_open(&f, 1).await.remove(0);

    call.abort();

    until_kind(&f, &node.id, Kind::Idle).await;
    assert!(open(&f).await.is_empty());
    let gone = cleared(&mut events);
    assert_eq!(gone.len(), 1);
    assert_eq!(
        (gone[0].id.as_str(), gone[0].outcome),
        (d.id.as_str(), Outcome::Terminal)
    );
}

#[tokio::test]
async fn h7_a_close_the_daemon_makes_clears_nothing_a_second_time() {
    let f = proven(Fixture::running("sleep 30"));
    let mut events = f.bus.subscribe();
    let node = f.spawn(None, None).await.unwrap();
    let call = hook(&f, &node.id, bash("a"));
    let d = until_open(&f, 1).await.remove(0);
    answer(&f, &d.id, "allow", Some(PROOF)).await.unwrap();
    finished(call).await.unwrap();

    tokio::time::sleep(Duration::from_millis(300)).await;

    let gone = cleared(&mut events);
    assert_eq!(gone.len(), 1);
    assert_eq!(kind_of(&f, &node.id).await, Kind::Working);
}

#[tokio::test]
async fn h9_a_permission_request_sent_as_a_signal_opens_no_decision() {
    let f = proven(Fixture::running("sleep 30"));
    let node = f.spawn(None, None).await.unwrap();
    let before = kind_of(&f, &node.id).await;

    f.call("agent.signal", json!({"id": node.id, "payload": bash("a")}))
        .await
        .unwrap();

    assert!(open(&f).await.is_empty());
    assert_eq!(kind_of(&f, &node.id).await, before);
}

#[tokio::test]
async fn h10_a_childs_permission_request_never_reaches_its_parent() {
    let f = proven(Fixture::running("cat"));
    let mut events = f.bus.subscribe();
    let parent = f.spawn(None, None).await.unwrap();
    let child = f.spawn(Some(&parent.id), None).await.unwrap();
    let parent_terminal = parent.terminal_id.clone().expect("a parent has a Terminal");
    let parent_kind = kind_of(&f, &parent.id).await;
    drained(&mut events);

    let call = hook(&f, &child.id, bash("a"));
    until_open(&f, 1).await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    let events = drained(&mut events);
    let to_parent = events.iter().filter(|event| match &event.data {
        EventData::AgentStatus(status) => status.id == parent.id,
        EventData::TerminalOutput(out) => out.id == parent_terminal,
        _ => false,
    });
    assert_eq!(to_parent.count(), 0);
    assert_eq!(kind_of(&f, &parent.id).await, parent_kind);
    assert_eq!(open(&f).await.len(), 1);
    call.abort();
}
