//! H15: which hook events `STATE_EVENTS` (`crates/agents/src/claude_code/launch.rs`) may drop is
//! not chosen by hand. An event is prunable only when both hold for every fixture: removing every
//! occurrence of its Signal from the fixture's payload stream leaves the replayed Kind sequence
//! unchanged (A1's method, checked below against every `spikes/hooks-state/log*.jsonl` fixture and
//! D2's own payload sequence); and, for every run that also has a `screen*.jsonl` title capture of
//! the same run (A2, `title.rs`'s RUN1 to RUN7), removing the Signal from the full interleaved
//! Signal-and-Title timeline leaves the replayed Status sequence byte-for-byte unchanged (the
//! Kind-only check cannot see the star hold A2 reads from the title, `mod.rs`: that is how a
//! production build silently lost it when the first version of this rule dropped `PreToolUse`).
//! `log.jsonl` has no paired title capture, so it is checked by Kind alone. This test checks the
//! rule against `STATE_EVENTS` itself, not a hand copy of it, and asserts the one event it drops:
//! `PreToolUse`.

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use agents::claude_code::{ClaudeCode, STATE_EVENTS};
use agents::{AgentAdapter, Observation};
use contracts::{Kind, Status};
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

/// Every `log*.jsonl` fixture paired with the `screen*.jsonl` title capture of the same run
/// (`title.rs`'s RUN1 to RUN7); `log.jsonl` has none, by timestamp (its run outlasts every
/// `screen*.jsonl` capture), so it is covered only by the Kind-only check above.
const TITLED_RUNS: [(&str, &str); 7] = [
    (
        include_str!("../../../spikes/hooks-state/log.run1.jsonl"),
        include_str!("../../../spikes/hooks-state/screen.jsonl"),
    ),
    (
        include_str!("../../../spikes/hooks-state/log.run2.jsonl"),
        include_str!("../../../spikes/hooks-state/screen2.jsonl"),
    ),
    (
        include_str!("../../../spikes/hooks-state/log.run3.jsonl"),
        include_str!("../../../spikes/hooks-state/screen3.jsonl"),
    ),
    (
        include_str!("../../../spikes/hooks-state/log.run4.jsonl"),
        include_str!("../../../spikes/hooks-state/screen.run4.jsonl"),
    ),
    (
        include_str!("../../../spikes/hooks-state/log.run5.jsonl"),
        include_str!("../../../spikes/hooks-state/screen.run5.jsonl"),
    ),
    (
        include_str!("../../../spikes/hooks-state/log.run6.jsonl"),
        include_str!("../../../spikes/hooks-state/screen.run6.jsonl"),
    ),
    (
        include_str!("../../../spikes/hooks-state/log.run7.jsonl"),
        include_str!("../../../spikes/hooks-state/screen.run7.jsonl"),
    ),
];

