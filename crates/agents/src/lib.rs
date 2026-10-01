//! Agents, the Rail tree and the Claude Code adapter. Owner: agents Builder.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use rpc::{Bus, Ctx, Module, OpenError, RpcError};
use serde_json::Value;

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
