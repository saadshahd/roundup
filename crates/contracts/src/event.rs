use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::common::Actor;
use crate::{agent, message, pad, terminal, todo};

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
    #[serde(rename = "message.sent")]
    MessageSent(message::Message),
    #[serde(rename = "message.held")]
    MessageHeld(message::Message),
    #[serde(rename = "message.delivered")]
    MessageDelivered(message::Message),
    #[serde(rename = "message.dropped")]
    MessageDropped(message::Message),
    #[serde(rename = "route.changed")]
    RouteChanged(message::Route),
    #[serde(rename = "agent.status")]
    AgentStatus(agent::StatusEvent),
    #[serde(rename = "rail.changed")]
    RailChanged,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Event {
    pub actor: Actor,
    #[serde(flatten)]
    pub data: EventData,
}