fn lines(jsonl: &str) -> Vec<Value> {
    jsonl
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

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
fn replay_signals(payloads: &[Value]) -> Vec<Kind> {
    let mut adapter = ClaudeCode::new(|| 0);
    payloads
        .iter()
        .filter_map(|payload| adapter.observe(Observation::Signal(payload.clone())))
        .map(|status| status.kind)
        .collect()
}

fn without_event(payloads: &[Value], event: &str) -> Vec<Value> {
    payloads
        .iter()
        .filter(|payload| payload["hook_event_name"] != event)
        .cloned()
        .collect()
}

/// `event` is prunable from the Signal stream alone only when dropping it changes no fixture's
/// replayed Kinds at all.
fn prunable_from_signals(event: &str, runs: &[Vec<Value>]) -> bool {
    runs.iter()
        .all(|run| replay_signals(run) == replay_signals(&without_event(run, event)))
}

/// Every hook and every title of one run, in the order they happened (`title.rs`'s `timeline`):
/// the titles come from the terminal, not a hook, so dropping a Signal never removes one.
fn timeline(log: &str, screen: &str) -> Vec<(f64, Observation)> {
    let hooks = lines(log)
        .into_iter()
        .filter(|line| line["payload"]["hook_event_name"] != "MARK")
        .map(|line| {
            (
                line["ts"].as_f64().unwrap(),
                Observation::Signal(line["payload"].clone()),
            )
        });
    let titles = lines(screen).into_iter().flat_map(|line| {
        let ts = line["ts"].as_f64().unwrap();
        let out = line["out"].as_str().unwrap_or_default().to_owned();
        out.split("\x1b]0;")
            .skip(1)
            .filter_map(|rest| rest.split('\x07').next())
            .map(|title| (ts, Observation::Title(title.to_owned())))
            .collect::<Vec<_>>()
    });
    let mut all: Vec<_> = hooks.chain(titles).collect();
    all.sort_by(|a, b| a.0.total_cmp(&b.0));
    all
}

fn millis(seconds: f64) -> i64 {
    (seconds * 1000.0).round() as i64
}

/// `Observation` carries no `Clone`: a replay over the same timeline twice (full, then pruned)
/// needs its own copy of each observation.
fn clone_observation(observation: &Observation) -> Observation {
    match observation {
        Observation::Signal(payload) => Observation::Signal(payload.clone()),
        Observation::Title(title) => Observation::Title(title.clone()),
        Observation::Tick | Observation::Stopped | Observation::Exit { .. } => {
            unreachable!("a timeline never carries a Tick, Stopped or Exit")
        }
    }
}

struct Clocked {
    adapter: ClaudeCode,
    now: Arc<AtomicI64>,
}

impl Clocked {
    fn new() -> Self {
        let now = Arc::new(AtomicI64::new(0));
        let clock = Arc::clone(&now);
        Self {
            adapter: ClaudeCode::new(move || clock.load(Ordering::SeqCst)),
            now,
        }
    }

    fn at(&mut self, ms: i64, observation: Observation) -> Option<Status> {
        self.now.store(ms, Ordering::SeqCst);
        self.adapter.observe(observation)
    }
}

/// Deliver every Tick `clocked`'s adapter asks for up to and including `until`.
fn deliver_due_ticks(clocked: &mut Clocked, until: i64, statuses: &mut Vec<Status>) {
    while let Some(due) = clocked.adapter.tick_at().filter(|due| *due <= until) {
        if let Some(status) = clocked.at(due, Observation::Tick) {
            statuses.push(status);
        }
    }
}

/// The full Status sequence a fresh adapter emits, in order, replaying `items` on a clock that
/// follows their timestamps, delivering a Tick whenever one falls due before the next item.
fn replay_full(items: Vec<(f64, Observation)>) -> Vec<Status> {
    let mut clocked = Clocked::new();
    let mut statuses = Vec::new();
    for (ts, observation) in items {
        let ms = millis(ts);
        deliver_due_ticks(&mut clocked, ms, &mut statuses);
        if let Some(status) = clocked.at(ms, observation) {
            statuses.push(status);
        }
    }
    deliver_due_ticks(&mut clocked, i64::MAX, &mut statuses);
    statuses
}

fn clone_items(items: &[(f64, Observation)]) -> Vec<(f64, Observation)> {
    items
        .iter()
        .map(|(ts, observation)| (*ts, clone_observation(observation)))
        .collect()
}

fn without_signal(items: &[(f64, Observation)], event: &str) -> Vec<(f64, Observation)> {
    items
        .iter()
        .filter(|(_, observation)| {
            !matches!(observation, Observation::Signal(payload) if payload["hook_event_name"] == event)
        })
        .map(|(ts, observation)| (*ts, clone_observation(observation)))
        .collect()
}

/// `event` is prunable from the full Signal-and-Title timeline only when dropping its Signal
/// changes no titled run's replayed Status sequence at all.
fn prunable_from_timelines(event: &str, timelines: &[Vec<(f64, Observation)>]) -> bool {
    timelines
        .iter()
        .all(|items| replay_full(clone_items(items)) == replay_full(without_signal(items, event)))
}

#[test]
fn h15_pre_tool_use_is_the_only_event_a_replay_lets_state_events_drop() {
    let signal_only_runs: Vec<Vec<Value>> = FIXTURES
        .iter()
        .copied()
        .map(payloads)
        .chain(std::iter::once(d2_payloads()))
        .collect();
    let timelines: Vec<Vec<(f64, Observation)>> = TITLED_RUNS
        .iter()
        .map(|(log, screen)| timeline(log, screen))
        .collect();

    let candidates: Vec<&str> = STATE_EVENTS.iter().copied().chain(["PreToolUse"]).collect();

    let dropped: Vec<&str> = candidates
        .into_iter()
        .filter(|event| {
            prunable_from_signals(event, &signal_only_runs)
                && prunable_from_timelines(event, &timelines)
        })
        .collect();

    assert_eq!(dropped, ["PreToolUse"]);
}
