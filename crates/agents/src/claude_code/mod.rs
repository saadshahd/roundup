//! The Claude Code adapter. Hook payloads, the terminal title and the exit status become a Status
//! (ADR 0006); `session`, `hook` and `transcript` are Claude Code's own words, so they must not
//! leak out of this module into public names.

mod brief;
mod conversation;
pub mod extras;
pub mod inbox;
mod launch;
mod models;
mod placement;
pub use placement::Call as PlacementCall;
pub mod plugins;
mod skills;

use contracts::decision::Answer;
use contracts::{Kind, Status};
use serde_json::Value;

use crate::{AgentAdapter, Observation};

pub use brief::Role;
pub use launch::{
    DOOR_TOOLS, DOOR_TOOLS_NO_MCP, Folders, Launcher, Lean, PROFILES, Pair, Profile, SCOUT_TOOLS,
    SCOUT_TOOLS_NO_MCP, STATE_EVENTS, profile, settings_marker,
};
pub use models::{efforts, models};
pub use skills::{BANNED, skills};

/// F4, F5: the program names this Adapter's vendor CLI is started by. The shim and the process
/// scan read the names of every Adapter through a function like this one, and write none.
pub fn programs() -> &'static [&'static str] {
    &["claude"]
}

/// Env vars Claude Code sets when the program running it is itself inside a Claude Code run
/// (A15). The Daemon can inherit these from its own environment; every program it starts must
/// not, or it would believe itself nested in that run too. Handed to [`terminal::Terminals`]
/// at `open_with` time, so no other module needs to name them.
pub const MARKERS: [&str; 8] = [
    "CLAUDECODE",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
];

/// A30: variables that make Claude Code bill per token or leave the user's plan, ranked above the
/// subscription login in code.claude.com/docs/en/authentication. `CLAUDE_CODE_OAUTH_TOKEN` is the
/// user's plan and stays.
pub const OFF_PLAN: [&str; 9] = [
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_USE_FOUNDRY",
    "ANTHROPIC_PROFILE",
    "ANTHROPIC_FEDERATION_RULE_ID",
    "ANTHROPIC_ORGANIZATION_ID",
    "ANTHROPIC_BASE_URL",
];

/// How long a star right after `PreToolUse` waits for its `PermissionRequest`, in milliseconds,
/// measured from the star. Across 9 dialogs the star precedes its `PermissionRequest` by 18-79 ms;
/// 200 ms is about 2.5 times that. Shorter flickers `idle` before a dialog; longer delays the
/// `idle` of an Esc during an auto-allowed tool.
pub const STAR_HOLD: i64 = 200;

/// How the program's life ended, if it has.
#[derive(Clone, Copy, PartialEq)]
enum Ending {
    Running,
    Exited,
    /// Roundup killed it on purpose, so the exit that follows is not an error.
    Stopped,
    /// A21: roundup could not save the Agent's conversation. Final like `Stopped`: the Exit that
    /// follows (roundup kills the Terminal once this is set) must not overwrite the error.
    Failed,
}

pub struct ClaudeCode {
    status: Option<Status>,
    ending: Ending,
    /// The latest recognised Signal was `PreToolUse`: a dialog may be about to open.
    after_pre_tool_use: bool,
    /// When a star after `PreToolUse` was first seen and not yet resolved.
    star_held_since: Option<i64>,
    clock: Box<dyn Fn() -> i64 + Send>,
}

impl ClaudeCode {
    /// `clock` returns milliseconds since the Unix epoch; it stamps every Status `since`.
    pub fn new(clock: impl Fn() -> i64 + Send + 'static) -> Self {
        Self {
            status: None,
            ending: Ending::Running,
            after_pre_tool_use: false,
            star_held_since: None,
            clock: Box::new(clock),
        }
    }

    /// An adapter for a program that has just started: working until it says otherwise.
    pub fn starting(clock: impl Fn() -> i64 + Send + 'static) -> Self {
        let mut adapter = Self::new(clock);
        adapter.settle(Kind::Working, "starting".into());
        adapter
    }

    pub(super) fn is_ended(&self) -> bool {
        self.ending != Ending::Running
    }

    pub fn status(&self) -> Option<&Status> {
        self.status.as_ref()
    }

    /// Adopt `kind` and `label`. `since` moves only when the Kind does.
    fn settle(&mut self, kind: Kind, label: String) -> Option<Status> {
        let since = match &self.status {
            Some(current) if current.kind == kind => current.since,
            _ => (self.clock)(),
        };
        let next = Status { kind, label, since };
        if self.status.as_ref() == Some(&next) {
            return None;
        }
        self.status = Some(next.clone());
        Some(next)
    }

