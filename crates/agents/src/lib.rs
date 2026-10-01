//! Agents, the Rail tree and the Claude Code adapter. Owner: agents Builder.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use contracts::agent::{
    CreateGroupParams, MoveParams, NodeKind, RailNode, RenameParams, SpawnParams, StatusEvent,
};
use contracts::terminal::SpawnParams as TerminalSpawn;
use contracts::{Actor, EventData, Kind, Status};
use rpc::{Bus, Ctx, Module, OpenError, RpcError, params, reply};
use serde_json::Value;
use terminal::Terminals;
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::error::RecvError;

pub mod claude_code;
mod rail;

use claude_code::{ClaudeCode, Launcher};

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

/// State the Terminal watchers and the RPC calls share.
struct Shared {
    rail: Mutex<rail::Rail>,
    runs: Mutex<HashMap<String, ClaudeCode>>,
    bus: Bus,
    terminals: Arc<Terminals>,
    /// Since when an Agent without a Terminal has been `done`.
    opened: i64,
}

impl Shared {
    fn rail(&self) -> MutexGuard<'_, rail::Rail> {
        self.rail.lock().expect("rail lock")
    }

    fn runs(&self) -> MutexGuard<'_, HashMap<String, ClaudeCode>> {
        self.runs.lock().expect("runs lock")
    }

    /// An Agent, or a Meta-agent, has a Status: its live one, else `done` because no Terminal
    /// outlives the Daemon. Every node that leaves the module passes through here.
    fn present(&self, mut node: RailNode) -> RailNode {
        if node.kind == NodeKind::Agent || node.meta {
            let live = self
                .runs()
                .get(&node.id)
                .and_then(|run| run.status().cloned());
            node.status = Some(live.unwrap_or_else(|| Status {
                kind: Kind::Done,
                label: "terminal gone".into(),
                since: self.opened,
            }));
        }
        node
    }

    /// Fold `observation` into Agent `id`'s Status and announce it if it changed.
    fn observe(&self, actor: Actor, id: &str, observation: Observation) {
        let changed = self
            .runs()
            .get_mut(id)
            .and_then(|run| run.observe(observation));
        if let Some(status) = changed {
            let event = StatusEvent {
                id: id.to_owned(),
                status,
            };
            self.bus.emit(actor, EventData::AgentStatus(event));
        }
    }
}

/// Feed one Terminal's title changes and its exit to the Agent behind it.
async fn watch(
    shared: Arc<Shared>,
    id: String,
    terminal_id: String,
    mut events: Receiver<EventData>,
) {
    let daemon = Actor::daemon;
    loop {
        match events.recv().await {
            Ok(EventData::TerminalTitle(title)) => {
                shared.observe(daemon(), &id, Observation::Title(title.title));
            }
            Ok(EventData::TerminalExited(exited)) => {
                shared.observe(daemon(), &id, Observation::Exit { code: exited.code });
                return;
            }
            Ok(_) => {}
            // Output can outrun this task and drop the exit event with it; the table still knows.
            Err(RecvError::Lagged(_)) => {
                let gone = shared
                    .terminals
                    .list()
                    .into_iter()
                    .find(|t| t.id == terminal_id && !t.running);
                if let Some(gone) = gone {
                    shared.observe(
                        daemon(),
                        &id,
                        Observation::Exit {
                            code: gone.exit_code,
                        },
                    );
                    return;
                }
            }
            Err(RecvError::Closed) => return,
        }
    }
}

pub struct Agents {
    shared: Arc<Shared>,
    dir: PathBuf,
    launcher: Launcher,
}

impl Agents {
    /// `dir` is the Project's `.roundup/` directory. `bus` is for events no call caused.
    /// Claude Code is `ROUNDUP_CLAUDE_BIN` (default `claude`).
    pub fn open(dir: &Path, bus: Bus, terminals: Arc<Terminals>) -> Result<Self, OpenError> {
        Self::open_with(dir, bus, terminals, Launcher::from_env()?)
    }

    pub fn open_with(
        dir: &Path,
        bus: Bus,
        terminals: Arc<Terminals>,
        launcher: Launcher,
    ) -> Result<Self, OpenError> {
        Ok(Self {
            shared: Arc::new(Shared {
                rail: Mutex::new(rail::Rail::open(&dir.join("agents.db"))?),
                runs: Mutex::new(HashMap::new()),
                bus,
                terminals,
                opened: now_ms(),
            }),
            dir: dir.to_owned(),
            launcher,
        })
    }

    /// Put a new Agent in the Rail and start Claude Code for it in a Terminal.
    async fn spawn(&self, ctx: &Ctx, params: SpawnParams) -> Result<RailNode, RpcError> {
        let cwd = Path::new(&params.cwd);
        let name = cwd
            .file_name()
            .map_or_else(|| "claude".into(), |n| n.to_string_lossy().into_owned());
        let node =
            self.shared
                .rail()
                .insert(NodeKind::Agent, &name, params.parent.as_deref(), None)?;
        let spawned = match self.start(&node.id, cwd).await {
            Ok(spawned) => spawned,
            Err(err) => {
                self.shared.rail().remove(&node.id)?;
                return Err(err);
            }
        };
        self.shared
            .runs()
            .insert(node.id.clone(), ClaudeCode::starting(now_ms));
        self.shared.rail().attach_terminal(&node.id, &spawned.id)?;
        tokio::spawn(watch(
            Arc::clone(&self.shared),
            node.id.clone(),
            spawned.id.clone(),
            spawned.events,
        ));
        ctx.emit(EventData::RailChanged);
        Ok(self.shared.present(RailNode {
            terminal_id: Some(spawned.id),
            ..node
        }))
    }

    async fn start(&self, id: &str, cwd: &Path) -> Result<terminal::Spawned, RpcError> {
        let argv = self.launcher.prepare(&self.dir, id, cwd)?;
        self.shared
            .terminals
            .spawn(TerminalSpawn {
                cwd: cwd.to_string_lossy().into_owned(),
                command: Some(argv),
                env: BTreeMap::new(),
                cols: 80,
                rows: 24,
            })
            .await
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
        let shared = &self.shared;
        match method {
            "agent.spawn" => reply(&self.spawn(ctx, params(value)?).await?),
            "rail.tree" => {
                let nodes = shared.rail().tree()?;
                reply(
                    &nodes
                        .into_iter()
                        .map(|node| shared.present(node))
                        .collect::<Vec<_>>(),
                )
            }
            "rail.createGroup" => {
                let CreateGroupParams { name, parent } = params(value)?;
                let node = shared
                    .rail()
                    .insert(NodeKind::Group, &name, parent.as_deref(), None)?;
                ctx.emit(EventData::RailChanged);
                reply(&shared.present(node))
            }
            "rail.rename" => {
                let RenameParams { id, name } = params(value)?;
                let node = shared.rail().rename(&id, &name)?;
                ctx.emit(EventData::RailChanged);
                reply(&shared.present(node))
            }
            "rail.move" => {
                let MoveParams { id, parent, index } = params(value)?;
                let node = shared.rail().move_node(&id, parent.as_deref(), index)?;
                ctx.emit(EventData::RailChanged);
                reply(&shared.present(node))
            }
            _ => Err(RpcError::method_not_found(method)),
        }
    }
}
