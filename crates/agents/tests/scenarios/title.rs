//! A2: the terminal title closes the gaps no hook covers, replayed from `screen*.jsonl`.

use agents::claude_code::ClaudeCode;
use agents::{AgentAdapter, Observation};
use contracts::Kind;
use serde_json::{Value, json};

const INTERRUPT_AND_DENY: &str = include_str!("../../../../spikes/hooks-state/screen2.jsonl");
const DIALOG_ESCAPED: &str = include_str!("../../../../spikes/hooks-state/screen3.jsonl");

fn adapter() -> ClaudeCode {
    ClaudeCode::new(|| 0)
}

fn signal(adapter: &mut ClaudeCode, event: &str) {
    let payload = json!({"hook_event_name": event, "tool_name": "Bash"});
    adapter.observe(Observation::Signal(payload));
}

/// The window titles the fixture's output set after the mark `from`, up to the next mark.
fn titles_after(fixture: &str, from: &str) -> Vec<String> {
    let lines: Vec<Value> = fixture
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let start = lines.iter().position(|l| l["mark"] == from).unwrap();
    lines[start + 1..]
        .iter()
        .take_while(|l| l.get("mark").is_none())
        .filter_map(|l| l["out"].as_str())
        .flat_map(|out| out.split("\x1b]0;").skip(1))
        .filter_map(|rest| rest.split('\x07').next())
        .map(str::to_owned)
        .collect()
}

fn replay(adapter: &mut ClaudeCode, titles: Vec<String>) -> Vec<Kind> {
    titles
        .into_iter()
        .filter_map(|title| adapter.observe(Observation::Title(title)))
        .map(|status| status.kind)
        .collect()
}

#[test]
fn a2_a_spinner_title_gives_working() {
    let mut adapter = adapter();
    signal(&mut adapter, "Stop");
    let status = adapter.observe(Observation::Title("◐ Say hi in 3 words".into()));
    assert_eq!(status.unwrap().kind, Kind::Working);
}

#[test]
fn a2_a_star_title_after_an_esc_interrupt_leaves_working() {
    let mut adapter = adapter();
    signal(&mut adapter, "UserPromptSubmit");
    let titles = titles_after(INTERRUPT_AND_DENY, "esc-interrupt");
    assert_eq!(replay(&mut adapter, titles), [Kind::Idle]);
}

#[test]
fn a2_a_denied_dialog_leaves_needs_you_when_the_turn_resumes_then_ends() {
    let mut adapter = adapter();
    signal(&mut adapter, "PermissionRequest");
    let titles = titles_after(INTERRUPT_AND_DENY, "deny-enter");
    assert_eq!(replay(&mut adapter, titles), [Kind::Working, Kind::Idle]);
}

/// The dialog and the idle prompt both show a star, so a star alone cannot tell them apart. The
/// rule: only a spinner (the turn went on) moves the Agent off needs-you. Esc on a dialog changes
/// no title in the fixtures (`esc-perm`), so that one gap stays open until the next hook.
#[test]
fn a2_a_star_title_does_not_clear_needs_you() {
    let mut adapter = adapter();
    signal(&mut adapter, "PermissionRequest");
    let star = adapter.observe(Observation::Title("✳ Create z.txt".into()));
    assert_eq!(star, None);
    assert!(titles_after(DIALOG_ESCAPED, "esc-perm").is_empty());
}

#[test]
fn a2_a_star_title_leaves_idle_alone() {
    let mut adapter = adapter();
    signal(&mut adapter, "Stop");
    assert_eq!(
        adapter.observe(Observation::Title("✳ Claude Code".into())),
        None
    );
}

#[test]
fn a2_a_title_without_a_glyph_changes_nothing() {
    let mut adapter = adapter();
    signal(&mut adapter, "UserPromptSubmit");
    assert_eq!(adapter.observe(Observation::Title("zsh".into())), None);
}

#[test]
fn a2_a_title_after_exit_is_ignored() {
    let mut adapter = adapter();
    adapter.observe(Observation::Exit { code: Some(0) });
    assert_eq!(adapter.observe(Observation::Title("◐ x".into())), None);
}
