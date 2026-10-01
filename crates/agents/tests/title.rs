//! A2: the terminal title closes the gaps no hook covers. Titles come from `screen*.jsonl`, hooks
//! from the `log*.jsonl` of the same run, interleaved by timestamp.

use agents::claude_code::ClaudeCode;
use agents::{AgentAdapter, Observation};
use contracts::Kind;
use serde_json::{Value, json};

const RUN1: (&str, &str) = (
    include_str!("../../../spikes/hooks-state/log.run1.jsonl"),
    include_str!("../../../spikes/hooks-state/screen.jsonl"),
);
const RUN2: (&str, &str) = (
    include_str!("../../../spikes/hooks-state/log.run2.jsonl"),
    include_str!("../../../spikes/hooks-state/screen2.jsonl"),
);
const RUN3: (&str, &str) = (
    include_str!("../../../spikes/hooks-state/log.run3.jsonl"),
    include_str!("../../../spikes/hooks-state/screen3.jsonl"),
);

fn adapter() -> ClaudeCode {
    ClaudeCode::new(|| 0)
}

fn signal(adapter: &mut ClaudeCode, event: &str) -> Option<Kind> {
    let payload = json!({"hook_event_name": event, "tool_name": "Bash"});
    adapter
        .observe(Observation::Signal(payload))
        .map(|status| status.kind)
}

fn title(adapter: &mut ClaudeCode, title: &str) -> Option<Kind> {
    adapter
        .observe(Observation::Title(title.into()))
        .map(|status| status.kind)
}

fn lines(jsonl: &str) -> Vec<Value> {
    jsonl
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// Every hook and every title of one run, in the order they happened.
fn timeline((log, screen): (&str, &str)) -> Vec<(f64, Observation)> {
    let hooks = lines(log).into_iter().map(|line| {
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

/// The Status changes a run causes, each with the time it happened.
fn replay(run: (&str, &str)) -> Vec<(f64, Kind)> {
    let mut adapter = adapter();
    timeline(run)
        .into_iter()
        .filter_map(|(ts, observation)| adapter.observe(observation).map(|s| (ts, s.kind)))
        .collect()
}

fn kinds_between(changes: &[(f64, Kind)], from: f64, to: f64) -> Vec<Kind> {
    changes
        .iter()
        .filter(|(ts, _)| *ts > from && *ts < to)
        .map(|(_, kind)| *kind)
        .collect()
}

#[test]
fn a2_a_spinner_title_gives_working() {
    let mut adapter = adapter();
    signal(&mut adapter, "Stop");
    assert_eq!(
        title(&mut adapter, "◐ Say hi in 3 words"),
        Some(Kind::Working)
    );
}

#[test]
fn a2_a_star_title_after_an_esc_interrupt_gives_idle() {
    let changes = replay(RUN2);
    // The story prompt (978.54) is interrupted with Esc at 982.55; the star lands at 982.62.
    let after = kinds_between(&changes, 1_790_860_978.0, 1_790_860_990.0);
    assert_eq!(after, [Kind::Working, Kind::Idle]);
}

#[test]
fn a2_a_star_between_pre_tool_use_and_its_permission_request_is_ignored() {
    for run in [RUN1, RUN2, RUN3] {
        let changes = replay(run);
        for (at, (_, kind)) in changes.iter().enumerate() {
            if *kind == Kind::NeedsYou && at > 0 {
                assert_ne!(changes[at - 1].1, Kind::Idle, "idle before needs-you");
            }
        }
    }
}

#[test]
fn a2_a_star_alone_does_not_end_needs_you_nor_a_spinner() {
    let mut adapter = adapter();
    signal(&mut adapter, "PermissionRequest");
    assert_eq!(title(&mut adapter, "✳ Create z.txt"), None);
    assert_eq!(title(&mut adapter, "◐ Create z.txt"), None);
    assert_eq!(title(&mut adapter, "◑ Create z.txt"), None);
}

#[test]
fn a2_a_denied_dialog_holds_needs_you_until_the_next_prompt() {
    let changes = replay(RUN3);
    // First dialog opens at 228.46; the deny (spinner title at 239.11) and 20 s of Claude at its
    // prompt follow; the next prompt is submitted at 260.58.
    let held = kinds_between(&changes, 1_790_861_228.5, 1_790_861_260.5);
    assert_eq!(held, Vec::<Kind>::new());
}

#[test]
fn a2_a_star_after_a_tool_that_needed_no_dialog_gives_idle() {
    let mut adapter = adapter();
    signal(&mut adapter, "UserPromptSubmit");
    signal(&mut adapter, "PreToolUse");
    assert_eq!(title(&mut adapter, "◐ Reading"), None);
    assert_eq!(title(&mut adapter, "✳ Reading"), Some(Kind::Idle));
}

#[test]
fn a2_a_title_without_a_glyph_changes_nothing() {
    let mut adapter = adapter();
    signal(&mut adapter, "UserPromptSubmit");
    assert_eq!(title(&mut adapter, "zsh"), None);
}

#[test]
fn a2_a_title_after_exit_is_ignored() {
    let mut adapter = adapter();
    adapter.observe(Observation::Exit { code: Some(0) });
    assert_eq!(title(&mut adapter, "◐ x"), None);
}
