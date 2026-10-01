//! Agents, the Rail tree and the Claude Code adapter. Owner: agents Builder.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use contracts::Status;
use rpc::{Bus, Ctx, Module, OpenError, RpcError};
use serde_json::Value;

pub mod claude_code;

/// One input an adapter reads about its Agent.
pub enum Observation {
    /// A structured event the Agent's own tooling pushed; for Claude Code, a hook payload.
    Signal(Value),
    /// The Agent's program ended. `None` when it was killed by a signal.
    Exit { code: Option<i32> },
}

/// Turns one vendor's program into an Agent's Status. It emits `error`, `needs-you`, `working`,
/// `idle` and `done`, never `blocked`: that Kind is the Daemon's, from Todos and Routes.
pub trait AgentAdapter {
    /// Fold one Observation into the Status. `None` means no change.
    fn observe(&mut self, observation: Observation) -> Option<Status>;
}

pub struct Agents;

impl Agents {
    /// `dir` is the Project's `.roundup/` directory. `bus` is for events no call caused.
    pub fn open(
        _dir: &Path,
        _bus: Bus,
        _terminals: Arc<terminal::Terminals>,
    ) -> Result<Self, OpenError> {
        Ok(Self)
    }
}

#[async_trait]
impl Module for Agents {
    fn namespaces(&self) -> &'static [&'static str] {
        &["agent", "rail"]
    }

    async fn call(&self, _ctx: &Ctx, method: &str, _params: Value) -> Result<Value, RpcError> {
        Err(RpcError::method_not_found(method))
    }
}