    /// When the adapter wants an `Observation::Tick`: `STAR_HOLD` after the star it is holding.
    pub fn tick_at(&self) -> Option<i64> {
        self.star_held_since.map(|since| since + STAR_HOLD)
    }

    /// Claude Code spins ◐/◑ while it works and shows ✳ whenever it is not, a permission dialog
    /// included. A spinner means working, but never ends `needs-you` (a denied dialog resumes the
    /// spinner with no hook). A star means idle only for a working Agent, so it never ends
    /// `needs-you`, `error` or `done`. The one star not read at once follows `PreToolUse`: it may
    /// be the dialog's, which precedes its `PermissionRequest`, so it is held for `STAR_HOLD`;
    /// reading it as idle would flicker `idle` before `needs-you`. A spinner after that star shows
    /// the tool still running and drops the hold.
    fn retitle(&mut self, title: &str) -> Option<Status> {
        let kind = self.status.as_ref().map(|status| status.kind);
        match title.chars().next()? {
            '◐' | '◑' => {
                self.star_held_since = None;
                if kind == Some(Kind::NeedsYou) {
                    return None;
                }
                self.settle(Kind::Working, "working".into())
            }
            '✳' if kind == Some(Kind::Working) && self.after_pre_tool_use => {
                let now = (self.clock)();
                self.star_held_since.get_or_insert(now);
                None
            }
            '✳' if kind == Some(Kind::Working) => self.settle(Kind::Idle, "idle".into()),
            _ => None,
        }
    }

    /// A held star that no `PermissionRequest` followed was a plain idle.
    fn tick(&mut self) -> Option<Status> {
        self.star_held_since
            .filter(|since| (self.clock)() - since >= STAR_HOLD)?;
        self.star_held_since = None;
        self.settle(Kind::Idle, "idle".into())
    }

    fn signal(&mut self, payload: &Value) -> Option<Status> {
        let (kind, label) = hook_status(payload)?;
        self.after_pre_tool_use = payload["hook_event_name"] == "PreToolUse";
        self.star_held_since = None;
        self.settle(kind, label)
    }

    fn exit(&mut self, code: Option<i32>) -> Option<Status> {
        self.star_held_since = None;
        if matches!(self.ending, Ending::Stopped | Ending::Failed) {
            return None;
        }
        self.ending = Ending::Exited;
        match code {
            Some(0) if self.status.as_ref().is_some_and(|s| s.kind == Kind::Done) => None,
            Some(0) => self.settle(Kind::Done, "exited 0".into()),
            Some(code) => self.settle(Kind::Error, format!("exited {code}")),
            None => self.settle(Kind::Error, "exited by signal".into()),
        }
    }

    /// H12: the first prompt was not accepted. Not final: a Signal that arrives later (the user
    /// submitting the text left in the input line) moves the Status on as usual.
    pub fn refuse_prompt(&mut self) -> Option<Status> {
        if self.ending != Ending::Running {
            return None;
        }
        self.star_held_since = None;
        self.settle(Kind::Error, "prompt not accepted".into())
    }

    /// A21: roundup itself could not save the Agent's conversation. Final: no later Observation
    /// changes the Status again, the Exit that follows the Terminal being stopped included.
    pub fn fail(&mut self, label: String) -> Option<Status> {
        if self.ending != Ending::Running {
            return None;
        }
        self.ending = Ending::Failed;
        self.star_held_since = None;
        self.settle(Kind::Error, label)
    }
}

impl AgentAdapter for ClaudeCode {
    fn observe(&mut self, observation: Observation) -> Option<Status> {
        match observation {
            Observation::Exit { code } => self.exit(code),
            Observation::Stopped if self.ending != Ending::Running => None,
            Observation::Stopped => {
                self.ending = Ending::Stopped;
                self.star_held_since = None;
                self.settle(Kind::Done, "stopped".into())
            }
            Observation::Signal(_)
            | Observation::Title(_)
            | Observation::Tick
            | Observation::Answered
            | Observation::Dismissed
                if self.ending != Ending::Running =>
            {
                None
            }
            Observation::Tick => self.tick(),
            Observation::Signal(payload) => self.signal(&payload),
            Observation::Title(title) => self.retitle(&title),
            Observation::Answered => {
                self.star_held_since = None;
                self.settle(Kind::Working, "working".into())
            }
            Observation::Dismissed => {
                self.star_held_since = None;
                self.settle(Kind::Idle, "idle".into())
            }
        }
    }
}

