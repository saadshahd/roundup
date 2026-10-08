//! H2 to H7 and H10: the permission Decision store, driven through `agent.permission`,
//! `decision.list` and `decision.answer` the way `rup permission` and the App call them. A hook
//! that closes its connection is a call that is dropped.

use std::sync::Arc;
use std::time::Duration;

use contracts::decision::{ClearedEvent, Decision, Outcome};
use contracts::{Actor, Event, EventData, Kind};
use rpc::{Ctx, Module, RpcError, code};
use serde_json::{Value, json};
use tokio::sync::broadcast::Receiver;
use tokio::task::JoinHandle;

use crate::common::{Fixture, status_of};

const PROOF: &str = "app-proof";

/// The replies `spikes/hooks-permission/REPORT.md` findings 12 and 13 record, with the deny
/// message the Daemon chooses.
const ALLOW: &str = r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}"#;
const DENY: &str = r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","message":"Denied in roundup"}}}"#;

type Waiting = JoinHandle<Result<Value, RpcError>>;

fn bash(command: &str) -> Value {
    json!({"hook_event_name": "PermissionRequest", "tool_name": "Bash",
           "tool_input": {"command": command}})
}

fn ask_question() -> Value {
    json!({"hook_event_name": "PermissionRequest", "tool_name": "AskUserQuestion",
           "tool_input": {"questions": [{"question": "Do you prefer cats or dogs?"}]}})
}

async fn running() -> (Arc<Fixture>, String) {
    let f = Arc::new(Fixture::running("sleep 30").with_proof(PROOF));
    let node = f.spawn(None, None).await.unwrap();
    (f, node.id)
}

/// `rup permission`'s call, left waiting.
fn permission(f: &Arc<Fixture>, id: &str, payload: Value) -> Waiting {
    let (f, id) = (Arc::clone(f), id.to_owned());
    tokio::spawn(async move {
        f.call("agent.permission", json!({"id": id, "payload": payload}))
            .await
    })
}

async fn list(f: &Fixture) -> Vec<Decision> {
    serde_json::from_value(f.call("decision.list", Value::Null).await.unwrap()).unwrap()
}

