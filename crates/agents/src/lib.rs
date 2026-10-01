//! Agents, the Rail tree and the Claude Code adapter. Owner: agents Builder.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use contracts::agent::{CreateGroupParams, NodeKind};
use contracts::{EventData, Status};
use rpc::{Bus, Ctx, Module, OpenError, RpcError, params, reply};
use serde_json::Value;

pub mod claude_code;
mod rail;

/// One input an adapter reads about its Agent.
pub enum Observation {
    /// A structured event the Agent's own tooling pushed; for Claude Code, a hook payload.
    Signal(Value),
    /// The window title the Agent's program set.
    Title(String),
    /// The Agent's program ended. `None` when it was killed by a signal.
    Exit { code: Option<i32> },
}

/// Turns one vendor's program into an Agent's Status. It emits `error`, `needs-you`, `working`,
/// `idle` and `done`, never `blocked`: that Kind is the Daemon's, from Todos and Routes.
pub trait AgentAdapter {
    /// Fold one Observation into the Status. `None` means no change.
    fn observe(&mut self, observation: Observation) -> Option<Status>;
}

pub struct Agents {
    rail: Mutex<rail::Rail>,
}

impl Agents {
    /// `dir` is the Project's `.roundup/` directory. `bus` is for events no call caused.
    pub fn open(
        dir: &Path,
        _bus: Bus,
        _terminals: Arc<terminal::Terminals>,
    ) -> Result<Self, OpenError> {
        let rail = rail::Rail::open(&dir.join("agents.db"), now_ms())?;
        Ok(Self {
            rail: Mutex::new(rail),
        })
    }

    fn rail(&self) -> std::sync::MutexGuard<'_, rail::Rail> {
        self.rail.lock().expect("rail lock")
    }
}

/// Milliseconds since the Unix epoch.
fn now_ms() -> i64 {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(since_epoch.as_millis()).unwrap_or(i64::MAX)
}

#[async_trait]
impl Module for Agents {
    fn namespaces(&self) -> &'static [&'static str] {
        &["agent", "rail"]
    }

    async fn call(&self, ctx: &Ctx, method: &str, value: Value) -> Result<Value, RpcError> {
        match method {
            "rail.tree" => reply(&self.rail().tree()?),
            "rail.createGroup" => {
                let CreateGroupParams { name, parent } = params(value)?;
                let node = self
                    .rail()
                    .insert(NodeKind::Group, &name, parent.as_deref(), None)?;
                ctx.emit(EventData::RailChanged);
                reply(&node)
            }
            _ => Err(RpcError::method_not_found(method)),
        }
    }
}
