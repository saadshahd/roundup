//! A2: the terminal title closes the gaps no hook covers. Titles come from `screen*.jsonl`, hooks
//! from the `log*.jsonl` of the same run, interleaved by timestamp and replayed on a clock that
//! follows those timestamps.

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use agents::claude_code::{ClaudeCode, STAR_HOLD};
use agents::{AgentAdapter, Observation};
use contracts::Kind;
use serde_json::{Value, json};

type Run = (&'static str, &'static str);

const RUN1: Run = (
    include_str!("../../../spikes/hooks-state/log.run1.jsonl"),
    include_str!("../../../spikes/hooks-state/screen.jsonl"),
);
const RUN2: Run = (
    include_str!("../../../spikes/hooks-state/log.run2.jsonl"),
    include_str!("../../../spikes/hooks-state/screen2.jsonl"),
);
const RUN3: Run = (
    include_str!("../../../spikes/hooks-state/log.run3.jsonl"),
    include_str!("../../../spikes/hooks-state/screen3.jsonl"),
);

const RUN4: Run = (
    include_str!("../../../spikes/hooks-state/log.run4.jsonl"),
    include_str!("../../../spikes/hooks-state/screen.run4.jsonl"),
);
const RUN5: Run = (
    include_str!("../../../spikes/hooks-state/log.run5.jsonl"),
    include_str!("../../../spikes/hooks-state/screen.run5.jsonl"),
);
const RUN6: Run = (
    include_str!("../../../spikes/hooks-state/log.run6.jsonl"),
    include_str!("../../../spikes/hooks-state/screen.run6.jsonl"),
);
const RUN7: Run = (
    include_str!("../../../spikes/hooks-state/log.run7.jsonl"),
    include_str!("../../../spikes/hooks-state/screen.run7.jsonl"),
);

/// An adapter on a clock the test sets, in milliseconds.
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

    fn at(&mut self, ms: i64, observation: Observation) -> Option<Kind> {
        self.now.store(ms, Ordering::SeqCst);
        self.adapter.observe(observation).map(|status| status.kind)
    }

    fn signal(&mut self, ms: i64, event: &str) -> Option<Kind> {
        let payload = json!({"hook_event_name": event, "tool_name": "Bash"});
        self.at(ms, Observation::Signal(payload))
    }

    fn title(&mut self, ms: i64, title: &str) -> Option<Kind> {
        self.at(ms, Observation::Title(title.into()))
    }
}

