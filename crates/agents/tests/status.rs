//! A1 replays the hooks-state spike fixtures, so re-recording the spike after a Claude Code
//! payload change re-tests the adapter against it. A3 exits are hand-built.

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use agents::claude_code::ClaudeCode;
use agents::{AgentAdapter, Observation};
use contracts::{Kind, Status};
use serde_json::Value;

const RUN1: &str = include_str!("../../../spikes/hooks-state/log.run1.jsonl");
const RUN2: &str = include_str!("../../../spikes/hooks-state/log.run2.jsonl");
const FAILURE: &str = include_str!("../../../spikes/hooks-state/log.jsonl");

/// An adapter whose clock the test winds by hand.
fn adapter() -> (ClaudeCode, Arc<AtomicI64>) {
    let now = Arc::new(AtomicI64::new(1000));
    let clock = Arc::clone(&now);
    (ClaudeCode::new(move || clock.load(Ordering::SeqCst)), now)
}

fn payloads(log: &str) -> Vec<Value> {
    log.lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap()["payload"].clone())
        .collect()
}

fn event(log: &str, name: &str) -> Value {
    payloads(log)
        .into_iter()
        .find(|payload| payload["hook_event_name"] == name)
        .unwrap_or_else(|| panic!("no {name} in fixture"))
}

fn signal(adapter: &mut ClaudeCode, payload: Value) -> Option<Status> {
    adapter.observe(Observation::Signal(payload))
}

fn kind(status: Option<Status>) -> Kind {
    status.expect("status changed").kind
}

#[test]
fn a1_user_prompt_submit_gives_working() {
    let (mut adapter, _) = adapter();
    let status = signal(&mut adapter, event(RUN1, "UserPromptSubmit"));
    assert_eq!(kind(status), Kind::Working);
}

#[test]
fn a1_stop_gives_idle() {
    let (mut adapter, _) = adapter();
    signal(&mut adapter, event(RUN1, "UserPromptSubmit"));
    let status = signal(&mut adapter, event(RUN1, "Stop"));
    assert_eq!(kind(status), Kind::Idle);
}

#[test]
fn a1_permission_request_gives_needs_you_quoting_the_tool() {
    let (mut adapter, _) = adapter();
    let status = signal(&mut adapter, event(RUN1, "PermissionRequest")).unwrap();
    assert_eq!(status.kind, Kind::NeedsYou);
    assert_eq!(status.label, "Bash: touch x.txt");
}

#[test]
fn a1_permission_request_quotes_the_question_it_asks() {
    let (mut adapter, _) = adapter();
    let ask = payloads(RUN2)
        .into_iter()
        .find(|p| {
            p["tool_name"] == "AskUserQuestion" && p["hook_event_name"] == "PermissionRequest"
        })
        .unwrap();
    let status = signal(&mut adapter, ask).unwrap();
    assert_eq!(status.kind, Kind::NeedsYou);
    assert_eq!(status.label, "Do you prefer A or B?");
}

#[test]
fn a1_stop_failure_gives_error_labelled_with_the_error_field() {
    let (mut adapter, _) = adapter();
    let status = signal(&mut adapter, event(FAILURE, "StopFailure")).unwrap();
    assert_eq!(status.kind, Kind::Error);
    assert_eq!(status.label, "model_not_found");
}

#[test]
fn a1_stop_failure_without_an_error_field_is_still_an_error() {
    let (mut adapter, _) = adapter();
    signal(&mut adapter, event(RUN1, "UserPromptSubmit"));
    let mut failure = event(FAILURE, "StopFailure");
    failure.as_object_mut().unwrap().remove("error");
    let status = signal(&mut adapter, failure).unwrap();
    assert_eq!((status.kind, status.label.as_str()), (Kind::Error, "error"));
}

#[test]
fn a1_post_tool_use_after_needs_you_gives_working() {
    let (mut adapter, _) = adapter();
    signal(&mut adapter, event(RUN1, "PermissionRequest"));
    let status = signal(&mut adapter, event(RUN1, "PostToolUse"));
    assert_eq!(kind(status), Kind::Working);
}

#[test]
fn a1_pre_tool_use_after_a_stop_gives_working() {
    let (mut adapter, _) = adapter();
    signal(&mut adapter, event(RUN1, "Stop"));
    let status = signal(&mut adapter, event(RUN1, "PreToolUse"));
    assert_eq!(kind(status), Kind::Working);
}

#[test]
fn a1_a_failed_tool_is_not_an_agent_error() {
    let (mut adapter, _) = adapter();
    signal(&mut adapter, event(RUN2, "PermissionRequest"));
    let status = signal(&mut adapter, event(RUN2, "PostToolUseFailure"));
    assert_eq!(kind(status), Kind::Working);
}

#[test]
fn a1_session_end_gives_done() {
    let (mut adapter, _) = adapter();
    let status = signal(&mut adapter, event(RUN1, "SessionEnd"));
    assert_eq!(kind(status), Kind::Done);
}

#[test]
fn a1_replaying_a_whole_run_yields_the_kinds_the_spike_saw() {
    let (mut adapter, _) = adapter();
    let kinds: Vec<Kind> = payloads(RUN1)
        .into_iter()
        .filter_map(|payload| signal(&mut adapter, payload))
        .map(|status| status.kind)
        .collect();
    // SubagentStop, PostToolBatch, MessageDisplay and the late Notification change nothing.
    assert_eq!(
        kinds,
        [
            Kind::Idle,
            Kind::Working,
            Kind::Idle,
            Kind::Working,
            Kind::NeedsYou,
            Kind::Working,
            Kind::Idle,
            Kind::Done,
        ]
    );
}

