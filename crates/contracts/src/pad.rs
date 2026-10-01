//! Pads: markdown notes owned by an Agent or the user. The owner may rewrite; everyone else may only append.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::common::Actor;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "pad/")]
pub struct Pad {
    pub name: String,
    pub owner: Actor,
    pub text: String,
    #[ts(type = "number")]
    pub updated_at: i64,
}

/// The owner is the calling Actor.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "pad/")]
pub struct CreateParams {
    pub name: String,
    pub text: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "pad/")]
pub struct PadName {
    pub name: String,
}

/// Rewrite the whole text. Owner only.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "pad/")]
pub struct WriteParams {
    pub name: String,
    pub text: String,
}

/// Add to the end. Anyone; counts as a write in Provenance.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "pad/")]
pub struct AppendParams {
    pub name: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "pad/")]
pub struct SetOwnerParams {
    pub name: String,
    pub owner: Actor,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "pad/")]
pub struct ExportParams {
    pub name: String,
    pub path: String,
}

/// Project-wide: store Pads as `.roundup/pads/<name>.md` (true) or in the app database (false).
/// Flipping to false re-imports the files.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "pad/")]
pub struct SetStorageParams {
    pub files: bool,
}