fn lines(jsonl: &str) -> Vec<Value> {
    jsonl
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// The time of the mark `name` in a screen fixture.
fn mark(screen: &str, name: &str) -> f64 {
    let line = lines(screen).into_iter().find(|line| line["mark"] == name);
    line.unwrap_or_else(|| panic!("no mark {name}"))["ts"]
        .as_f64()
        .unwrap()
}

/// Every hook and every title of one run, in the order they happened.
fn timeline((log, screen): Run) -> Vec<(f64, Observation)> {
    // The spike's driver also logged its own marks beside the hooks.
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

/// One observation of a replay and what the Agent read as afterwards.
struct Step {
    ts: f64,
    event: String,
    kind: Option<Kind>,
}

/// Replay a run; whenever the adapter has asked for a Tick that falls before the next observation,
/// deliver it first, as the Agents module does.
fn replay(run: Run) -> Vec<Step> {
    let mut clocked = Clocked::new();
    let mut steps = vec![];
    let mut current = None;
    let mut deliver = |clocked: &mut Clocked, ts: f64, event: String, observation: Observation| {
        let ms = (ts * 1000.0).round() as i64;
        current = clocked.at(ms, observation).or(current);
        steps.push(Step {
            ts,
            event,
            kind: current,
        });
    };
    for (ts, observation) in timeline(run) {
        while let Some(due) = clocked
            .adapter
            .tick_at()
            .filter(|due| *due <= (ts * 1000.0) as i64)
        {
            deliver(
                &mut clocked,
                due as f64 / 1000.0,
                "Tick".into(),
                Observation::Tick,
            );
        }
        let event = match &observation {
            Observation::Signal(payload) => payload["hook_event_name"].as_str().unwrap().to_owned(),
            Observation::Title(title) => format!("title {title}"),
            _ => unreachable!(),
        };
        deliver(&mut clocked, ts, event, observation);
    }
    while let Some(due) = clocked.adapter.tick_at() {
        deliver(
            &mut clocked,
            due as f64 / 1000.0,
            "Tick".into(),
            Observation::Tick,
        );
    }
    steps
}

/// The Kinds the Agent changed to, in order, between `from` and `to` (seconds).
fn changes(steps: &[Step], from: f64, to: f64) -> Vec<Kind> {
    let mut before = steps
        .iter()
        .rev()
        .find(|s| s.ts <= from)
        .and_then(|s| s.kind);
    let mut out = vec![];
    for step in steps.iter().filter(|s| s.ts > from && s.ts < to) {
        if step.kind != before {
            out.extend(step.kind);
            before = step.kind;
        }
    }
    out
}

fn first_after(steps: &[Step], from: f64, event: &str) -> f64 {
    steps
        .iter()
        .find(|s| s.ts > from && s.event == event)
        .unwrap()
        .ts
}

#[test]
fn a2_a_spinner_title_gives_working() {
    let mut a = Clocked::new();
    a.signal(0, "Stop");
    assert_eq!(a.title(1, "◐ Say hi in 3 words"), Some(Kind::Working));
}

#[test]
fn a2_a_star_after_an_esc_interrupt_gives_idle() {
    let steps = replay(RUN2);
    let from = mark(RUN2.1, "submit:story");
    let to = mark(RUN2.1, "idle70");
    assert_eq!(changes(&steps, from, to), [Kind::Working, Kind::Idle]);
}

#[test]
fn a2_a_dialog_goes_working_to_needs_you_with_no_idle_between() {
    for run in [RUN1, RUN2, RUN3, RUN6, RUN7] {
        let steps = replay(run);
        let kinds: Vec<_> = changes(&steps, 0.0, f64::MAX);
        let into_needs_you: Vec<_> = kinds
            .windows(2)
            .filter(|w| w[1] == Kind::NeedsYou)
            .collect();
        assert!(!into_needs_you.is_empty());
        assert!(
            into_needs_you.iter().all(|w| w[0] == Kind::Working),
            "{kinds:?}"
        );
    }
}

#[test]
fn a2_a_permission_request_within_the_hold_goes_straight_to_needs_you() {
    let mut a = Clocked::new();
    a.signal(0, "UserPromptSubmit");
    a.signal(10, "PreToolUse");
    assert_eq!(a.title(30, "✳ Create z.txt"), None);
    assert_eq!(
        a.signal(30 + STAR_HOLD - 1, "PermissionRequest"),
        Some(Kind::NeedsYou)
    );
    assert_eq!(a.adapter.tick_at(), None);
}

#[test]
fn a2_an_unanswered_star_after_pre_tool_use_resolves_to_idle_on_the_tick() {
    let mut a = Clocked::new();
    a.signal(0, "UserPromptSubmit");
    a.signal(10, "PreToolUse");
    assert_eq!(a.title(500, "✳ Reading"), None);
    assert_eq!(a.adapter.tick_at(), Some(500 + STAR_HOLD));
    assert_eq!(a.at(500 + STAR_HOLD - 1, Observation::Tick), None);
    assert_eq!(a.at(500 + STAR_HOLD, Observation::Tick), Some(Kind::Idle));
    assert_eq!(a.adapter.tick_at(), None);
}

#[test]
fn a2_a_spinner_after_the_held_star_cancels_the_hold() {
    let mut a = Clocked::new();
    a.signal(0, "UserPromptSubmit");
    a.signal(10, "PreToolUse");
    a.title(20, "✳ Reading");
    assert_eq!(a.title(50, "◐ Reading"), None);
    assert_eq!(a.at(20 + STAR_HOLD, Observation::Tick), None);
}

#[test]
fn a2_post_tool_use_ends_the_hold_so_a_later_star_is_idle_at_once() {
    let mut a = Clocked::new();
    a.signal(0, "UserPromptSubmit");
    a.signal(10, "PreToolUse");
    a.signal(20, "PostToolUse");
    assert_eq!(a.title(30, "✳ Done"), Some(Kind::Idle));
}

#[test]
fn a2_a_star_never_ends_needs_you_error_or_done_and_a_spinner_never_ends_needs_you() {
    let mut a = Clocked::new();
    a.signal(0, "PermissionRequest");
    assert_eq!(a.title(1, "✳ Create z.txt"), None);
    assert_eq!(a.title(2, "◐ Create z.txt"), None);
    assert_eq!(a.title(3, "◑ Create z.txt"), None);

    let mut failed = Clocked::new();
    failed.signal(0, "UserPromptSubmit");
    failed.at(
        1,
        Observation::Signal(json!({"hook_event_name": "StopFailure", "error": "x"})),
    );
    assert_eq!(failed.title(2, "✳ Claude Code"), None);

    let mut ended = Clocked::new();
    ended.signal(0, "UserPromptSubmit");
    ended.signal(1, "SessionEnd");
    assert_eq!(ended.title(2, "✳ Claude Code"), None);
}

#[test]
fn a2_a_denied_dialog_reads_needs_you_until_the_next_prompt() {
    let steps = replay(RUN3);
    let deny = mark(RUN3.1, "deny-select4");
    let next_prompt = first_after(&steps, deny, "UserPromptSubmit");
    let held: Vec<_> = steps
        .iter()
        .filter(|s| s.ts >= deny && s.ts < next_prompt)
        .collect();
    assert!(
        held.iter().any(|s| s.event.starts_with("title ◐")),
        "the deny's spinner is in the window"
    );
    assert!(
        held.iter().all(|s| s.kind == Some(Kind::NeedsYou)),
        "reads working while Claude waits"
    );
}

#[test]
fn a2_esc_on_a_dialog_changes_nothing() {
    let steps = replay(RUN3);
    let opened = first_after(
        &steps,
        mark(RUN3.1, "submit:cancelperm"),
        "PermissionRequest",
    );
    let after: Vec<_> = steps.iter().filter(|s| s.ts >= opened).collect();
    assert!(after.iter().all(|s| s.kind == Some(Kind::NeedsYou)));
    assert!(mark(RUN3.1, "esc-perm") > opened);
}

#[test]
fn a2_a_spinner_before_the_star_does_not_stop_the_hold() {
    let mut a = Clocked::new();
    a.signal(0, "UserPromptSubmit");
    a.signal(10, "PreToolUse");
    assert_eq!(a.title(20, "◐ Create z.txt"), None);
    assert_eq!(a.title(40, "✳ Create z.txt"), None);
    assert_eq!(
        a.signal(60, "PermissionRequest"),
        Some(Kind::NeedsYou),
        "no idle between"
    );
}

#[test]
fn a2_an_ignored_hook_does_not_end_the_hold() {
    for ignored in [
        "Notification",
        "SubagentStop",
        "MessageDisplay",
        "PostToolBatch",
    ] {
        let mut a = Clocked::new();
        a.signal(0, "UserPromptSubmit");
        a.signal(10, "PreToolUse");
        a.title(20, "✳ Create z.txt");
        assert_eq!(a.signal(30, ignored), None);
        assert_eq!(a.adapter.tick_at(), Some(20 + STAR_HOLD), "{ignored}");
        assert_eq!(a.signal(40, "PermissionRequest"), Some(Kind::NeedsYou));
    }
}

#[test]
fn a2_a_title_without_a_glyph_changes_nothing() {
    let mut a = Clocked::new();
    a.signal(0, "UserPromptSubmit");
    assert_eq!(a.title(1, "zsh"), None);
}

#[test]
fn a2_a_title_or_tick_after_exit_is_ignored() {
    let mut a = Clocked::new();
    a.signal(0, "UserPromptSubmit");
    a.signal(1, "PreToolUse");
    a.title(2, "✳ x");
    a.at(3, Observation::Exit { code: Some(0) });
    assert_eq!(a.adapter.tick_at(), None);
    assert_eq!(a.title(4, "◐ x"), None);
    assert_eq!(a.at(2 + STAR_HOLD, Observation::Tick), None);
}

#[test]
fn a2_esc_during_an_auto_allowed_tool_reads_idle_only_after_the_hold() {
    for run in [RUN4, RUN5] {
        let steps = replay(run);
        let esc = mark(run.1, "esc");
        let star = steps
            .iter()
            .find(|s| s.ts > esc && s.event.starts_with("title ✳"))
            .unwrap()
            .ts;
        let idle = steps
            .iter()
            .find(|s| s.ts > esc && s.kind == Some(Kind::Idle))
            .unwrap();
        assert_eq!(idle.event, "Tick");
        assert!(
            idle.ts - star >= STAR_HOLD as f64 / 1000.0 - 0.001,
            "idle {}s after the star",
            idle.ts - star
        );
        let working = steps.iter().rev().find(|s| s.ts < idle.ts).unwrap();
        assert_eq!(working.kind, Some(Kind::Working), "{}", working.event);
    }
}

#[test]
fn a2_esc_on_an_ask_dialog_changes_nothing() {
    let steps = replay(RUN2);
    let opened = first_after(&steps, mark(RUN2.1, "submit:ask"), "PermissionRequest");
    let esc = mark(RUN2.1, "esc-ask");
    assert!(esc > opened);
    assert!(changes(&steps, opened, esc + 0.5).is_empty());
    let kind = steps.iter().rev().find(|s| s.ts <= esc + 0.5).unwrap().kind;
    assert_eq!(kind, Some(Kind::NeedsYou));
}

#[test]
fn a2_esc_on_an_open_dialog_leaves_needs_you_for_the_rest_of_the_run() {
    let steps = replay(RUN6);
    let opened = first_after(&steps, 0.0, "PermissionRequest");
    assert!(mark(RUN6.1, "esc-on-dialog") > opened);
    assert!(changes(&steps, opened, f64::MAX).is_empty());
    assert_eq!(steps.last().unwrap().kind, Some(Kind::NeedsYou));
}

#[test]
fn a2_a_yes_answer_reads_needs_you_until_its_post_tool_use() {
    let steps = replay(RUN7);
    let dialogs: Vec<_> = steps
        .iter()
        .filter(|s| s.event == "PermissionRequest")
        .collect();
    assert_eq!(dialogs.len(), 3);
    for dialog in dialogs {
        let answered = first_after(&steps, dialog.ts, "PostToolUse");
        let between = steps
            .iter()
            .filter(|s| s.ts >= dialog.ts && s.ts < answered);
        assert!(between.clone().all(|s| s.kind == Some(Kind::NeedsYou)));
        assert!(
            between.count() > 1,
            "the spinner after Yes is in the window"
        );
    }
}
