//! Agents, the Rail tree and the Claude Code adapter. Owner: agents Builder.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use contracts::agent::{
    CreateGroupParams, MoveParams, NodeId, NodeKind, RailNode, RenameParams, SignalParams,
    SpawnParams, StatusEvent,
};
use contracts::terminal::SpawnParams as TerminalSpawn;
use contracts::{Actor, EventData, Kind, Status};
use rpc::{Bus, Ctx, Module, OpenError, RpcError, code, params, reply};
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
    /// Roundup stopped the Agent's program on purpose.
    Stopped,
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
    runs: Mutex<HashMap<String, Run>>,
    bus: Bus,
    terminals: Arc<Terminals>,
    /// Since when an Agent without a Terminal has been `done`.
    opened: i64,
}

impl Shared {
    fn rail(&self) -> MutexGuard<'_, rail::Rail> {
        self.rail.lock().expect("rail lock")
    }

    fn runs(&self) -> MutexGuard<'_, HashMap<String, Run>> {
        self.runs.lock().expect("runs lock")
    }

    /// An Agent, or a Meta-agent, has a Status: its live one, else `done` because no Terminal
    /// outlives the Daemon. Every node that leaves the module passes through here.
    fn present(&self, mut node: RailNode) -> RailNode {
        if node.kind == NodeKind::Agent || node.meta {
            let live = self
                .runs()
                .get(&node.id)
                .and_then(|run| run.adapter.status().cloned());
            node.status = Some(live.unwrap_or_else(|| Status {
                kind: Kind::Done,
                label: "terminal gone".into(),
                since: self.opened,
            }));
        }
        node
    }

    /// Fold `observation` into Agent `id`'s Status. A change is announced as `agent.status`, and
    /// the first idle types the prompt the Agent was spawned with.
    fn observe(&self, actor: Actor, id: &str, observation: Observation) -> Result<(), RpcError> {
        let (changed, prompt) = {
            let mut runs = self.runs();
            let run = runs
                .get_mut(id)
                .ok_or_else(|| RpcError::not_found(format!("agent {id}")))?;
            let changed = run.adapter.observe(observation);
            let idle = changed
                .as_ref()
                .is_some_and(|status| status.kind == Kind::Idle);
            let prompt = run.prompt.take_if(|_| idle);
            (
                changed,
                prompt.map(|prompt| (run.terminal_id.clone(), prompt)),
            )
        };
        if let Some(status) = changed {
            let event = StatusEvent {
                id: id.to_owned(),
                status,
            };
            self.bus.emit(actor, EventData::AgentStatus(event));
        }
        if let Some((terminal_id, prompt)) = prompt {
            tokio::spawn(type_prompt(
                Arc::clone(&self.terminals),
                terminal_id,
                prompt,
            ));
        }
        Ok(())
    }
}

/// An Agent's program and what is left to tell it.
struct Run {
    adapter: ClaudeCode,
    /// Typed into the Terminal at the first idle, then gone.
    prompt: Option<String>,
    terminal_id: String,
}

/// Claude Code reads a prompt typed and submitted in one burst as a paste, so Enter follows after
/// a pause, as in the spike (`spikes/hooks-state/drive.py`).
const SUBMIT_DELAY: Duration = Duration::from_secs(1);

async fn type_prompt(terminals: Arc<Terminals>, terminal_id: String, prompt: String) {
    let typed = async {
        terminals.write(&terminal_id, prompt.as_bytes()).await?;
        tokio::time::sleep(SUBMIT_DELAY).await;
        terminals.write(&terminal_id, b"\r").await
    };
    if let Err(err) = typed.await {
        eprintln!("agents: could not type the prompt into terminal {terminal_id}: {err}");
    }
}