/// E3: mutable Context travels after the work text so A27's name still comes from the work.
pub(crate) const CONTEXT_START: &str = "\n\n<roundup-context>\n";

/// The hook event a payload names, or `Signal` when it names none.
pub fn event_name(payload: &Value) -> &str {
    payload["hook_event_name"].as_str().unwrap_or("Signal")
}

/// The prompt a `UserPromptSubmit` payload carries (empty when it carries none); `None` for any
/// other payload.
pub fn submitted_prompt(payload: &Value) -> Option<&str> {
    (payload["hook_event_name"] == "UserPromptSubmit")
        .then(|| payload["prompt"].as_str().unwrap_or_default())
}

/// B39: a paste that Claude Code shows as a Paste placeholder reaches `UserPromptSubmit` inside an
/// envelope (seen with 2.1.302): `<pasted_content id="…">`, a newline, the pasted text, a newline,
/// `</pasted_content id="…">`. The pasted text of the envelope that ends `prompt`, or `prompt` when
/// none ends it.
pub(crate) fn pasted_text(prompt: &str) -> &str {
    const CLOSE: &str = "\n</pasted_content id=\"";
    let trimmed = prompt.trim_end();
    let Some(close_at) = trimmed.rfind(CLOSE) else {
        return prompt;
    };
    let Some(id) = trimmed[close_at + CLOSE.len()..]
        .strip_suffix("\">")
        .filter(|id| !id.contains(['"', '\n']))
    else {
        return prompt;
    };
    let open = format!("<pasted_content id=\"{id}\">\n");
    trimmed[..close_at]
        .rfind(&open)
        .map_or(prompt, |open_at| &trimmed[open_at + open.len()..close_at])
}

/// Whether `payload` is the `SessionStart` Signal, which readies a fresh Agent for its first
/// prompt (H12).
pub fn is_session_start(payload: &Value) -> bool {
    payload["hook_event_name"] == "SessionStart"
}

/// E3: what the `SessionStart` hook prints so Claude Code adds `text` to the model's context: the
/// vendor's own JSON, `hookSpecificOutput.additionalContext`.
pub fn session_start_context(text: &str) -> String {
    serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": text,
        }
    })
    .to_string()
}

/// H20: typed outside the paste markers, so the model's first user message holds typed text that
/// asks it to act on the pasted block instead of treating the block as bare quoted paste.
pub const LEAD_IN: &str = "Follow this instruction: ";

/// B33: typed instead of `LEAD_IN` before a prompt that holds only pushed Digests, which report
/// state and ask for nothing.
pub const STATUS_LEAD: &str = "Status, nothing to follow: ";

/// What B30 writes before each pushed Digest of a batch.
const DIGEST_HEAD: &str = "[from rupd, note] [digest]";

/// The lead a Steer of `text` is typed with: `STATUS_LEAD` when every Message of the batch (the
/// paragraphs B30 joins) is a pushed Digest, `LEAD_IN` otherwise. B39's id line closing the prompt is
/// not a Message.
fn lead_of(text: &str) -> &'static str {
    let parts = text
        .split("\n\n")
        .filter(|part| !part.starts_with("[rup messages:"));
    if parts.into_iter().all(|part| part.starts_with(DIGEST_HEAD)) {
        STATUS_LEAD
    } else {
        LEAD_IN
    }
}

/// The bytes of a Steer (H11, H20, B33): `LEAD_IN` (or `STATUS_LEAD`), then `text` as one bracketed paste, then `\r`, written
/// as `write_submitted` does. A paste followed by a pause submitted 8 of 8 times in the spike and a bare
/// `text\r` failed once, so the paste markers stay. `text` loses its `ESC` bytes, so it cannot
/// close the paste early and type what follows as keys.
pub fn steer_bytes(text: &str) -> Vec<u8> {
    let mut bytes = lead_of(text).as_bytes().to_vec();
    bytes.extend(verbatim_bytes(&text.replace('\x1b', "")));
    bytes
}

/// A41: retain the first slash of the user's Ask while keeping H11's paste fence.
pub(super) fn verbatim_bytes(text: &str) -> Vec<u8> {
    let mut bytes = b"\x1b[200~".to_vec();
    bytes.extend_from_slice(text.as_bytes());
    bytes.extend_from_slice(b"\x1b[201~\r");
    bytes
}