#[test]
fn a1_a_signal_that_repeats_the_status_changes_nothing() {
    let (mut adapter, _) = adapter();
    signal(&mut adapter, event(RUN1, "UserPromptSubmit"));
    assert_eq!(signal(&mut adapter, event(RUN1, "PreToolUse")), None);
}

#[test]
fn a1_an_unrecognised_payload_changes_nothing() {
    let (mut adapter, _) = adapter();
    signal(&mut adapter, event(RUN1, "UserPromptSubmit"));
    let unknown = serde_json::json!({"hook_event_name": "FutureEvent"});
    assert_eq!(signal(&mut adapter, unknown), None);
    assert_eq!(
        signal(&mut adapter, serde_json::json!("not an object")),
        None
    );
}

#[test]
fn a1_since_is_the_injected_clock_at_the_change_and_survives_a_relabel() {
    let (mut adapter, now) = adapter();
    now.store(5000, Ordering::SeqCst);
    let working = signal(&mut adapter, event(RUN1, "UserPromptSubmit")).unwrap();
    assert_eq!(working.since, 5000);

    now.store(6000, Ordering::SeqCst);
    let asking = signal(&mut adapter, event(RUN1, "PermissionRequest")).unwrap();
    assert_eq!(asking.since, 6000);

    now.store(7000, Ordering::SeqCst);
    let mut other = event(RUN1, "PermissionRequest");
    other["tool_name"] = "Write".into();
    let relabelled = signal(&mut adapter, other).unwrap();
    assert_eq!((relabelled.kind, relabelled.since), (Kind::NeedsYou, 6000));
}

#[test]
fn a3_exit_zero_is_done() {
    let (mut adapter, _) = adapter();
    signal(&mut adapter, event(RUN1, "UserPromptSubmit"));
    let status = adapter
        .observe(Observation::Exit { code: Some(0) })
        .unwrap();
    assert_eq!(status.kind, Kind::Done);
}

#[test]
fn a3_non_zero_exit_is_an_error_naming_the_code() {
    let (mut adapter, _) = adapter();
    let status = adapter
        .observe(Observation::Exit { code: Some(3) })
        .unwrap();
    assert_eq!(
        (status.kind, status.label.as_str()),
        (Kind::Error, "exited 3")
    );
}

#[test]
fn a3_death_by_signal_is_an_error() {
    let (mut adapter, _) = adapter();
    let status = adapter.observe(Observation::Exit { code: None }).unwrap();
    assert_eq!(
        (status.kind, status.label.as_str()),
        (Kind::Error, "exited by signal")
    );
}

#[test]
fn a3_a_signal_after_exit_is_ignored() {
    let (mut adapter, _) = adapter();
    adapter.observe(Observation::Exit { code: Some(1) });
    assert_eq!(signal(&mut adapter, event(RUN1, "UserPromptSubmit")), None);
}

#[test]
fn a3_exit_after_a_clean_session_end_stays_done() {
    let (mut adapter, _) = adapter();
    signal(&mut adapter, event(RUN1, "SessionEnd"));
    assert_eq!(adapter.observe(Observation::Exit { code: Some(0) }), None);
}

#[test]
fn a7_a_stopped_agent_stays_done_when_its_program_dies_by_signal() {
    let (mut adapter, _) = adapter();
    let status = adapter.observe(Observation::Stopped).unwrap();
    assert_eq!(
        (status.kind, status.label.as_str()),
        (Kind::Done, "stopped")
    );
    assert_eq!(adapter.observe(Observation::Exit { code: None }), None);
}

#[test]
fn h9_answered_after_needs_you_gives_working() {
    let (mut adapter, _) = adapter();
    signal(&mut adapter, event(RUN1, "PermissionRequest"));
    let status = adapter.observe(Observation::Answered).unwrap();
    assert_eq!(
        (status.kind, status.label.as_str()),
        (Kind::Working, "working")
    );
}

#[test]
fn h9_dismissed_after_needs_you_gives_idle() {
    let (mut adapter, _) = adapter();
    signal(&mut adapter, event(RUN1, "PermissionRequest"));
    let status = adapter.observe(Observation::Dismissed).unwrap();
    assert_eq!((status.kind, status.label.as_str()), (Kind::Idle, "idle"));
}

#[test]
fn h9_answered_clears_a_held_star() {
    let (mut adapter, _) = adapter();
    signal(&mut adapter, event(RUN1, "UserPromptSubmit"));
    signal(&mut adapter, event(RUN1, "PreToolUse"));
    adapter.observe(Observation::Title("✳ Reading".into()));
    assert!(adapter.tick_at().is_some(), "the star should be held");

    adapter.observe(Observation::Answered);

    assert_eq!(adapter.tick_at(), None);
}

#[test]
fn h9_dismissed_clears_a_held_star() {
    let (mut adapter, _) = adapter();
    signal(&mut adapter, event(RUN1, "UserPromptSubmit"));
    signal(&mut adapter, event(RUN1, "PreToolUse"));
    adapter.observe(Observation::Title("✳ Reading".into()));
    assert!(adapter.tick_at().is_some(), "the star should be held");

    adapter.observe(Observation::Dismissed);

    assert_eq!(adapter.tick_at(), None);
}

#[test]
fn h9_answered_after_the_agent_has_ended_changes_nothing() {
    let (mut adapter, _) = adapter();
    adapter.observe(Observation::Exit { code: Some(1) });

    assert_eq!(adapter.observe(Observation::Answered), None);
    assert_eq!(adapter.status().unwrap().kind, Kind::Error);
}

#[test]
fn h9_dismissed_after_a_stopped_agent_changes_nothing() {
    let (mut adapter, _) = adapter();
    adapter.observe(Observation::Stopped);

    assert_eq!(adapter.observe(Observation::Dismissed), None);
    assert_eq!(adapter.status().unwrap().kind, Kind::Done);
}
