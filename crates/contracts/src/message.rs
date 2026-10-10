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

/// Why a Message is `held` (`ask-first`: waiting on the user; `takeover`: on a Takeover's end;
/// `escalated`: on the user's answer to a bubbled-up question, B15) or `dropped`
/// (`receiver gone`, B9; `not accepted`, B2; `passed`, a question's hop that was still `pending`
/// when the next was made, B13; `merged`, a pushed digest folded into a newer one, B22).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "message/")]
pub enum Reason {
    AskFirst,
    Takeover,
    Escalated,
    #[serde(rename = "receiver gone")]
    ReceiverGone,
    #[serde(rename = "not accepted")]
    NotAccepted,
    Passed,
    /// B22: two pushed digests folded into one Message.
    Merged,
}

/// A typed envelope between Actors. `reply_to` names the Message this answers, if any; `reason`
/// names why a `held` Message is held or a `dropped` one was dropped, and is `None` otherwise;
/// `passed_from` names the earlier hop of a bubbling question (B13), and is `None` for any other
/// Message and for a question's first hop.
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
    pub reason: Option<Reason>,
    #[serde(rename = "passedFrom")]
    pub passed_from: Option<u32>,
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

/// `takeover.begin` and `takeover.end` name the Agent to take over.
#[derive(Clone, Debug, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "message/")]
pub struct TakeoverParams {
    pub agent: String,
}

/// `takeover.changed {agent, on}`: pushed each time a Takeover of `agent` begins or ends.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "message/")]
pub struct TakeoverChanged {
    pub agent: String,
    pub on: bool,
}
