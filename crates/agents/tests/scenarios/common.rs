//! Helpers shared by the scenario tests.

use std::path::Path;
use std::sync::Arc;

use agents::Agents;
use contracts::agent::RailNode;
use contracts::{Actor, Event, EventData};
use provenance::Touches;
use rpc::{Bus, Ctx, Module, RpcError};
use serde_json::{Value, json};
use terminal::Terminals;
use tokio::sync::broadcast::Receiver;

pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub bus: Bus,
    pub events: Receiver<Event>,
    pub agents: Agents,
}

pub fn ctx(bus: &Bus) -> Ctx {
    Ctx {
        actor: Actor::user(),
        bus: bus.clone(),
        touches: Arc::new(Touches::in_memory().unwrap()),
    }
}

pub fn open_in(dir: &Path, bus: &Bus) -> Agents {
    let terminals = Arc::new(Terminals::open(dir, bus.clone()).unwrap());
    Agents::open(dir, bus.clone(), terminals).unwrap()
}

impl Fixture {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let bus = Bus::new();
        let events = bus.subscribe();
        let agents = open_in(dir.path(), &bus);
        Self {
            dir,
            bus,
            events,
            agents,
        }
    }

    /// The same directory, opened again as after a Daemon restart (new Terminals, none running).
    pub fn reopen(self) -> Self {
        let Self {
            dir, bus, agents, ..
        } = self;
        drop(agents);
        let events = bus.subscribe();
        let agents = open_in(dir.path(), &bus);
        Self {
            dir,
            bus,
            events,
            agents,
        }
    }

    pub async fn call(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        self.agents.call(&ctx(&self.bus), method, params).await
    }

    pub async fn group(&self, name: &str, parent: Option<&str>) -> String {
        let node = self
            .call("rail.createGroup", json!({"name": name, "parent": parent}))
            .await
            .unwrap();
        node["id"].as_str().unwrap().to_owned()
    }

    pub async fn tree(&self) -> Vec<RailNode> {
        let tree = self.call("rail.tree", Value::Null).await.unwrap();
        serde_json::from_value(tree).unwrap()
    }

    /// How many `rail.changed` events arrived since the last call.
    pub fn changed(&mut self) -> usize {
        std::iter::from_fn(|| self.events.try_recv().ok())
            .filter(|event| matches!(event.data, EventData::RailChanged))
            .count()
    }
}
