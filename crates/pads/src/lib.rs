//! Pads: markdown notes, app-stored or as files. Owner: pads Builder.

use std::path::Path;

use async_trait::async_trait;
use rpc::{Bus, Ctx, Module, OpenError, RpcError};
use serde_json::Value;

pub struct Pads;

impl Pads {
    /// `dir` is the Project's `.roundup/` directory. `bus` is for events no call caused.
    pub fn open(_dir: &Path, _bus: Bus) -> Result<Self, OpenError> {
        Ok(Self)
    }
}

#[async_trait]
impl Module for Pads {
    fn namespaces(&self) -> &'static [&'static str] {
        &["pad"]
    }

    async fn call(&self, _ctx: &Ctx, method: &str, _params: Value) -> Result<Value, RpcError> {
        Err(RpcError::method_not_found(method))
    }
}
