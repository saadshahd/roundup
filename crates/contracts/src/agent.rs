//! Agents and the Rail: the tree of Rooms, Agents and Terminals.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::common::Status;

/// Parse the wire ordinal without accepting aliases such as leading zeroes or signs.
pub fn parse_positive_ordinal(value: &str) -> Option<i64> {
    value
        .parse::<i64>()
        .ok()
        .filter(|n| *n > 0 && n.to_string() == value)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "agent/")]
pub enum NodeKind {
    Room,
    Agent,
    Terminal,
}

/// The git worktree Provisioning made for a node's Agent (G2, `scenarios/worktrees.md`).
/// `base` is the branch that was checked out in the Project when it was made.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct Worktree {
    pub path: String,
    pub branch: String,
    pub base: String,
}

/// What `agent.worktreeState` returns (G3): commits only the branch has, commits only the Base
/// has, and whether the Worktree has a staged, unstaged or untracked change outside `.roundup/`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct WorktreeState {
    pub ahead: u32,
    pub behind: u32,
    pub dirty: bool,
}

/// What `agent.land` returns (G4): the commit the Base points at after Landing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct Landed {
    pub base: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct RailNode {
    pub id: String,
    pub kind: NodeKind,
    pub name: String,
    pub parent: Option<String>,
    /// Position among siblings; lower first.
    pub order: u32,
    /// `None` for Terminals.
    pub status: Option<Status>,
    /// Last allocated program attempt; present even after stop or reopen.
    pub attempt: Option<String>,
    /// Status transition within the current Attempt; absent after reopen.
    pub status_revision: Option<String>,
    /// The Terminal behind an Agent or Terminal node.
    pub terminal_id: Option<String>,
    /// `None` for a Terminal, or when the Project's `worktrees` setting was off.
    pub worktree: Option<Worktree>,
    /// A22: true only for an exited Agent or Door with saved conversation data (A21) and no
    /// start in flight. `false` for a Terminal.
    pub can_resume: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct SpawnParams {
    pub cwd: String,
    pub prompt: Option<String>,
    pub parent: Option<String>,
}

/// Start the user's login shell in a new Terminal and place its node last under `parent`.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct SpawnTerminalParams {
    pub cwd: String,
    pub parent: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct NodeId {
    pub id: String,
}

/// One adapter-specific Signal for an Agent; for Claude Code, a hook payload.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct SignalParams {
    pub id: String,
    pub attempt: String,
    #[ts(type = "Record<string, unknown>")]
    pub payload: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct CreateRoomParams {
    pub name: String,
    pub parent: Option<String>,
}

/// Re-parent and position a node. `parent: None` moves it to the root.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct MoveParams {
    pub id: String,
    pub parent: Option<String>,
    pub index: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct RenameParams {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct StatusEvent {
    pub id: String,
    pub attempt: String,
    pub status_revision: String,
    pub status: Status,
}