async fn until_open(f: &Fixture, count: usize) -> Vec<Decision> {
    for _ in 0..500 {
        let open = list(f).await;
        if open.len() == count {
            return open;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("never {count} open Decisions");
}

async fn answer(f: &Fixture, id: &str, word: &str, proof: Option<&str>) -> Result<Value, RpcError> {
    f.call(
        "decision.answer",
        json!({"id": id, "answer": word, "proof": proof}),
    )
    .await
}

async fn kind(f: &Fixture, agent: &str) -> Kind {
    status_of(&f.tree().await, agent).kind
}

async fn until_kind(f: &Fixture, agent: &str, want: Kind) {
    f.until(|tree| status_of(tree, agent).kind == want).await;
}

fn signal(f: &Arc<Fixture>, id: &str, event: &str) -> Waiting {
    let (f, id, event) = (Arc::clone(f), id.to_owned(), event.to_owned());
    tokio::spawn(async move {
        let attempt = f.attempt(&id).await;
        let payload = json!({"hook_event_name": event, "tool_name": "Bash"});
        f.call(
            "agent.signal",
            json!({"id": id, "attempt": attempt, "payload": payload}),
        )
        .await
    })
}

fn decision_events(events: &mut Receiver<Event>) -> Vec<EventData> {
    std::iter::from_fn(|| events.try_recv().ok())
        .map(|event| event.data)
        .filter(|data| {
            matches!(
                data,
                EventData::DecisionOpened(_) | EventData::DecisionCleared(_)
            )
        })
        .collect()
}

fn cleared(data: &EventData) -> Option<&ClearedEvent> {
    match data {
        EventData::DecisionCleared(cleared) => Some(cleared),
        _ => None,
    }
}

async fn error_of(waiting: Waiting) -> RpcError {
    tokio::time::timeout(Duration::from_secs(10), waiting)
        .await
        .expect("the hook's call returns")
        .unwrap()
        .unwrap_err()
}

#[tokio::test]
async fn h2_a_permission_request_opens_one_decision_and_the_agent_needs_you() {
    let (f, agent) = running().await;
    let mut events = f.bus.subscribe();

    let _hook = permission(&f, &agent, bash("touch spike_out.txt"));

    let open = until_open(&f, 1).await;
    assert_eq!(open[0].agent, agent);
    assert_eq!(open[0].tool, "Bash");
    assert_eq!(open[0].args, r#"{"command":"touch spike_out.txt"}"#);
    assert!(open[0].answerable);
    let opened: Vec<_> = decision_events(&mut events);
    assert_eq!(opened.len(), 1);
    assert!(matches!(&opened[0], EventData::DecisionOpened(d) if *d == open[0]));
    let tree = f.tree().await;
    assert_eq!(status_of(&tree, &agent).kind, Kind::NeedsYou);
    assert_eq!(status_of(&tree, &agent).label, "Bash: touch spike_out.txt");
}

#[tokio::test]
async fn h2_args_over_2000_characters_are_cut_with_a_final_ellipsis() {
    let (f, agent) = running().await;

    let _hook = permission(&f, &agent, bash(&"é".repeat(3000)));

    let args = until_open(&f, 1).await.remove(0).args;
    assert_eq!(args.chars().count(), 2001);
    assert!(args.ends_with('…'));
}

#[tokio::test]
async fn h2_a_request_for_an_unknown_agent_is_not_found() {
    let (f, _) = running().await;

    let err = permission(&f, "nope", bash("ls"))
        .await
        .unwrap()
        .unwrap_err();

    assert_eq!(err.code, code::NOT_FOUND);
    assert!(list(&f).await.is_empty());
}

#[tokio::test]
async fn h2_ask_user_question_opens_an_unanswerable_decision_and_returns_at_once() {
    let (f, agent) = running().await;

    let output = permission(&f, &agent, ask_question())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(output, json!({"output": ""}));
    let open = list(&f).await;
    assert_eq!(
        open.len(),
        1,
        "the Daemon ending the call leaves the Decision open"
    );
    assert_eq!(open[0].tool, "AskUserQuestion");
    assert!(!open[0].answerable);
    assert!(open[0].args.contains("Do you prefer cats or dogs?"));
    let tree = f.tree().await;
    assert_eq!(status_of(&tree, &agent).kind, Kind::NeedsYou);
    assert_eq!(
        status_of(&tree, &agent).label,
        "Do you prefer cats or dogs?"
    );
}

#[tokio::test]
async fn h2_a_second_request_replaces_the_first_and_supersedes_its_hook() {
    let (f, agent) = running().await;
    let first = permission(&f, &agent, bash("one"));
    let old = until_open(&f, 1).await.remove(0);
    let mut events = f.bus.subscribe();

    let _second = permission(&f, &agent, bash("two"));

    let superseded = error_of(first).await;
    assert_eq!(superseded.message, "superseded");
    let open = until_open(&f, 1).await;
    assert_ne!(open[0].id, old.id);
    assert_eq!(open[0].args, r#"{"command":"two"}"#);
    let seen = decision_events(&mut events);
    assert_eq!(seen.len(), 2);
    assert!(
        matches!(&seen[0], EventData::DecisionCleared(c) if c.id == old.id && c.outcome == Outcome::Replaced)
    );
    assert!(matches!(&seen[1], EventData::DecisionOpened(d) if d.id == open[0].id));
    assert_eq!(kind(&f, &agent).await, Kind::NeedsYou);
}

#[tokio::test]
async fn h3_an_answer_prints_the_recorded_reply_and_the_agent_works_again() {
    for (word, reply, outcome) in [
        ("allow", ALLOW, Outcome::Allow),
        ("deny", DENY, Outcome::Deny),
    ] {
        let (f, agent) = running().await;
        let hook = permission(&f, &agent, bash("ls"));
        let id = until_open(&f, 1).await.remove(0).id;
        let mut events = f.bus.subscribe();

        let done = answer(&f, &id, word, Some(PROOF)).await.unwrap();

        assert_eq!(done, Value::Null);
        assert_eq!(hook.await.unwrap().unwrap(), json!({"output": reply}));
        assert!(list(&f).await.is_empty());
        let seen = decision_events(&mut events);
        assert_eq!(seen.len(), 1);
        assert_eq!(cleared(&seen[0]).unwrap().outcome, outcome);
        assert_eq!(kind(&f, &agent).await, Kind::Working);
    }
}

#[tokio::test]
async fn h3_an_answer_for_an_unknown_or_cleared_decision_is_not_found() {
    let (f, agent) = running().await;
    let _hook = permission(&f, &agent, bash("ls"));
    let id = until_open(&f, 1).await.remove(0).id;
    answer(&f, &id, "allow", Some(PROOF)).await.unwrap();

    let again = answer(&f, &id, "allow", Some(PROOF)).await.unwrap_err();
    let unknown = answer(&f, "d99", "allow", Some(PROOF)).await.unwrap_err();

    assert_eq!(again.code, code::NOT_FOUND);
    assert_eq!(unknown.code, code::NOT_FOUND);
}

#[tokio::test]
async fn h3_a_word_that_is_neither_allow_nor_deny_is_invalid_params() {
    let (f, agent) = running().await;
    let _hook = permission(&f, &agent, bash("ls"));
    let id = until_open(&f, 1).await.remove(0).id;

    let err = answer(&f, &id, "ask", Some(PROOF)).await.unwrap_err();

    assert_eq!(err.code, code::INVALID_PARAMS);
    assert_eq!(list(&f).await.len(), 1);
}

#[tokio::test]
async fn h3_a_decision_that_is_not_answerable_is_invalid_params() {
    let (f, agent) = running().await;
    permission(&f, &agent, ask_question())
        .await
        .unwrap()
        .unwrap();
    let id = list(&f).await.remove(0).id;

    let err = answer(&f, &id, "allow", Some(PROOF)).await.unwrap_err();

    assert_eq!(err.code, code::INVALID_PARAMS);
    assert_eq!(list(&f).await.len(), 1);
}

#[tokio::test]
async fn h3_a_missing_or_wrong_proof_is_forbidden_and_changes_nothing() {
    let (f, agent) = running().await;
    let _hook = permission(&f, &agent, bash("ls"));
    let id = until_open(&f, 1).await.remove(0).id;

    let missing = answer(&f, &id, "allow", None).await.unwrap_err();
    let wrong = answer(&f, &id, "allow", Some("guess")).await.unwrap_err();

    assert_eq!(missing.code, code::FORBIDDEN);
    assert_eq!(wrong.code, code::FORBIDDEN);
    assert_eq!(list(&f).await.len(), 1);
    assert_eq!(kind(&f, &agent).await, Kind::NeedsYou);
}

#[tokio::test]
async fn h3_a_daemon_with_no_proof_refuses_every_answer() {
    let f = Arc::new(Fixture::running("sleep 30"));
    let agent = f.spawn(None, None).await.unwrap().id;
    let _hook = permission(&f, &agent, bash("ls"));
    let id = until_open(&f, 1).await.remove(0).id;

    let err = answer(&f, &id, "allow", None).await.unwrap_err();

    assert_eq!(err.code, code::FORBIDDEN);
}

#[tokio::test]
async fn h5_a_new_client_lists_the_same_decision_and_answers_the_waiting_hook() {
    let (f, agent) = running().await;
    let hook = permission(&f, &agent, bash("ls"));
    let before = until_open(&f, 1).await;
    // The reloaded webview is a new connection: a new Ctx with its own Actor.
    let reloaded = Ctx {
        actor: Actor::user(),
        ..f.ctx()
    };

    let seen: Vec<Decision> = serde_json::from_value(
        f.agents
            .call(&reloaded, "decision.list", Value::Null)
            .await
            .unwrap(),
    )
    .unwrap();
    f.agents
        .call(
            &reloaded,
            "decision.answer",
            json!({"id": seen[0].id, "answer": "allow", "proof": PROOF}),
        )
        .await
        .unwrap();

    assert_eq!(seen, before);
    assert_eq!(hook.await.unwrap().unwrap(), json!({"output": ALLOW}));
}

#[tokio::test]
async fn h5_a_new_daemon_holds_no_decision() {
    let (f, agent) = running().await;
    // No waiting hook, so no task keeps the Fixture alive.
    permission(&f, &agent, ask_question())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(list(&f).await.len(), 1);
    let f = Arc::into_inner(f)
        .expect("no task holds the Fixture")
        .reopen();

    assert!(list(&f).await.is_empty());
}

#[tokio::test]
async fn h6_stopping_the_agent_clears_its_decision_and_fails_the_hook() {
    let (f, agent) = running().await;
    let hook = permission(&f, &agent, bash("ls"));
    let id = until_open(&f, 1).await.remove(0).id;
    let mut events = f.bus.subscribe();

    f.call("agent.stop", json!({"id": agent})).await.unwrap();

    let err = error_of(hook).await;
    assert_eq!(err.message, "the Agent ended");
    assert!(list(&f).await.is_empty());
    let seen = decision_events(&mut events);
    assert_eq!(seen.len(), 1);
    let gone = cleared(&seen[0]).unwrap();
    assert_eq!(
        (gone.id.as_str(), gone.outcome),
        (id.as_str(), Outcome::AgentGone)
    );
    assert_eq!(kind(&f, &agent).await, Kind::Done);
}

#[tokio::test]
async fn h6_a_terminal_that_exits_clears_the_decision() {
    let (f, agent) = running().await;
    let hook = permission(&f, &agent, bash("ls"));
    until_open(&f, 1).await;
    let mut events = f.bus.subscribe();
    let terminal = f
        .tree()
        .await
        .into_iter()
        .find(|node| node.id == agent)
        .and_then(|node| node.terminal_id)
        .expect("a running Agent has a Terminal");

    f.terminals.kill(&terminal).await.unwrap();

    let err = error_of(hook).await;
    assert_eq!(err.message, "the Agent ended");
    assert!(list(&f).await.is_empty());
    let seen = decision_events(&mut events);
    assert_eq!(seen.len(), 1);
    assert_eq!(cleared(&seen[0]).unwrap().outcome, Outcome::AgentGone);
    until_kind(&f, &agent, Kind::Error).await;
}

#[tokio::test]
async fn h7_a_signal_after_an_answer_in_the_terminal_clears_the_decision() {
    for (event, want) in [
        ("PostToolUse", Kind::Working),
        ("PostToolUseFailure", Kind::Working),
        ("UserPromptSubmit", Kind::Working),
        ("Stop", Kind::Idle),
    ] {
        let (f, agent) = running().await;
        let hook = permission(&f, &agent, bash("ls"));
        let id = until_open(&f, 1).await.remove(0).id;
        let mut events = f.bus.subscribe();

        signal(&f, &agent, event).await.unwrap().unwrap();

        let seen = decision_events(&mut events);
        assert_eq!(seen.len(), 1, "{event}");
        assert_eq!(
            cleared(&seen[0]).unwrap().outcome,
            Outcome::Terminal,
            "{event}"
        );
        assert!(list(&f).await.is_empty(), "{event}");
        assert_eq!(kind(&f, &agent).await, want, "{event}");
        let late = answer(&f, &id, "allow", Some(PROOF)).await.unwrap_err();
        assert_eq!(late.code, code::NOT_FOUND, "{event}");
        // The Daemon closes the hook's connection; it is not a dismissal.
        assert_eq!(error_of(hook).await.code, code::CONFLICT, "{event}");
        assert_eq!(kind(&f, &agent).await, want, "{event}");
    }
}

#[tokio::test]
async fn h7_a_hook_that_closes_early_is_a_no_or_an_esc_and_leaves_the_agent_idle() {
    let (f, agent) = running().await;
    let hook = permission(&f, &agent, bash("ls"));
    let id = until_open(&f, 1).await.remove(0).id;
    let mut events = f.bus.subscribe();

    hook.abort();
    let _ = hook.await;

    f.until(|tree| status_of(tree, &agent).kind == Kind::Idle)
        .await;
    assert!(list(&f).await.is_empty());
    let seen = decision_events(&mut events);
    assert_eq!(seen.len(), 1);
    assert_eq!(cleared(&seen[0]).unwrap().outcome, Outcome::Terminal);
    let late = answer(&f, &id, "allow", Some(PROOF)).await.unwrap_err();
    assert_eq!(late.code, code::NOT_FOUND);
}

#[tokio::test]
async fn h7_a_decision_with_no_live_hook_survives_a_close_and_is_cleared_by_a_signal() {
    let (f, agent) = running().await;
    permission(&f, &agent, ask_question())
        .await
        .unwrap()
        .unwrap();

    // The Daemon itself ended that call: nothing cleared the Decision.
    assert_eq!(list(&f).await.len(), 1);
    assert_eq!(kind(&f, &agent).await, Kind::NeedsYou);

    signal(&f, &agent, "UserPromptSubmit")
        .await
        .unwrap()
        .unwrap();

    assert!(list(&f).await.is_empty());
    assert_eq!(kind(&f, &agent).await, Kind::Working);
}

/// H10: a permission request stays with the user. The parent hears nothing.
#[tokio::test]
async fn h10_a_childs_decision_reaches_no_byte_signal_or_call_of_its_parent() {
    let f = Arc::new(Fixture::running("sleep 30").with_proof(PROOF));
    // A Door is the parent Agent of the Agents in its Room.
    let parent = f.room("team", None).await;
    let child = f.spawn(Some(&parent), None).await.unwrap().id;
    f.call("rail.startDoor", json!({"id": parent}))
        .await
        .unwrap();
    until_kind(&f, &parent, Kind::Working).await;
    let parent_before = status_of(&f.tree().await, &parent).clone();
    let mut events = f.bus.subscribe();
    let parent_terminal = f
        .tree()
        .await
        .into_iter()
        .find(|node| node.id == parent)
        .and_then(|node| node.terminal_id)
        .unwrap();
    let mut output = f.terminals.subscribe(&parent_terminal).unwrap();

    let _hook = permission(&f, &child, bash("ls"));
    let open = until_open(&f, 1).await;

    assert_eq!(open[0].agent, child);
    assert_eq!(status_of(&f.tree().await, &parent), &parent_before);
    let mut heard_by_parent = vec![];
    while let Ok(event) = output.try_recv() {
        heard_by_parent.push(event);
    }
    assert!(heard_by_parent.is_empty(), "{heard_by_parent:?}");
    let about_parent =
        std::iter::from_fn(|| events.try_recv().ok()).any(|event| match event.data {
            EventData::AgentStatus(status) => status.id == parent,
            _ => false,
        });
    assert!(!about_parent);
    // The parent cannot answer it however it is addressed: it has no proof.
    let forbidden = answer(&f, &open[0].id, "allow", None).await.unwrap_err();
    assert_eq!(forbidden.code, code::FORBIDDEN);
}
