use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::common::Actor;
use crate::{agent, decision, pad, terminal, todo};

/// Pushed to subscribers as a JSON-RPC notification: `{"method":"event","params":<Event>}`.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "name", content = "data")]
#[ts(export)]
pub enum EventData {
    #[serde(rename = "terminal.output")]
    TerminalOutput(terminal::OutputEvent),
    #[serde(rename = "terminal.title")]
    TerminalTitle(terminal::TitleEvent),
    #[serde(rename = "terminal.exited")]
    TerminalExited(terminal::ExitedEvent),
    #[serde(rename = "todo.created")]
    TodoCreated(todo::Todo),
    #[serde(rename = "todo.updated")]
    TodoUpdated(todo::Todo),
    #[serde(rename = "todo.unblocked")]
    TodoUnblocked(todo::TodoId),
    #[serde(rename = "todo.deleted")]
    TodoDeleted(todo::TodoId),
    #[serde(rename = "pad.changed")]
    PadChanged(pad::PadName),
    #[serde(rename = "agent.status")]
    AgentStatus(agent::StatusEvent),
    #[serde(rename = "rail.changed")]
    RailChanged,
    #[serde(rename = "decision.opened")]
    DecisionOpened(decision::Decision),
    #[serde(rename = "decision.cleared")]
    DecisionCleared(decision::ClearedEvent),
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Event {
    pub actor: Actor,
    #[serde(flatten)]
    pub data: EventData,
}