/// How long the `\r` of a Steer waits behind its paste: Claude Code takes an `\r` that arrives
/// with the end of a paste as part of the paste and leaves the text unsubmitted.
const SUBMIT_PAUSE: std::time::Duration = std::time::Duration::from_millis(100);

/// Write `bytes` from `steer_bytes` or `verbatim_bytes`: everything up to the final `\r`, a
/// flush, `SUBMIT_PAUSE`, then the `\r` on its own. The Terminal's input thread runs this whole,
/// so no other write comes between the two.
pub(super) fn write_submitted(
    writer: &mut dyn std::io::Write,
    bytes: &[u8],
) -> std::io::Result<()> {
    let paste = bytes
        .strip_suffix(b"\r")
        .expect("a Steer's bytes end with the submitting carriage return");
    writer.write_all(paste)?;
    writer.flush()?;
    std::thread::sleep(SUBMIT_PAUSE);
    writer.write_all(b"\r")?;
    writer.flush()
}

/// The bytes of an Interrupt (H13): one `ESC`, which ends Claude Code's turn.
pub fn interrupt_bytes() -> Vec<u8> {
    vec![0x1b]
}

/// A21: the conversation a `SessionStart` payload names, accepted only as a UUID; `None` for any
/// other payload, or one whose `session_id` is not a UUID.
pub fn conversation_id(payload: &Value) -> Option<String> {
    (payload["hook_event_name"] == "SessionStart")
        .then(|| payload["session_id"].as_str())
        .flatten()
        .filter(|id| is_uuid(id))
        .map(str::to_owned)
}

/// A22: whether a `SessionStart` payload says it began with `--resume`; `None` for any other
/// payload.
pub fn session_source(payload: &Value) -> Option<&str> {
    (payload["hook_event_name"] == "SessionStart")
        .then(|| payload["source"].as_str())
        .flatten()
}

/// `8-4-4-4-12` lowercase or uppercase hex, dashes at those exact positions; no other form (no
/// braces, no `urn:uuid:`) is accepted as a conversation id.
fn is_uuid(id: &str) -> bool {
    let bytes = id.as_bytes();
    bytes.len() == 36
        && [8, 13, 18, 23].iter().all(|&i| bytes[i] == b'-')
        && bytes
            .iter()
            .enumerate()
            .all(|(i, b)| matches!(i, 8 | 13 | 18 | 23) || b.is_ascii_hexdigit())
}

/// What one hook payload says about the Agent. `None` for events that say nothing: the late
/// permission Notification, the spurious `SubagentStop` and display noise; an unknown event also
/// goes to stderr.
fn hook_status(payload: &Value) -> Option<(Kind, String)> {
    let text = |field: &str| payload[field].as_str().map(str::to_owned);
    // Only the event name is logged: the rest of a payload carries prompts and tool input.
    let Some(event) = payload["hook_event_name"].as_str() else {
        eprintln!("agents: ignoring a hook payload with no event name");
        return None;
    };
    match event {
        "SessionStart" | "Stop" => Some((Kind::Idle, "idle".into())),
        "UserPromptSubmit" | "PreToolUse" | "PostToolUse" | "PostToolUseFailure" => {
            Some((Kind::Working, "working".into()))
        }
        "PermissionRequest" => Some((Kind::NeedsYou, permission_label(payload))),
        "StopFailure" => Some((Kind::Error, text("error").unwrap_or_else(|| "error".into()))),
        "SessionEnd" => Some((Kind::Done, text("reason").unwrap_or_else(|| "done".into()))),
        "Notification" | "SubagentStop" | "MessageDisplay" | "PostToolBatch" => None,
        other => {
            eprintln!("agents: ignoring an unrecognised hook event {other}");
            None
        }
    }
}

/// The question being asked; else the tool name, with its command when it has one.
fn permission_label(payload: &Value) -> String {
    let tool = payload["tool_name"].as_str().unwrap_or("tool");
    let input = &payload["tool_input"];
    if let Some(question) = input["questions"][0]["question"].as_str() {
        return question.to_owned();
    }
    match input["command"].as_str() {
        Some(command) => format!("{tool}: {command}"),
        None => tool.to_owned(),
    }
}

/// The `timeout`, in seconds, of the `PermissionRequest` hook in the per-Agent settings (H9).
pub const PERMISSION_TIMEOUT_SECS: u64 = 86400;

/// How many characters of a tool's input a Decision's `args` keeps (H2).
const ARGS_LIMIT: usize = 2000;

