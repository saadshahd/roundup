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
use tokio::time::{Instant, sleep_until};

pub mod claude_code;
mod rail;

use claude_code::{ClaudeCode, Launcher};

/// One input an adapter reads about its Agent.
pub enum Observation {
    /// A structured event the Agent's own tooling pushed; for Claude Code, a hook payload.
    Signal(Value),
    /// The window title the Agent's program set.
    Title(String),
    /// The clock reached the time the adapter asked for (see `ClaudeCode::tick_at`).
    Tick,
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
    /// `None` while the Agent's Terminal is starting. A node is marked, and unmarked on failure,
    /// in the same Rail critical section that creates or reserves it, so `agent.stop` never finds
    /// a starting node unmarked.
    runs: Mutex<HashMap<String, Option<Run>>>,
    bus: Bus,
    terminals: Arc<Terminals>,
    /// Milliseconds since the Unix epoch. Adapters stamp Statuses with it and the watchers time
    /// Ticks by it, so the two never disagree about when a hold ends.
    clock: Clock,
    /// Since when an Agent without a Terminal has been `done`.
    opened: i64,
}

type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;

impl Shared {
    fn rail(&self) -> MutexGuard<'_, rail::Rail> {
        self.rail.lock().expect("rail lock")
    }

    fn runs(&self) -> MutexGuard<'_, HashMap<String, Option<Run>>> {
        self.runs.lock().expect("runs lock")
    }

    /// An Agent, or a Meta-agent, has a Status: its live one, else `done` because no Terminal
    /// outlives the Daemon. Every node that leaves the module passes through here.
    fn present(&self, mut node: RailNode) -> RailNode {
        if node.kind == NodeKind::Agent || node.meta {
            let live = self
                .runs()
                .get(&node.id)
                .and_then(Option::as_ref)
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
    /// the first idle types the prompt the Agent was spawned with. Returns when, on `clock`, the
    /// adapter wants its next `Observation::Tick`.
    fn observe(
        &self,
        actor: Actor,
        id: &str,
        observation: Observation,
    ) -> Result<Option<i64>, RpcError> {
        let (prompt, tick_at) = {
            let mut runs = self.runs();
            let run = runs
                .get_mut(id)
                .and_then(Option::as_mut)
                .ok_or_else(|| RpcError::not_found(format!("agent {id}")))?;
            let changed = run.adapter.observe(observation);
            let idle = changed
                .as_ref()
                .is_some_and(|status| status.kind == Kind::Idle);
            // Announced while `runs` is held, so announcements leave in the order the Status
            // changed; `emit` never blocks.
            if let Some(status) = changed {
                let event = StatusEvent {
                    id: id.to_owned(),
                    status,
                };
                self.bus.emit(actor, EventData::AgentStatus(event));
            }
            let prompt = run.prompt.take_if(|_| idle);
            (
                prompt.map(|prompt| (run.terminal_id.clone(), prompt)),
                run.adapter.tick_at(),
            )
        };
        if let Some((terminal_id, prompt)) = prompt {
            tokio::spawn(type_prompt(
                Arc::clone(&self.terminals),
                terminal_id,
                prompt,
            ));
        }
        Ok(tick_at)
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

/// Feed one Terminal's titles and its exit to the Agent behind it, and a Tick at the time its
/// adapter asks for one. Only a held star asks, so the watcher of an idle Agent never wakes.
async fn watch(shared: Arc<Shared>, id: String, mut events: Receiver<EventData>) {
    let mut tick: Option<Instant> = None;
    loop {
        let observation = tokio::select! {
            event = events.recv() => match event {
                Ok(EventData::TerminalTitle(title)) => Observation::Title(title.title),
                Ok(EventData::TerminalExited(exited)) => Observation::Exit { code: exited.code },
                // A lag loses the oldest events, never the exit: it is the last one sent.
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return,
            },
            () = sleep_until(tick.unwrap_or_else(Instant::now)), if tick.is_some() => Observation::Tick,
        };
        let exited = matches!(observation, Observation::Exit { .. });
        let tick_at = shared
            .observe(Actor::daemon(), &id, observation)
            .expect(REGISTERED);
        if exited {
            return;
        }
        tick = tick_at.map(|at| {
            let wait = u64::try_from(at - (shared.clock)()).unwrap_or(0);
            Instant::now() + Duration::from_millis(wait)
        });
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
                clock: Arc::new(now_ms),
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
        let node = {
            let mut rail = self.shared.rail();
            let node = rail.insert(NodeKind::Agent, &name, params.parent.as_deref(), None)?;
            self.shared.runs().insert(node.id.clone(), None);
            node
        };
        let terminal_id = match self.run_agent(&node.id, cwd, params.prompt).await {
            Ok(terminal_id) => terminal_id,
            Err(err) => {
                let mut rail = self.shared.rail();
                self.shared.runs().remove(&node.id);
                report_undo("remove the Agent from the Rail", rail.remove(&node.id));
                return Err(err);
            }
        };
        ctx.emit(EventData::RailChanged);
        Ok(self.shared.present(RailNode {
            terminal_id: Some(terminal_id),
            ..node
        }))
    }

    /// Make a Group a Meta-agent: start a live Agent that sits at it, in the Project's folder.
    async fn promote(&self, ctx: &Ctx, id: &str) -> Result<RailNode, RpcError> {
        {
            let mut rail = self.shared.rail();
            rail.reserve_meta(id)?;
            self.shared.runs().insert(id.to_owned(), None);
        }
        let project = self.dir.parent().unwrap_or(&self.dir);
        if let Err(err) = self.run_agent(id, project, None).await {
            let mut rail = self.shared.rail();
            self.shared.runs().remove(id);
            report_undo("give the Group back its plain state", rail.release_meta(id));
            return Err(err);
        }
        ctx.emit(EventData::RailChanged);
        Ok(self.shared.present(self.shared.rail().node(id)?))
    }

    /// Stop an Agent's program; it stays in the Rail as `done`. A Meta-agent's children move up
    /// to where it was and keep running.
    async fn stop(&self, ctx: &Ctx, id: &str) -> Result<(), RpcError> {
        let (node, run) = {
            let rail = self.shared.rail();
            let node = rail.node(id)?;
            let run = self
                .shared
                .runs()
                .get(id)
                .map(|run| run.as_ref().map(|run| run.terminal_id.clone()));
            (node, run)
        };
        if node.kind == NodeKind::Terminal || (node.kind == NodeKind::Group && !node.meta) {
            return Err(RpcError::conflict(format!("{id} is not an Agent")));
        }
        match run {
            Some(Some(terminal_id)) => {
                self.shared
                    .observe(ctx.actor.clone(), id, Observation::Stopped)?;
                match self.shared.terminals.kill(&terminal_id).await {
                    Err(err) if err.code != code::NOT_FOUND => return Err(err),
                    // It exited on its own first.
                    _ => {}
                }
            }
            Some(None) => return Err(RpcError::conflict(format!("{id} is still starting"))),
            // An earlier Daemon ran it; its Terminal ended with that Daemon.
            None => {}
        }
        if node.kind == NodeKind::Group && self.shared.rail().lift_children(id)? {
            ctx.emit(EventData::RailChanged);
        }
        Ok(())
    }

    /// Start Claude Code for node `id`, marked as starting, in a Terminal recorded in the Rail,
    /// then register and watch it; returns the Terminal's id. A failure leaves no settings file
    /// and no Terminal, and the caller unmarks `id`.
    async fn run_agent(
        &self,
        id: &str,
        cwd: &Path,
        prompt: Option<String>,
    ) -> Result<String, RpcError> {
        let spawned = match self.start(id, cwd).await {
            Ok(spawned) => spawned,
            Err(err) => {
                report_undo(
                    "delete the Agent's settings file",
                    Launcher::discard(&self.dir, id),
                );
                return Err(err);
            }
        };
        let clock = Arc::clone(&self.shared.clock);
        let run = Run {
            adapter: ClaudeCode::starting(move || clock()),
            prompt,
            terminal_id: spawned.id.clone(),
        };
        self.shared.runs().insert(id.to_owned(), Some(run));
        tokio::spawn(watch(
            Arc::clone(&self.shared),
            id.to_owned(),
            spawned.events,
        ));
        Ok(spawned.id)
    }

    async fn start(&self, id: &str, cwd: &Path) -> Result<terminal::Spawned, RpcError> {
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
        let attached = self.shared.rail().attach_terminal(id, &spawned.id);
        if let Err(err) = attached {
            report_undo(
                "kill the Agent's Terminal",
                self.shared.terminals.kill(&spawned.id).await,
            );
            return Err(err);
        }
        Ok(spawned)
    }
}

/// The error that made a caller undo its work is the one it returns, so a failed undo is only
/// reported.
fn report_undo<E: std::fmt::Display>(what: &str, undone: Result<(), E>) {
    if let Err(err) = undone {
        eprintln!("agents: could not {what}: {err}");
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
                shared.rail().move_node(&id, parent.as_deref(), index)?;
                ctx.emit(EventData::RailChanged);
                reply(&())
            }
            _ => Err(RpcError::method_not_found(method)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use contracts::terminal::{ExitedEvent, TitleEvent};
    use contracts::{Actor, Event, EventData, Kind};
    use rpc::Bus;
    use serde_json::json;
    use terminal::Terminals;
    use tokio::sync::broadcast;
    use tokio::task::JoinHandle;
    use tokio::time::Instant;

    use super::claude_code::{ClaudeCode, STAR_HOLD};
    use super::{Clock, Observation, Run, Shared, rail, watch};

    const PATIENCE: Duration = Duration::from_secs(60);
    const HOLD: Duration = Duration::from_millis(STAR_HOLD as u64);

    /// Agent `1`, watched over a stand-in for its Terminal's events, on tokio's clock, which these
    /// tests pause.
    struct Watched {
        _dir: tempfile::TempDir,
        shared: Arc<Shared>,
        terminal: broadcast::Sender<EventData>,
        events: broadcast::Receiver<Event>,
        watcher: JoinHandle<()>,
    }

    impl Watched {
        fn start() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let bus = Bus::new();
            let begun = Instant::now();
            let clock: Clock = Arc::new(move || begun.elapsed().as_millis() as i64);
            let adapter_clock = Arc::clone(&clock);
            let shared = Arc::new(Shared {
                rail: Mutex::new(rail::Rail::open(&dir.path().join("agents.db")).unwrap()),
                runs: Mutex::new(HashMap::new()),
                bus: bus.clone(),
                terminals: Arc::new(Terminals::open(dir.path(), bus.clone()).unwrap()),
                clock,
                opened: 0,
            });
            let run = Run {
                adapter: ClaudeCode::starting(move || adapter_clock()),
                prompt: None,
                terminal_id: "1".into(),
            };
            shared.runs().insert("1".into(), Some(run));
            let (terminal, terminal_events) = broadcast::channel(16);
            let events = bus.subscribe();
            let watcher = tokio::spawn(watch(Arc::clone(&shared), "1".into(), terminal_events));
            Self {
                _dir: dir,
                shared,
                terminal,
                events,
                watcher,
            }
        }

        fn signal(&self, event: &str) {
            let payload = json!({"hook_event_name": event, "tool_name": "Bash"});
            self.shared
                .observe(Actor::daemon(), "1", Observation::Signal(payload))
                .unwrap();
        }

        fn send(&self, data: EventData) {
            self.terminal.send(data).unwrap();
        }

        fn star(&self) {
            self.send(EventData::TerminalTitle(TitleEvent {
                id: "1".into(),
                title: "\u{2733} Claude Code".into(),
            }));
        }

        /// The Kind of the next `agent.status`.
        async fn next_kind(&mut self) -> Kind {
            loop {
                let event = tokio::time::timeout(PATIENCE, self.events.recv())
                    .await
                    .expect("an agent.status in time")
                    .unwrap();
                if let EventData::AgentStatus(status) = event.data {
                    return status.status.kind;
                }
            }
        }

        /// Whether an `agent.status` is waiting.
        fn has_news(&mut self) -> bool {
            std::iter::from_fn(|| self.events.try_recv().ok())
                .any(|event| matches!(event.data, EventData::AgentStatus(_)))
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a2_a_held_star_reads_idle_when_star_hold_has_passed_and_not_before() {
        let mut w = Watched::start();
        w.signal("PreToolUse");
        assert_eq!(w.next_kind().await, Kind::Working);
        let star = Instant::now();

        w.star();

        // An observation window: no Status may change while the star is held.
        tokio::time::sleep(HOLD - Duration::from_millis(1)).await;
        assert!(!w.has_news());
        assert_eq!(w.next_kind().await, Kind::Idle);
        assert_eq!(star.elapsed().as_millis(), HOLD.as_millis());
    }

    #[tokio::test(start_paused = true)]
    async fn a2_an_exit_ends_the_watch_without_waiting_out_a_held_star() {
        let w = Watched::start();
        w.signal("PreToolUse");
        w.star();
        let star = Instant::now();

        w.send(EventData::TerminalExited(ExitedEvent {
            id: "1".into(),
            code: Some(0),
        }));

        tokio::time::timeout(PATIENCE, w.watcher)
            .await
            .expect("the watch ends")
            .unwrap();
        assert!(star.elapsed() < HOLD);
    }
}