/// `spawn` registers an Agent before its watcher starts, so the watcher always finds it.
const REGISTERED: &str = "a watched Agent is registered";

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
                shared
                    .observe(daemon(), &id, Observation::Title(title.title))
                    .expect(REGISTERED);
            }
            Ok(EventData::TerminalExited(exited)) => {
                shared
                    .observe(daemon(), &id, Observation::Exit { code: exited.code })
                    .expect(REGISTERED);
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
                    let exit = Observation::Exit {
                        code: gone.exit_code,
                    };
                    shared.observe(daemon(), &id, exit).expect(REGISTERED);
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
        let terminal_id = match self.run_agent(&node.id, cwd, params.prompt).await {
            Ok(terminal_id) => terminal_id,
            Err(err) => {
                self.shared.rail().remove(&node.id)?;
                return Err(err);
            }
        };
        self.shared.rail().attach_terminal(&node.id, &terminal_id)?;
        ctx.emit(EventData::RailChanged);
        Ok(self.shared.present(RailNode {
            terminal_id: Some(terminal_id),
            ..node
        }))
    }

    /// Make a Group a Meta-agent: start a live Agent that sits at it, in the Project's folder.
    async fn promote(&self, ctx: &Ctx, id: &str) -> Result<RailNode, RpcError> {
        let group = self.shared.rail().node(id)?;
        if group.kind != NodeKind::Group || group.meta {
            return Err(RpcError::conflict(format!("{id} is not a plain Group")));
        }
        let project = self.dir.parent().unwrap_or(&self.dir);
        let terminal_id = self.run_agent(id, project, None).await?;
        let node = self.shared.rail().promote(id, &terminal_id)?;
        ctx.emit(EventData::RailChanged);
        Ok(self.shared.present(node))
    }

    /// Stop an Agent's program; it stays in the Rail as `done`. A Meta-agent's children move up
    /// to where it was and keep running.
    async fn stop(&self, ctx: &Ctx, id: &str) -> Result<(), RpcError> {
        let node = self.shared.rail().node(id)?;
        if node.kind == NodeKind::Terminal || (node.kind == NodeKind::Group && !node.meta) {
            return Err(RpcError::conflict(format!("{id} is not an Agent")));
        }
        let terminal_id = self
            .shared
            .runs()
            .get(id)
            .map(|run| run.terminal_id.clone());
        if let Some(terminal_id) = terminal_id {
            self.shared
                .observe(ctx.actor.clone(), id, Observation::Stopped)?;
            match self.shared.terminals.kill(&terminal_id).await {
                // It exited on its own first.
                Err(err) if err.code != code::NOT_FOUND => return Err(err),
                _ => {}
            }
        }
        if node.kind == NodeKind::Group {
            self.shared.rail().lift_children(id)?;
            ctx.emit(EventData::RailChanged);
        }
        Ok(())
    }

    /// Start Claude Code for node `id` in a Terminal and watch it; returns the Terminal's id.
    async fn run_agent(
        &self,
        id: &str,
        cwd: &Path,
        prompt: Option<String>,
    ) -> Result<String, RpcError> {
        let argv = self.launcher.prepare(&self.dir, id, cwd)?;
        let spawned = self
            .shared
            .terminals
            .spawn(TerminalSpawn {
                cwd: cwd.to_string_lossy().into_owned(),
                command: Some(argv),
                env: BTreeMap::new(),
                cols: 80,
                rows: 24,
            })
            .await?;
        let run = Run {
            adapter: ClaudeCode::starting(now_ms),
            prompt,
            terminal_id: spawned.id.clone(),
        };
        self.shared.runs().insert(id.to_owned(), run);
        tokio::spawn(watch(
            Arc::clone(&self.shared),
            id.to_owned(),
            spawned.id.clone(),
            spawned.events,
        ));
        Ok(spawned.id)
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
            "agent.stop" => {
                let NodeId { id } = params(value)?;
                self.stop(ctx, &id).await?;
                reply(&())
            }
            "rail.promote" => {
                let NodeId { id } = params(value)?;
                reply(&self.promote(ctx, &id).await?)
            }
            "agent.signal" => {
                let SignalParams { id, payload } = params(value)?;
                shared.observe(ctx.actor.clone(), &id, Observation::Signal(payload))?;
                reply(&())
            }
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
