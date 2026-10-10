//! Agents and the Rail: the tree of Workstreams, Agents and Terminals.

use schemars::JsonSchema;
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
    Workstream,
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

/// O1: what an Agent or a Door holds before it works, exactly one of a Work order (the ask and
/// what it must not do) or a Clarification order (the open question that comes first).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
#[ts(export, export_to = "agent/")]
pub enum Order {
    Work { ask: String, limits: Vec<String> },
    Clarification { question: String },
}

impl Order {
    /// O2: what an Agent spawned with only a `prompt` holds.
    pub fn work(ask: &str) -> Self {
        Self::Work {
            ask: ask.to_owned(),
            limits: Vec::new(),
        }
    }

    pub fn clarification(question: &str) -> Self {
        Self::Clarification {
            question: question.to_owned(),
        }
    }

    /// O1's shape: the ask or question, and every limit, non-empty after trimming.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Work { ask, limits } => {
                if ask.trim().is_empty() {
                    return Err("order ask must not be empty".into());
                }
                if limits.iter().any(|limit| limit.trim().is_empty()) {
                    return Err("order limits must not be empty".into());
                }
            }
            Self::Clarification { question } => {
                if question.trim().is_empty() {
                    return Err("order question must not be empty".into());
                }
            }
        }
        Ok(())
    }
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
    /// E6: whether the Agent's `rup mcp` has reported to the Daemon. `None` for a Workstream that is
    /// no Door, a Terminal, and an Agent with no live Terminal.
    pub channel: Option<Channel>,
    /// O1: the order of an Agent or a Workstream's Door; `None` for a Terminal. Named `work` because
    /// `order` is the position among siblings.
    pub work: Option<Order>,
    /// F5: processes in a Door's Terminal tree that carry a vendor program's name and that the
    /// Daemon did not register. Empty for every other node, and for a Door with none.
    pub stray: Vec<Stray>,
}

/// F5: one process found by name under a Door, by its pid and the command line `ps` printed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct Stray {
    pub pid: u32,
    pub command: String,
}

/// The `agent.stray` event: the Door `id` now holds exactly `stray`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct StrayEvent {
    pub id: String,
    pub stray: Vec<Stray>,
}

/// E6: `pending` from the start, `up` once `agent.channelUp` arrived, `missing` when it had not
/// within the Daemon's deadline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "agent/")]
pub enum Channel {
    Pending,
    Up,
    Missing,
}

/// The `agent.channel` event: Agent `id`'s Channel changed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct ChannelEvent {
    pub id: String,
    pub channel: Channel,
}

/// What `agent.brief` returns (E3): the text `rup context` prints, verbatim.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct Brief {
    pub stdout: String,
}

/// The node an Agent sits under: a Workstream with no running Door, or an Agent (a Door or the Agent that started it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "agent/")]
pub enum ParentNode {
    Group,
    Agent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct ContextSelf {
    pub id: String,
    pub name: String,
    pub status: Status,
    /// O4: the Agent's own order; `None` for a Terminal.
    pub order: Option<Order>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct ContextParent {
    pub id: String,
    pub name: String,
    pub node: ParentNode,
}

/// Who a Message for help goes to: a Door's id, or the user's Actor id.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct ContextAsk {
    pub to: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct ContextPeer {
    pub id: String,
    pub name: String,
    pub status: Status,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct ContextTodo {
    pub id: u32,
    pub title: String,
    pub blocked: bool,
}

/// B19: what a Door is told about one child. `last` is one line, `todos` counts the open Todos
/// whose Home is the child, and `pads` names the child's Pads written since the previous envelope.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct DigestEntry {
    pub name: String,
    pub kind: crate::Kind,
    pub last: String,
    pub todos: u32,
    pub pads: Vec<String>,
}

/// B20: one item of `agent.digest`'s `children`: a child's entry, or the last item `{more: n}` when
/// the Door has more children than the result holds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(untagged)]
#[ts(export, export_to = "agent/")]
pub enum DigestItem {
    Child(DigestEntry),
    More { more: u32 },
}

/// What `agent.digest` returns (B20).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct Digest {
    pub children: Vec<DigestItem>,
}

/// What `agent.context` returns (E2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct Context {
    #[serde(rename = "self")]
    #[ts(rename = "self")]
    pub this: ContextSelf,
    pub parent: Option<ContextParent>,
    pub ask: ContextAsk,
    pub peers: Vec<ContextPeer>,
    pub todos: Vec<ContextTodo>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "agent/")]
pub struct SpawnParams {
    pub cwd: String,
    pub prompt: Option<String>,
    pub parent: Option<String>,
    /// O2: replaces `prompt`; giving both is `INVALID_PARAMS`.
    #[serde(default)]
    #[ts(optional)]
    pub order: Option<Order>,
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

/// A Steer: `text` sent to the running Agent `id` as its next prompt (H11).
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct PromptParams {
    pub id: String,
    pub text: String,
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
pub struct CreateWorkstreamParams {
    pub name: String,
    pub parent: Option<String>,
    /// O3: the Door's order.
    #[serde(default)]
    #[ts(optional)]
    pub order: Option<Order>,
}

/// O4: replace the order of Agent or Door `id`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "agent/")]
pub struct SetOrderParams {
    pub id: String,
    pub order: Order,
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
