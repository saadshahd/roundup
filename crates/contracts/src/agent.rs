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

/// A34: what a Workstream's removal took off the Rail, the Todos and the Pads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "agent/")]
pub enum RemovalKind {
    Agent,
    Todo,
    Pad,
}

/// A34: why a Workstream's removal kept an item at the Project root.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "agent/")]
pub enum KeptReason {
    Running,
    Dirty,
    Ahead,
    Open,
}

/// A34: one item `rail.remove` of a Workstream deleted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct RemovedItem {
    pub id: String,
    pub kind: RemovalKind,
    pub name: String,
}

/// A34: one item `rail.remove` of a Workstream moved to the Project root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct KeptItem {
    pub id: String,
    pub kind: RemovalKind,
    pub name: String,
    pub reason: KeptReason,
}

/// A34: what `rail.remove` of a Workstream returns.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct Removal {
    pub removed: Vec<RemovedItem>,
    pub kept: Vec<KeptItem>,
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
    Work {
        /// O14: the short name of the order; absent from an old caller's JSON.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        title: Option<String>,
        ask: String,
        /// O7: how the Agent knows it is done; absent from an old caller's JSON.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        done: Option<String>,
        limits: Vec<String>,
    },
    Clarification {
        question: String,
    },
}

impl Order {
    /// O2: what an Agent spawned with only a `prompt` holds.
    pub fn work(ask: &str) -> Self {
        Self::Work {
            title: None,
            ask: ask.to_owned(),
            done: None,
            limits: Vec::new(),
        }
    }

    pub fn clarification(question: &str) -> Self {
        Self::Clarification {
            question: question.to_owned(),
        }
    }

    /// O1's shape: the ask or question, a present done condition and every limit, non-empty after
    /// trimming.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Work {
                title,
                ask,
                done,
                limits,
            } => {
                if ask.trim().is_empty() {
                    return Err("order ask must not be empty".into());
                }
                if title
                    .as_deref()
                    .is_some_and(|title| title.trim().is_empty())
                {
                    return Err("order title must not be empty".into());
                }
                if done.as_deref().is_some_and(|done| done.trim().is_empty()) {
                    return Err("order done must not be empty".into());
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

    /// O7: `validate` for an explicit order of a spawn, a Workstream or `agent.setOrder`, which
    /// also holds the ask to `ASK_CAP` characters.
    pub fn validate_explicit(&self) -> Result<(), String> {
        self.validate()?;
        if let Self::Work { title, ask, .. } = self {
            if ask.trim().chars().count() > ASK_CAP {
                return Err(format!("order ask is over {ASK_CAP} characters"));
            }
            if title
                .as_deref()
                .is_some_and(|title| title.split_whitespace().count() > TITLE_CAP)
            {
                return Err(format!("order title is over {TITLE_CAP} words"));
            }
        }
        Ok(())
    }
}

/// O7: the longest `ask` of an explicit order, in Unicode scalar values after trimming.
pub const ASK_CAP: usize = 200;

/// O14: the most words of a Work order's `title`, split on whitespace.
pub const TITLE_CAP: usize = 6;

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
    /// A36: the Model the Agent runs on; `None` passes none. `None` for a Terminal.
    pub model: Option<String>,
    /// A36: the Effort the Agent runs on; `None` passes none. `None` for a Terminal.
    pub effort: Option<String>,
    /// A32: the Iteration of a Workstream's Door, from `1`; absent on every other node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub iteration: Option<u32>,
    /// B51: whether the Claude inbox a `note` is written to is usable, from the Agent's latest
    /// check; absent before the first check and on a Terminal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub inbox: Option<Inbox>,
}

/// B51: the outcome of checking an Agent's Claude inbox: `ok`, `missing` (absent or not writable)
/// or `mismatch` (present, in a format the adapter holds no fixture for).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "agent/")]
pub enum Inbox {
    Ok,
    Missing,
    Mismatch,
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

/// A31: one Skill a Door can call, by the name the user types after `/`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct Skill {
    pub name: String,
    /// One line of at most 160 characters; empty when the Skill has none.
    pub description: String,
}

/// What `agent.skills` returns (A31), sorted by `name`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct Skills {
    pub skills: Vec<Skill>,
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

/// B41: ask to be woken when the named Agents finish.
#[derive(Clone, Debug, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "agent/")]
pub struct WakeParams {
    /// The Agents to wait on, one to twenty, none of them yourself.
    pub ids: Vec<String>,
    /// Wake when the first of them finishes; the default is when every one has.
    #[serde(default)]
    #[ts(optional)]
    pub any: Option<bool>,
}