/// What a `PermissionRequest` payload says the Decision is about (H2, H8).
pub struct PermissionRequest {
    pub tool: String,
    pub args: String,
    /// `false` when no hook reply can answer it: `AskUserQuestion` (H1c) or a payload this cannot
    /// read (H8).
    pub answerable: bool,
    /// B6: an unreadable Signal identifies no original prompt for guarded Terminal bytes.
    pub identity_available: bool,
}

/// The Decision a `PermissionRequest` payload opens: the tool name, its input as text cut at
/// `ARGS_LIMIT` characters with a final `…`, and whether a hook reply can answer it. An
/// `AskUserQuestion` reads as lines of text (H22).
pub fn permission_request(payload: &Value) -> PermissionRequest {
    let tool = payload["tool_name"].as_str();
    let input = &payload["tool_input"];
    let asked = if tool == Some("AskUserQuestion") {
        questions_text(&input["questions"])
    } else {
        None
    };
    let args = match asked {
        Some(text) => cut_at_line(text),
        None if tool == Some("AskUserQuestion") => cut(input["questions"].to_string(), ARGS_LIMIT),
        None if input.is_null() => String::new(),
        None => cut(input.to_string(), ARGS_LIMIT),
    };
    PermissionRequest {
        identity_available: tool.is_some_and(|tool| !tool.is_empty())
            && payload["hook_event_name"] == "PermissionRequest",
        answerable: tool.is_some()
            && tool != Some("AskUserQuestion")
            && payload["hook_event_name"] == "PermissionRequest",
        tool: tool.unwrap_or_default().to_owned(),
        args,
    }
}

/// `text` cut at `limit` characters with a final `…`.
fn cut(text: String, limit: usize) -> String {
    match text.char_indices().nth(limit) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text,
    }
}

/// How many characters an `AskUserQuestion`'s `args` keeps (H22).
const QUESTIONS_LIMIT: usize = 20000;

/// `text` cut at the end of the last line that fits `QUESTIONS_LIMIT`, with a final `…` (H22).
fn cut_at_line(text: String) -> String {
    let Some((end, _)) = text.char_indices().nth(QUESTIONS_LIMIT) else {
        return text;
    };
    match text[..end].rfind('\n') {
        Some(line_end) => format!("{}…", text[..line_end].trim_end()),
        None => cut(text, QUESTIONS_LIMIT),
    }
}

/// An `AskUserQuestion`'s `questions` as plain lines (H22); `None` when it is not a non-empty list
/// of questions with options.
fn questions_text(questions: &Value) -> Option<String> {
    let asked = questions.as_array().filter(|list| !list.is_empty())?;
    let mut blocks = Vec::new();
    for item in asked {
        let mut lines = Vec::new();
        if let Some(header) = item["header"].as_str().filter(|header| !header.is_empty()) {
            lines.push(header.to_owned());
        }
        let multiple = if item["multiSelect"] == true {
            " (choose several)"
        } else {
            ""
        };
        lines.push(format!("{}{multiple}", item["question"].as_str()?));
        for (n, option) in item["options"].as_array()?.iter().enumerate() {
            lines.push(format!("{}. {}", n + 1, option["label"].as_str()?));
            for field in ["description", "preview"] {
                let text = option[field].as_str().unwrap_or_default();
                lines.extend(
                    text.lines()
                        .filter(|_| !text.is_empty())
                        .map(|line| format!("   {line}")),
                );
            }
        }
        blocks.push(lines.join("\n"));
    }
    Some(blocks.join("\n\n"))
}

/// `payload` as the `PermissionRequest` Signal the adapter reads, whatever event name it carried:
/// an Agent is `needs-you` exactly while its Decision is open.
pub fn as_permission_signal(mut payload: Value) -> Value {
    match payload.as_object_mut() {
        Some(fields) => {
            fields.insert("hook_event_name".into(), "PermissionRequest".into());
            payload
        }
        None => serde_json::json!({"hook_event_name": "PermissionRequest"}),
    }
}

/// Whether a Signal means the tool was answered, ran or was abandoned in the Terminal, so an open
/// Decision is over (H7(a)).
pub fn ends_decision(payload: &Value) -> bool {
    matches!(
        payload["hook_event_name"].as_str(),
        Some("PostToolUse" | "PostToolUseFailure" | "Stop" | "UserPromptSubmit")
    )
}

/// What the hook prints for `answer`, exactly the replies `spikes/hooks-permission/REPORT.md`
/// findings 12 and 13 record.
pub fn reply(answer: Answer) -> String {
    match answer {
        Answer::Allow => r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}"#.into(),
        Answer::Deny => r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","message":"Denied in roundup"}}}"#.into(),
    }
}
