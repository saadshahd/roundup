//! B19 to B21: the Daemon reads the Rail, the Todos, the Pads and the Messages together, which no
//! module does alone, to answer `agent.digest` and to push a child's change to its Meta-agent.

use std::collections::HashMap;
use std::sync::Arc;

use contracts::message::{Message, MessageKind};
use contracts::{Actor, EventData};
use rpc::{Bus, Ctx, Module, RpcError};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;

/// The modules a digest is composed from, read as the Daemon or the user, so reading changes
/// nothing and logs no Touch.
#[derive(Clone)]
pub struct Reader {
    modules: HashMap<&'static str, Arc<dyn Module>>,
    bus: Bus,
    touches: Arc<provenance::Touches>,
}

impl Reader {
    pub fn new(
        modules: &HashMap<&'static str, Arc<dyn Module>>,
        bus: Bus,
        touches: Arc<provenance::Touches>,
    ) -> Self {
        let modules = ["rail", "todo", "pad", "message"]
            .iter()
            .filter_map(|name| modules.get(name).map(|module| (*name, Arc::clone(module))))
            .collect();
        Self {
            modules,
            bus,
            touches,
        }
    }

    pub async fn call<T: DeserializeOwned>(
        &self,
        namespace: &'static str,
        method: &'static str,
        params: Value,
        actor: Actor,
    ) -> Result<T, RpcError> {
        let module = self
            .modules
            .get(namespace)
            .ok_or_else(|| RpcError::internal(format!("{namespace} module is not registered")))?;
        let ctx = Ctx {
            actor,
            bus: self.bus.clone(),
            touches: Arc::clone(&self.touches),
        };
        let value = module.call(&ctx, method, params).await?;
        serde_json::from_value(value).map_err(RpcError::internal)
    }

    /// The Rail, the Todos, the Pads and every Message.
    pub async fn world(&self) -> Result<World, RpcError> {
        let daemon = Actor::daemon;
        Ok(World {
            nodes: self
                .call("rail", "rail.tree", Value::Null, daemon())
                .await?,
            todos: self
                .call("todo", "todo.list", Value::Null, daemon())
                .await?,
            pads: self.call("pad", "pad.list", Value::Null, daemon()).await?,
            // The user sees every Message; `message.list` shows the Daemon only its own.
            messages: self
                .call("message", "message.list", json!({}), Actor::user())
                .await?,
        })
    }
}

pub struct World {
    nodes: Vec<contracts::agent::RailNode>,
    todos: Vec<contracts::todo::Todo>,
    pads: Vec<contracts::pad::Pad>,
    messages: Vec<Message>,
}

impl World {
    pub fn sources(&self) -> messages::digest::Sources<'_> {
        messages::digest::Sources {
            nodes: &self.nodes,
            todos: &self.todos,
            pads: &self.pads,
            messages: &self.messages,
        }
    }
}

/// B21: whether `data` can change an entry of a digest.
fn moves_an_entry(data: &EventData) -> bool {
    matches!(
        data,
        EventData::AgentStatus(_)
            | EventData::TodoCreated(_)
            | EventData::TodoUpdated(_)
            | EventData::TodoDeleted(_)
            | EventData::PadChanged(_)
            | EventData::RailChanged
    )
}

/// B21: on each change that may alter a child's entry, send its Meta-agent one Message from the
/// Daemon, kind `note`, with that entry. Runs until the Bus closes.
pub async fn push_changes(
    reader: Reader,
    seen: Arc<messages::digest::Seen>,
    mut events: tokio::sync::broadcast::Receiver<contracts::Event>,
) {
    // What is already there is not news: record it as told before the first event is judged.
    if let Ok(world) = reader.world().await {
        seen.baseline(&world.sources());
    }
    loop {
        match events.recv().await {
            Ok(event) if moves_an_entry(&event.data) => {}
            Ok(_) => continue,
            // Events were missed: the next comparison with what was told finds their effect.
            Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => return,
        }
        if let Err(err) = push_once(&reader, &seen).await {
            eprintln!("rupd: digest push failed: {err}");
        }
    }
}

async fn push_once(reader: &Reader, seen: &messages::digest::Seen) -> Result<(), RpcError> {
    let world = reader.world().await?;
    for push in seen.changed(&world.sources()) {
        let sent: Result<Message, RpcError> = reader
            .call(
                "message",
                "message.send",
                json!({
                    "to": push.meta,
                    "kind": MessageKind::Note,
                    "body": messages::digest::push_body(&push.entry),
                }),
                Actor::daemon(),
            )
            .await;
        match sent {
            Ok(_) => {}
            // A Meta-agent that is gone gets none (B9); anything else is tried again at the next
            // change.
            Err(err) => {
                eprintln!("rupd: digest push to {} refused: {err}", push.meta);
                seen.release(push);
            }
        }
    }
    Ok(())
}
