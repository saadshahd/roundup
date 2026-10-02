//! H15: which hook events `STATE_EVENTS` (`crates/agents/src/claude_code/launch.rs`) may drop is
//! not chosen by hand. An event is prunable only when removing every one of its occurrences from a
//! fixture's payload stream, before replaying the rest through the pure `observe` function (A1),
//! leaves that fixture's Kind sequence byte-for-byte unchanged. This test runs the rule over every
//! `spikes/hooks-state/log*.jsonl` fixture and over D2's own payload sequence, and asserts the one
//! event the rule drops: `PreToolUse` only ever repeats the Kind `UserPromptSubmit` already set.

use agents::claude_code::ClaudeCode;
use agents::{AgentAdapter, Observation};
use contracts::Kind;
use serde_json::Value;

const FIXTURES: [&str; 8] = [
    include_str!("../../../spikes/hooks-state/log.jsonl"),
    include_str!("../../../spikes/hooks-state/log.run1.jsonl"),
    include_str!("../../../spikes/hooks-state/log.run2.jsonl"),
    include_str!("../../../spikes/hooks-state/log.run3.jsonl"),
    include_str!("../../../spikes/hooks-state/log.run4.jsonl"),
    include_str!("../../../spikes/hooks-state/log.run5.jsonl"),
    include_str!("../../../spikes/hooks-state/log.run6.jsonl"),
    include_str!("../../../spikes/hooks-state/log.run7.jsonl"),
];

/// `log.run1.jsonl`, the fixture D2 (`scenarios/daemon.md`) plays.
const RUN1: &str = FIXTURES[1];

/// D2's own events, in the fixed order its fake `claude` plays them (`crates/rup/tests/e2e/fake_claude.py`): one payload per name, the first recorded.
const D2_EVENTS: [&str; 5] = [
    "SessionStart",
    "UserPromptSubmit",
    "PermissionRequest",
    "PostToolUse",
    "Stop",
];

/// Every event `STATE_EVENTS` registers today, plus `PreToolUse`: the one candidate the rule has
/// already dropped, kept here so the test still proves it, not just that it stays dropped.
const CANDIDATES: [&str; 9] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "PostToolUseFailure",
    "Stop",
    "StopFailure",
    "SessionEnd",
];

fn payloads(log: &str) -> Vec<Value> {
    log.lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap()["payload"].clone())
        .collect()
}

/// D2's payload sequence: one payload per `D2_EVENTS` name, the first `RUN1` has, in that order.
fn d2_payloads() -> Vec<Value> {
    let run1 = payloads(RUN1);
    D2_EVENTS
        .iter()
        .map(|event| {
            run1.iter()
                .find(|payload| payload["hook_event_name"] == *event)
                .unwrap_or_else(|| panic!("no {event} in log.run1.jsonl"))
                .clone()
        })
        .collect()
}

/// The Kinds a fresh adapter emits, in order, replaying `payloads` as Signals (A1's method).
fn replay(payloads: &[Value]) -> Vec<Kind> {
    let mut adapter = ClaudeCode::new(|| 0);
    payloads
        .iter()
        .filter_map(|payload| adapter.observe(Observation::Signal(payload.clone())))
        .map(|status| status.kind)
        .collect()
}

fn without(payloads: &[Value], event: &str) -> Vec<Value> {
    payloads
        .iter()
        .filter(|payload| payload["hook_event_name"] != event)
        .cloned()
        .collect()
}

/// `event` is prunable only when dropping it changes no run's replayed Kinds at all.
fn prunable(event: &str, runs: &[Vec<Value>]) -> bool {
    runs.iter()
        .all(|run| replay(run) == replay(&without(run, event)))
}

#[test]
fn h15_pre_tool_use_is_the_only_event_a_replay_lets_state_events_drop() {
    let runs: Vec<Vec<Value>> = FIXTURES
        .iter()
        .copied()
        .map(payloads)
        .chain(std::iter::once(d2_payloads()))
        .collect();

    let dropped: Vec<&str> = CANDIDATES
        .into_iter()
        .filter(|event| prunable(event, &runs))
        .collect();

    assert_eq!(dropped, ["PreToolUse"]);
}
