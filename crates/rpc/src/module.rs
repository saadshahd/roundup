use std::sync::Arc;

use async_trait::async_trait;
use contracts::{Actor, Event, EventData, Verb};
use provenance::Touches;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::broadcast;

use crate::error::RpcError;

/// The typed event bus. Cheap to clone; every clone sends to the same subscribers.
#[derive(Clone)]
pub struct Bus {
    sender: broadcast::Sender<Event>,
}

impl Default for Bus {
    fn default() -> Self {
        Self::new()
    }
}

impl Bus {
    pub fn new() -> Self {
        Self {
            sender: broadcast::channel(1024).0,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }

    /// Publish an event. Having no subscribers is normal, not an error.
    pub fn emit(&self, actor: Actor, data: EventData) {
        let _ = self.sender.send(Event { actor, data });
    }
}

/// What one call knows: who made it, where to publish events, where to log Touches.
pub struct Ctx {
    pub actor: Actor,
    pub bus: Bus,
    pub touches: Arc<Touches>,
}

impl Ctx {
    /// Log a read or write of `item` (`todo:<id>` or `pad:<name>`) by the caller.
    pub fn touch(&self, verb: Verb, item: &str) -> Result<(), RpcError> {
        self.touches
            .record(&self.actor, verb, item)
            .map_err(RpcError::internal)
    }

    /// Publish an event caused by this call.
    pub fn emit(&self, data: EventData) {
        self.bus.emit(self.actor.clone(), data);
    }
}

/// One module of the Daemon. Owns one or more method namespaces (the part before the first dot).
#[async_trait]
pub trait Module: Send + Sync {
    fn namespaces(&self) -> &'static [&'static str];

    /// `method` is the full name, for example `todo.create`.
    async fn call(&self, ctx: &Ctx, method: &str, params: Value) -> Result<Value, RpcError>;
}

/// Decode request parameters; a bad shape is the caller's error.
pub fn params<T: DeserializeOwned>(value: Value) -> Result<T, RpcError> {
    serde_json::from_value(value).map_err(|err| {
        RpcError::new(
            crate::error::code::INVALID_PARAMS,
            format!("invalid params: {err}"),
        )
    })
}

/// Encode a result.
pub fn reply<T: Serialize>(value: &T) -> Result<Value, RpcError> {
    serde_json::to_value(value).map_err(RpcError::internal)
}
