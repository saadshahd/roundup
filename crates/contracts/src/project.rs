//! Per-Project settings (G1, `scenarios/worktrees.md`): today, only the `worktrees` toggle.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "project/")]
pub struct Worktrees {
    pub on: bool,
    /// The command Landing runs in a Worktree before it fast-forwards the Base. `None` is
    /// allowed: isolation needs no check, though Landing then refuses (`check_missing`).
    pub check: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "project/")]
pub struct ProjectSettings {
    pub worktrees: Worktrees,
}
