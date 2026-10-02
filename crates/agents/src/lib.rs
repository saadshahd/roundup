//! Agents, the Rail tree and the Claude Code adapter. Owner: agents Builder.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use contracts::agent::{
    CreateGroupParams, MoveParams, NodeId, NodeKind, RailNode, RenameParams, SignalParams,
    SpawnParams, SpawnTerminalParams, StatusEvent,
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
mod name;
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
    /// A node is marked `Starting`, and unmarked on failure, in the same Rail critical section
    /// that creates or reserves it, so `agent.stop` never finds a starting node unmarked.
    runs: Mutex<HashMap<String, Slot>>,
    bus: Bus,
    terminals: Arc<Terminals>,
    /// Milliseconds since the Unix epoch. Adapters stamp Statuses with it and the watchers time
    /// Ticks by it, so the two never disagree about when a hold ends.
    clock: Clock,
    /// Since when an Agent without a Terminal has been `done`.
    opened: i64,
}

type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;

/// What `Shared::apply` leaves to do once its locks are let go: the Terminal id and prompt to
/// type at the first idle, and when, on `clock`, the adapter wants its next `Observation::Tick`.
type Applied = (Option<(String, String)>, Option<i64>);

impl Shared {
    fn rail(&self) -> MutexGuard<'_, rail::Rail> {
        self.rail.lock().expect("rail lock")
    }

    fn runs(&self) -> MutexGuard<'_, HashMap<String, Slot>> {
        self.runs.lock().expect("runs lock")
    }

    fn mark_starting(&self, id: &str) {
        let since = (self.clock)();
        self.runs().insert(
            id.to_owned(),
            Slot::Starting {
                since,
                named: false,
                held: VecDeque::new(),
            },
        );
    }

    /// An Agent, or a Meta-agent, has a Status: its live one; `working` while it starts; else
    /// `done`, because the Daemon that ran it is gone and its Terminal with it. Every node that
    /// leaves the module passes through here.
    fn present(&self, mut node: RailNode) -> RailNode {
        if node.kind == NodeKind::Agent || node.meta {
            let live = match self.runs().get(&node.id) {
                Some(Slot::Running(run)) => run.adapter.status().cloned(),
                Some(Slot::Starting { since, .. }) => Some(Status {
                    kind: Kind::Working,
                    label: "starting".into(),
                    since: *since,
                }),
                None => None,
            };
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
        let first_prompt = match &observation {
            Observation::Signal(payload) => {
                claude_code::submitted_prompt(payload).map(name::from_prompt)
            }
            _ => None,
        };
        // Taken before `runs`, as every other path does, so a rename and a prompt cannot deadlock.
        let mut rail = first_prompt.is_some().then(|| self.rail());
        let (prompt, tick_at) = {
            let mut runs = self.runs();
            let run = match runs.get_mut(id) {
                Some(Slot::Running(run)) => run,
                // A14: a Signal here is held, in arrival order, for when the Agent is
                // registered, instead of `NOT_FOUND`; anything else cannot arrive this early
                // (`watch` only starts once the Agent is `Running`).
                Some(Slot::Starting { held, .. }) => {
                    let Observation::Signal(payload) = observation else {
                        return Err(RpcError::not_found(format!("agent {id}")));
                    };
                    hold(id, held, actor, payload);
                    return Ok(None);
                }
                None => return Err(RpcError::not_found(format!("agent {id}"))),
            };
            self.apply(
                rail.as_deref_mut(),
                run,
                id,
                &actor,
                observation,
                first_prompt,
            )?
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

    /// Fold `observation` into `run`, which `observe` has already found `Running`; `rail`, when
    /// given, is locked by the caller, never here, so a caller draining several Signals under
    /// one `rail`-then-`runs` lock section (`finish_starting`) is never asked to lock it twice.
    fn apply(
        &self,
        rail: Option<&mut rail::Rail>,
        run: &mut Run,
        id: &str,
        actor: &Actor,
        observation: Observation,
        first_prompt: Option<Option<String>>,
    ) -> Result<Applied, RpcError> {
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
            self.bus.emit(actor.clone(), EventData::AgentStatus(event));
        }
        if let (Some(rail), Some(name)) = (rail, first_prompt)
            && !std::mem::replace(&mut run.named, true)
            && let Some(name) = name
            && !rail.node(id)?.meta
        {
            rail.rename(id, &name)?;
            self.bus.emit(actor.clone(), EventData::RailChanged);
        }
        let prompt = run.prompt.take_if(|_| idle);
        Ok((
            prompt.map(|prompt| (run.terminal_id.clone(), prompt)),
            run.adapter.tick_at(),
        ))
    }

    /// Swap `id` from `Starting` to `Running` (the Run `build` makes, told whether a
    /// `rail.rename` already settled its name) and apply every Signal held for it, in arrival
    /// order, all under one `rail`-then-`runs` lock section: a Signal that arrives once the
    /// Agent is registered can never be applied ahead of one still held for it (A14), and
    /// `agent.signal` never finds `id` missing between the two (it is never `NOT_FOUND` for an
    /// id that was spawned). Returns the Terminal id and prompt to type for each applied Signal
    /// that left the Agent idle, to type once the locks are let go.
    fn finish_starting(&self, id: &str, build: impl FnOnce(bool) -> Run) -> Vec<(String, String)> {
        let mut rail = self.rail();
        let mut runs = self.runs();
        let Some(Slot::Starting { named, held, .. }) = runs.remove(id) else {
            unreachable!("spawn marks {id} Starting before run_agent runs")
        };
        let mut run = build(named);
        let mut prompts = Vec::new();
        for (actor, payload) in held {
            let first_prompt = claude_code::submitted_prompt(&payload).map(name::from_prompt);
            let (prompt, _tick_at) = self
                .apply(
                    Some(&mut *rail),
                    &mut run,
                    id,
                    &actor,
                    Observation::Signal(payload),
                    first_prompt,
                )
                .expect(STILL_IN_RAIL);
            prompts.extend(prompt);
        }
        runs.insert(id.to_owned(), Slot::Running(run));
        prompts
    }
}

/// A node's place in `Shared::runs`.
enum Slot {
    /// Its Terminal is starting; `since` is when, on `clock`, it was marked.
    Starting {
        since: i64,
        /// A `rail.rename` came while it started; its first prompt must not undo it.
        named: bool,
        /// Signals that reached `agent.signal` in this window, in arrival order, applied once
        /// the Agent is registered (A14).
        held: VecDeque<(Actor, Value)>,
    },
    Running(Run),
}

impl Slot {
    /// The node's name is final: no prompt renames it.
    fn settle_name(&mut self) {
        match self {
            Self::Running(run) => run.named = true,
            Self::Starting { named, .. } => *named = true,
        }
    }
}

/// How many Signals a `Slot::Starting` holds (A14); past this, the oldest is dropped and logged,
/// as A5 does for a payload the adapter refuses.
const HELD_BOUND: usize = 8;

/// Queue `payload` for `id`, dropping and logging the oldest once `held` already holds
/// `HELD_BOUND`.
fn hold(id: &str, held: &mut VecDeque<(Actor, Value)>, actor: Actor, payload: Value) {
    if held.len() >= HELD_BOUND {
        held.pop_front();
        eprintln!(
            "agents: dropping the oldest Signal held for {id}: more than {HELD_BOUND} arrived \
             before it was registered"
        );
    }
    held.push_back((actor, payload));
}

/// An Agent's program and what is left to tell it.
struct Run {
    adapter: ClaudeCode,
    /// Typed into the Terminal at the first idle, then gone.
    prompt: Option<String>,
    /// The first prompt has been submitted, or a `rail.rename` came first: either way the name
    /// is settled and no prompt renames the node.
    named: bool,
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

/// A Starting id's Rail node is removed only by its own failed spawn, which never reaches
/// `finish_starting`, so the node a held Signal renames is still there to rename.
const STILL_IN_RAIL: &str = "a Starting id still has its Rail node once registration finishes";

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
        let node = {
            let mut rail = self.shared.rail();
            let node = rail.insert(
                NodeKind::Agent,
                name::UNNAMED,
                params.parent.as_deref(),
                None,
            )?;
            self.shared.mark_starting(&node.id);
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

    /// Put a Terminal running the user's login shell in the Rail, last under `parent`.
    async fn spawn_terminal(
        &self,
        ctx: &Ctx,
        params: SpawnTerminalParams,
    ) -> Result<RailNode, RpcError> {
        let node = self.shared.rail().insert(
            NodeKind::Terminal,
            &shell_name(std::env::var_os("SHELL").as_deref()),
            params.parent.as_deref(),
            None,
        )?;
        let spawned = match self
            .spawn_behind(&node.id, Path::new(&params.cwd), None)
            .await
        {
            Ok(spawned) => spawned,
            Err(err) => {
                report_undo(
                    "remove the Terminal from the Rail",
                    self.shared.rail().remove(&node.id),
                );
                return Err(err);
            }
        };
        ctx.emit(EventData::RailChanged);
        Ok(RailNode {
            terminal_id: Some(spawned.id),
            ..node
        })
    }

    /// Make a Group a Meta-agent: start a live Agent that sits at it, in the Project's folder.
    async fn promote(&self, ctx: &Ctx, id: &str) -> Result<RailNode, RpcError> {
        {
            let mut rail = self.shared.rail();
            rail.reserve_meta(id)?;
            self.shared.mark_starting(id);
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

    /// Stop an Agent's program; it stays in the Rail as `done`, unless its program had already
    /// ended: then it keeps the Status that ending gave it. A Meta-agent's children move up to
    /// where it was and keep running.
    async fn stop(&self, ctx: &Ctx, id: &str) -> Result<(), RpcError> {
        let (node, run) = {
            let rail = self.shared.rail();
            let node = rail.node(id)?;
            let run = self.shared.runs().get(id).map(|slot| match slot {
                Slot::Running(run) => Some(run.terminal_id.clone()),
                Slot::Starting { .. } => None,
            });
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
        let terminal_id = spawned.id.clone();
        let prompts = self.shared.finish_starting(id, move |named| Run {
            adapter: ClaudeCode::starting(move || clock()),
            prompt,
            named,
            terminal_id,
        });
        for (terminal_id, prompt) in prompts {
            tokio::spawn(type_prompt(
                Arc::clone(&self.shared.terminals),
                terminal_id,
                prompt,
            ));
        }
        tokio::spawn(watch(
            Arc::clone(&self.shared),
            id.to_owned(),
            spawned.events,
        ));
        Ok(spawned.id)
    }

    async fn start(&self, id: &str, cwd: &Path) -> Result<terminal::Spawned, RpcError> {
        // Waiting for Claude's config lock can take seconds; it must not hold a runtime thread.
        let (launcher, dir, node, folder) = (
            self.launcher.clone(),
            self.dir.clone(),
            id.to_owned(),
            cwd.to_owned(),
        );
        let argv = tokio::task::spawn_blocking(move || launcher.prepare(&dir, &node, &folder))
            .await
            .map_err(RpcError::internal)??;
        self.spawn_behind(id, cwd, Some(argv)).await
    }

    /// Start `command` (the login shell when `None`) in a Terminal and record it as the one behind
    /// node `id`.
    async fn spawn_behind(
        &self,
        id: &str,
        cwd: &Path,
        command: Option<Vec<String>>,
    ) -> Result<terminal::Spawned, RpcError> {
        let spawned = self
            .shared
            .terminals
            .spawn(TerminalSpawn {
                cwd: cwd.to_string_lossy().into_owned(),
                command,
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

/// The login shell's file name, as the Rail shows it; `shell` when `$SHELL` names none.
fn shell_name(shell: Option<&OsStr>) -> String {
    shell
        .and_then(|shell| Path::new(shell).file_name())
        .map_or_else(
            || "shell".into(),
            |name| name.to_string_lossy().into_owned(),
        )
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
                let node = {
                    let mut rail = shared.rail();
                    let node = rail.rename(&id, &name)?;
                    if let Some(slot) = shared.runs().get_mut(&id) {
                        slot.settle_name();
                    }
                    node
                };
                ctx.emit(EventData::RailChanged);
                reply(&shared.present(node))
            }
            "rail.spawnTerminal" => reply(&self.spawn_terminal(ctx, params(value)?).await?),
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
    use std::collections::{HashMap, VecDeque};
    use std::sync::{Arc, Mutex, mpsc};
    use std::thread;
    use std::time::Duration;

    use contracts::terminal::{ExitedEvent, TitleEvent};
    use contracts::{Actor, Event, EventData, Kind};
    use rpc::Bus;
    use serde_json::json;
    use terminal::Terminals;
    use tokio::sync::broadcast;
    use tokio::task::JoinHandle;
    use tokio::time::Instant;

    use super::claude_code::ClaudeCode;
    use super::{Clock, Observation, Run, Shared, Slot, rail, shell_name, watch};

    const PATIENCE: Duration = Duration::from_secs(60);
    /// Spelled out, not `STAR_HOLD`, so a changed hold fails these tests.
    const HOLD: Duration = Duration::from_millis(200);

    /// A `Shared` wired to a fresh temp-dir Rail and Terminals, paired with the `Bus` it was built
    /// from so a caller can subscribe to the same events.
    fn shared_over_temp_dir(clock: Clock) -> (tempfile::TempDir, Bus, Arc<Shared>) {
        let dir = tempfile::tempdir().unwrap();
        let bus = Bus::new();
        let shared = Arc::new(Shared {
            rail: Mutex::new(rail::Rail::open(&dir.path().join("agents.db")).unwrap()),
            runs: Mutex::new(HashMap::new()),
            bus: bus.clone(),
            terminals: Arc::new(Terminals::open(dir.path(), bus.clone()).unwrap()),
            clock,
            opened: 0,
        });
        (dir, bus, shared)
    }

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
            let begun = Instant::now();
            let clock: Clock = Arc::new(move || begun.elapsed().as_millis() as i64);
            let adapter_clock = Arc::clone(&clock);
            let (dir, bus, shared) = shared_over_temp_dir(clock);
            let run = Run {
                adapter: ClaudeCode::starting(move || adapter_clock()),
                prompt: None,
                named: false,
                terminal_id: "1".into(),
            };
            shared.runs().insert("1".into(), Slot::Running(run));
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
    async fn a2_a_second_star_during_a_hold_keeps_the_first_stars_deadline() {
        let mut w = Watched::start();
        w.signal("PreToolUse");
        assert_eq!(w.next_kind().await, Kind::Working);
        let star = Instant::now();
        w.star();
        tokio::time::sleep(HOLD / 2).await;

        w.star();

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

    #[test]
    fn a10_the_node_is_named_for_the_shell_and_shell_when_there_is_none() {
        let name = |shell: Option<&str>| shell_name(shell.map(std::ffi::OsStr::new));
        assert_eq!(name(Some("/bin/zsh")), "zsh");
        assert_eq!(name(Some("fish")), "fish");
        assert_eq!(name(Some("")), "shell");
        assert_eq!(name(None), "shell");
    }

    /// `watch` only starts once an Agent is `Running`, so in production a non-Signal Observation
    /// never reaches a `Starting` id; reached directly here, it must still answer the way
    /// `agent.signal` answers any other unknown id, not pretend the id is already registered.
    #[test]
    fn a14_a_non_signal_observation_for_a_starting_id_is_still_not_found() {
        let (_dir, _bus, shared) = shared_over_temp_dir(Arc::new(|| 0));
        shared.runs().insert(
            "1".into(),
            Slot::Starting {
                since: 0,
                named: false,
                held: VecDeque::new(),
            },
        );

        let err = shared
            .observe(Actor::daemon(), "1", Observation::Tick)
            .unwrap_err();

        assert_eq!(err.code, rpc::code::NOT_FOUND);
    }

    /// How many trials `a14_a_signal_released_once_the_agent_is_registered_never_outruns_the_held_one`
    /// runs. Measured by reverting `finish_starting` to let go of `rail` and `runs` right after
    /// the swap into `Running`, then re-locking `runs` once per held Signal to apply it (the old
    /// bug this test exists to catch): over 30 runs of the mutated binary, every run failed, the
    /// latest at trial 5536. `TRIALS` leaves roughly 3.6x that much room, so a reintroduced bug
    /// would need to be dramatically harder to hit than the one measured to slip past a run.
    const TRIALS: usize = 20_000;

    /// `finish_starting` takes `rail` then `runs` for its whole swap into `Running` and its drain
    /// of what was held, so a Signal released the instant the Agent is registered still cannot
    /// land ahead of the one held for it (A14). So this does not race blindly: a rendezvous over a
    /// zero-capacity channel holds the late call at the threshold of its own call to `observe`
    /// until `finish_starting`'s `build` hook — already a plain closure argument, not a new
    /// public seam — confirms, by having been called at all, that both locks are still held.
    /// Only then are both sides let go. That leaves exactly the race the fix is answerable for:
    /// once released, a correct `finish_starting` never lets go of `runs` until every held Signal
    /// is applied, so the late call cannot win no matter how the OS schedules it; a regressed one
    /// drops `runs` early and must then win a second, real race to re-lock it before the already-
    /// waiting late call does — a race this test cannot referee, only repeat (`TRIALS`).
    #[test]
    fn a14_a_signal_released_once_the_agent_is_registered_never_outruns_the_held_one() {
        let (_dir, bus, shared) = shared_over_temp_dir(Arc::new(|| 0));
        let mut events = bus.subscribe();
        for i in 0..TRIALS {
            let id = i.to_string();
            shared.runs().insert(
                id.clone(),
                Slot::Starting {
                    since: 0,
                    named: false,
                    held: VecDeque::from([(
                        Actor::daemon(),
                        json!({"hook_event_name": "PreToolUse", "tool_name": "Bash"}),
                    )]),
                },
            );

            // `build` is called with both locks already held, so reaching here and blocking
            // proves `finish_starting` cannot have let either go yet.
            let (holding_tx, holding_rx) = mpsc::sync_channel::<()>(0);
            let (go_tx, go_rx) = mpsc::sync_channel::<()>(0);
            let registering = {
                let shared = Arc::clone(&shared);
                let id = id.clone();
                thread::spawn(move || {
                    shared.finish_starting(&id, |named| {
                        holding_tx.send(()).unwrap();
                        go_rx.recv().unwrap();
                        Run {
                            adapter: ClaudeCode::starting(|| 0),
                            prompt: None,
                            named,
                            terminal_id: id.clone(),
                        }
                    })
                })
            };
            holding_rx.recv().unwrap();

            // A zero-capacity send only completes once this thread is received by the line
            // below, so by the time it does, the late call's very next step is `observe`.
            let (about_tx, about_rx) = mpsc::sync_channel::<()>(0);
            let late = {
                let shared = Arc::clone(&shared);
                let id = id.clone();
                thread::spawn(move || {
                    about_tx.send(()).unwrap();
                    shared.observe(
                        Actor::daemon(),
                        &id,
                        Observation::Signal(
                            json!({"hook_event_name": "Stop", "tool_name": "Bash"}),
                        ),
                    )
                })
            };
            about_rx.recv().unwrap();
            go_tx.send(()).unwrap();

            registering.join().unwrap();
            late.join().unwrap().unwrap();

            let mut kinds = vec![];
            while let Ok(event) = events.try_recv() {
                if let EventData::AgentStatus(s) = event.data
                    && s.id == id
                {
                    kinds.push(s.status.kind);
                }
            }
            assert_eq!(kinds, [Kind::Working, Kind::Idle], "trial {i}");
        }
    }
}
