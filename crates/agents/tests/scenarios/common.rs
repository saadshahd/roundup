//! Helpers shared by the scenario tests.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use agents::Agents;
use agents::claude_code::Launcher;
use contracts::agent::RailNode;
use contracts::{Actor, Event, EventData};
use provenance::Touches;
use rpc::{Bus, Ctx, Module, RpcError};
use serde_json::{Value, json};
use terminal::Terminals;
use tokio::sync::broadcast::Receiver;

const PATIENCE: Duration = Duration::from_secs(10);

pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub bus: Bus,
    pub events: Receiver<Event>,
    pub agents: Agents,
}

/// An executable `sh` script standing in for `claude`.
pub fn fake_claude(dir: &Path, body: &str) -> String {
    let path = dir.join("fake-claude");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.to_string_lossy().into_owned()
}

/// Agents over a fresh Terminals, launching `bin` and keeping Claude's config inside `dir`.
pub fn open_in(dir: &Path, bus: &Bus, bin: &str) -> Agents {
    let terminals = Arc::new(Terminals::open(dir, bus.clone()).unwrap());
    let rup = dir.join("rup");
    std::fs::write(&rup, "").unwrap();
    let launcher = Launcher::new(bin, dir.join("claude.json"), rup);
    Agents::open_with(dir, bus.clone(), terminals, launcher).unwrap()
}

impl Fixture {
    pub fn new() -> Self {
        Self::running(":")
    }

    /// A Fixture whose Agents run `script` as their `claude`.
    pub fn running(script: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let bin = fake_claude(dir.path(), script);
        Self::over(dir, Bus::new(), &bin)
    }

    fn over(dir: tempfile::TempDir, bus: Bus, bin: &str) -> Self {
        let events = bus.subscribe();
        let agents = open_in(dir.path(), &bus, bin);
        Self {
            dir,
            bus,
            events,
            agents,
        }
    }

    pub fn reopen(self) -> Self {
        let Self {
            dir, bus, agents, ..
        } = self;
        drop(agents);
        let bin = dir
            .path()
            .join("fake-claude")
            .to_string_lossy()
            .into_owned();
        Self::over(dir, bus, &bin)
    }

    pub async fn call(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let ctx = Ctx {
            actor: Actor::user(),
            bus: self.bus.clone(),
            touches: Arc::new(Touches::in_memory().unwrap()),
        };
        self.agents.call(&ctx, method, params).await
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

    /// The tree, once `done` holds for it.
    pub async fn until(&self, done: impl Fn(&[RailNode]) -> bool) -> Vec<RailNode> {
        tokio::time::timeout(PATIENCE, async {
            loop {
                let tree = self.tree().await;
                if done(&tree) {
                    return tree;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("the tree reaches the expected state in time")
    }
}
