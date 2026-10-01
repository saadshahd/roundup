//! The Claude Code adapter. Hook payloads, the terminal title and the exit status become a Status
//! (ADR 0006); `session`, `hook` and `transcript` are Claude Code's own words and stay in here.

mod launch;

use contracts::{Kind, Status};
use serde_json::Value;

use crate::{AgentAdapter, Observation};

pub use launch::Launcher;

pub struct ClaudeCode {
    status: Option<Status>,
    exited: bool,
    /// Roundup killed it on purpose, so the exit that follows is not an error.
    stopped: bool,
    /// The latest Signal was `PreToolUse`: a dialog may be about to open.
    tool_pending: bool,
    clock: Box<dyn Fn() -> i64 + Send>,
}

impl ClaudeCode {
    /// `clock` returns milliseconds since the Unix epoch; it stamps every Status `since`.
    pub fn new(clock: impl Fn() -> i64 + Send + 'static) -> Self {
        Self {
            status: None,
            exited: false,
            stopped: false,
            tool_pending: false,
            clock: Box::new(clock),
        }
    }

    /// An adapter for a program that has just started: working until it says otherwise.
    pub fn starting(clock: impl Fn() -> i64 + Send + 'static) -> Self {
        let mut adapter = Self::new(clock);
        adapter.settle(Kind::Working, "starting".into());
        adapter
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

    /// Claude Code spins ◐/◑ while it works and shows ✳ whenever it is not, a permission dialog
    /// included. So a spinner means working unless a dialog is open (a denied dialog resumes the
    /// spinner with no hook, and must keep `needs-you`), and a star means idle only if the Agent
    /// was working. The dialog's star lands 40-80 ms before its `PermissionRequest`, right after
    /// `PreToolUse`; a spinner since then shows no dialog is coming.
    fn retitle(&mut self, title: &str) -> Option<Status> {
        let kind = self.status.as_ref().map(|status| status.kind);
        match title.chars().next()? {
            '◐' | '◑' if kind == Some(Kind::NeedsYou) => {
                self.tool_pending = false;
                None
            }
            '◐' | '◑' => {
                self.tool_pending = false;
                self.settle(Kind::Working, "working".into())
            }
            '✳' if kind == Some(Kind::Working) && !self.tool_pending => {
                self.settle(Kind::Idle, "idle".into())
            }
            _ => None,
        }
    }

    fn exit(&mut self, code: Option<i32>) -> Option<Status> {
        self.exited = true;
        if self.stopped {
            return None;
        }
        match code {
            Some(0) if self.status.as_ref().is_some_and(|s| s.kind == Kind::Done) => None,
            Some(0) => self.settle(Kind::Done, "exited 0".into()),
            Some(code) => self.settle(Kind::Error, format!("exited {code}")),
            None => self.settle(Kind::Error, "exited by signal".into()),
        }
    }
}

impl AgentAdapter for ClaudeCode {
    fn observe(&mut self, observation: Observation) -> Option<Status> {
        match observation {
            Observation::Exit { code } => self.exit(code),
            Observation::Stopped if self.exited => None,
            Observation::Stopped => {
                self.exited = true;
                self.stopped = true;
                self.settle(Kind::Done, "stopped".into())
            }
            Observation::Signal(_) | Observation::Title(_) if self.exited => None,
            Observation::Signal(payload) => {
                self.tool_pending = payload["hook_event_name"] == "PreToolUse";
                let (kind, label) = hook_status(&payload)?;
                self.settle(kind, label)
            }
            Observation::Title(title) => self.retitle(&title),
        }
    }
}

/// What one hook payload says about the Agent. `None` for events that say nothing: the late
/// permission Notification, the spurious `SubagentStop` and display noise; an unknown event also
/// goes to stderr.
fn hook_status(payload: &Value) -> Option<(Kind, String)> {
    let text = |field: &str| payload[field].as_str().map(str::to_owned);
    match payload["hook_event_name"].as_str() {
        Some("SessionStart" | "Stop") => Some((Kind::Idle, "idle".into())),
        Some("UserPromptSubmit" | "PreToolUse" | "PostToolUse" | "PostToolUseFailure") => {
            Some((Kind::Working, "working".into()))
        }
        Some("PermissionRequest") => Some((Kind::NeedsYou, permission_label(payload))),
        Some("StopFailure") => Some((Kind::Error, text("error")?)),
        Some("SessionEnd") => Some((Kind::Done, text("reason").unwrap_or_else(|| "done".into()))),
        Some("Notification" | "SubagentStop" | "MessageDisplay" | "PostToolBatch") => None,
        _ => {
            eprintln!("agents: ignoring an unrecognised hook payload: {payload}");
            None
        }
    }
}

/// The question being asked, or the tool and what it is about to do.
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
