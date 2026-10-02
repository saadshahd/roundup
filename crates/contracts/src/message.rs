//! Messages between Actors, and the Routes that decide how a Message is delivered.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::common::Actor;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "message/")]
pub enum MessageKind {
    Note,
    Question,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "message/")]
pub enum MessageStatus {
    Pending,
    Held,
    Delivered,
    Dropped,
}

/// A typed envelope between Actors. `reply_to` names the Message this answers, if any; `reason`
/// explains a `held` or `dropped` status and is `None` otherwise.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "message/")]
pub struct Message {
    pub id: u32,
    pub from: Actor,
    pub to: String,
    pub kind: MessageKind,
    pub body: String,
    #[serde(rename = "replyTo")]
    pub reply_to: Option<u32>,
    pub status: MessageStatus,
    pub reason: Option<String>,
    #[ts(type = "number")]
    pub at: i64,
}

/// `from` is never a parameter: it is always the calling Actor.
#[derive(Clone, Debug, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "message/")]
pub struct SendParams {
    pub to: String,
    pub kind: MessageKind,
    pub body: String,
    #[serde(rename = "replyTo", default)]
    pub reply_to: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "message/")]
pub struct MessageId {
    pub id: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "message/")]
pub struct ListParams {
    pub to: Option<String>,
    pub status: Option<MessageStatus>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "message/")]
pub enum Delivery {
    Auto,
    AskFirst,
    Drop,
}

/// A sender-to-receiver delivery rule. Both `from` and `to` are Actor ids.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "message/")]
pub struct Route {
    pub from: String,
    pub to: String,
    pub delivery: Delivery,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "message/")]
pub struct SetRouteParams {
    pub from: String,
    pub to: String,
    pub delivery: Delivery,
}
