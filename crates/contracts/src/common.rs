use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ActorKind {
    User,
    Agent,
    Ext,
}

/// Who makes a call. `parent` is the Agent that started it, if any.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export)]
pub struct Actor {
    pub kind: ActorKind,
    pub id: String,
    pub parent: Option<String>,
}

impl Actor {
    pub fn user() -> Self {
        Self {
            kind: ActorKind::User,
            id: "you".into(),
            parent: None,
        }
    }

    /// The Daemon itself, for events no client call caused.
    pub fn daemon() -> Self {
        Self {
            kind: ActorKind::Ext,
            id: "rupd".into(),
            parent: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum Kind {
    Error,
    NeedsYou,
    Blocked,
    Working,
    Idle,
    Done,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Status {
    pub kind: Kind,
    pub label: String,
    /// Milliseconds since the Unix epoch.
    #[ts(type = "number")]
    pub since: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum Verb {
    Read,
    Wrote,
}

/// One logged read or write of a Todo, Pad or Agent order. `item` is `todo:<id>`, `pad:<name>` or `agent:<id>`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Touch {
    pub actor: Actor,
    pub verb: Verb,
    pub item: String,
    #[ts(type = "number")]
    pub at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct IdentifyParams {
    pub actor: Actor,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct HistoryParams {
    pub item: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TouchedParams {
    pub actor_id: String,
}