/// What `agent.wake` returns (B41, B48): the request's number, whether it already fired, and,
/// when it did not, why the request waits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct Woken {
    pub id: u32,
    pub fired: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub why: Option<String>,
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
    /// O2: replaces `prompt` for a user's spawn; an Agent's spawn gives both, and `prompt` alone is
    /// `INVALID_PARAMS` (O8).
    #[serde(default)]
    #[ts(optional)]
    pub order: Option<Order>,
    /// A28: the child's Rail name, at most 32 characters; A9 never renames it. Name the child after
    /// its task. Without one, A9 names it from its first prompt.
    #[serde(default)]
    #[ts(optional)]
    pub name: Option<String>,
    /// A36, M12: the child's Model, an id; without one the Project's for the Role applies. Any id
    /// `models.list` offers is accepted.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub model: Option<String>,
    /// A36: the child's Effort, an id from `models.list`. Only the user sets it.
    #[serde(default)]
    #[ts(optional = nullable)]
    #[schemars(skip)]
    pub effort: Option<String>,
    /// LC4: start a scout, which reads and searches only (`Read`, `Grep`, `Glob`, `Bash`) and has
    /// no skills, plugins or user instructions. Absent is false.
    #[serde(default)]
    #[ts(optional)]
    pub scout: Option<bool>,
    /// LC7: names of skills, plugins and MCP servers of the user's setup that this child needs
    /// beyond its Role's start. An unknown name fails the call.
    #[serde(default)]
    #[ts(optional)]
    pub extras: Option<Extras>,
}

/// LC7: names by kind. As a call's parameter, the extras to add; as the result of `agent.extras`,
/// the names that exist.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "agent/")]
pub struct Extras {
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub plugins: Vec<String>,
    #[serde(default)]
    pub mcp: Vec<String>,
}

/// LC8: add `extras` to a running child, which is relaunched with `--resume`.
#[derive(Clone, Debug, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "agent/")]
pub struct ExtrasAddParams {
    pub id: String,
    pub extras: Extras,
}

/// A36: `rail.startDoor`, with the Door's own Model and Effort when the caller sets them.
#[derive(Clone, Debug, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "agent/")]
pub struct StartDoorParams {
    pub id: String,
    #[serde(default)]
    #[ts(optional)]
    pub model: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub effort: Option<String>,
}

/// A37: change the Model and Effort of one Agent, running or not.
#[derive(Clone, Debug, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "agent/")]
pub struct SetModelParams {
    pub id: String,
    /// An id from `models.list`, or `None` for the Project's.
    pub model: Option<String>,
    pub effort: Option<String>,
    /// Must be `true`: the user has been told the Agent re-reads its conversation on the new Model.
    pub confirmed: bool,
}

/// X11: start `command` in a Run owned by Agent `id`; `cwd` defaults to its Worktree, else its cwd.
#[derive(Clone, Debug, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "agent/")]
pub struct RunParams {
    pub id: String,
    pub command: Vec<String>,
    #[serde(default)]
    #[ts(optional)]
    pub cwd: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "agent/")]
pub struct NodeId {
    pub id: String,
}

/// A Steer: `text` sent to the running Agent `id` as its next prompt (H11).
#[derive(Clone, Debug, Serialize, Deserialize, TS, JsonSchema)]
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

/// What `agent.signal` answers when it has something to say (otherwise `null`): `drop` tells the
/// hook to exit 2, which blocks a prompt that repeats Messages already delivered (B39).
#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct SignalOutcome {
    pub drop: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, JsonSchema)]
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
#[derive(Clone, Debug, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "agent/")]
pub struct MoveParams {
    pub id: String,
    pub parent: Option<String>,
    pub index: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, JsonSchema)]
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

/// B6: durable acceptance of an action that waits for a child's Takeover.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export, export_to = "agent/")]
pub struct HeldAction {
    pub id: String,
    pub agent: String,
    #[serde(flatten)]
    pub call: HeldCall,
    pub outcome: crate::terminal::ResizeOutcome,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "method", content = "params")]
#[ts(export, export_to = "agent/")]
pub enum HeldCall {
    #[serde(rename = "terminal.resize")]
    Resize(crate::terminal::ResizeParams),
    #[serde(rename = "terminal.close")]
    Close(crate::terminal::TerminalId),
    #[serde(rename = "terminal.kill")]
    Kill(crate::terminal::TerminalId),
    #[serde(rename = "terminal.write")]
    Write(crate::terminal::WriteParams),
    #[serde(rename = "rail.rename")]
    Rename(RenameParams),
    #[serde(rename = "agent.setOrder")]
    SetOrder(SetOrderParams),
    #[serde(rename = "decision.answer")]
    Answer(crate::decision::AnswerParams),
    #[serde(rename = "agent.resume")]
    Resume(NodeId),
    #[serde(rename = "agent.stop")]
    Stop(NodeId),
    #[serde(rename = "agent.interrupt")]
    Interrupt(NodeId),
    #[serde(rename = "agent.prompt")]
    Prompt(PromptParams),
    #[serde(rename = "workstream.rewind")]
    Rewind(crate::workstream::RewindParams),
}
