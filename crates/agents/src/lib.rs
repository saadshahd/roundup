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
use contracts::project::{ProjectSettings, Worktrees};
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
pub mod worktree;

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
    /// The user's `decision.answer` reached the hook (`scenarios/decisions.md` H9).
    Answered,
    /// The hook's own side closed its connection with no later Signal (H7(b), H9).
    Dismissed,
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
    /// The same Terminals, behind the seam `stop` and `remove` kill through: real in production,
    /// a fake in the one test that proves a failed kill deletes nothing (A16), since `Terminals`
    /// is a concrete type a test cannot otherwise make fail on demand.
    kill: Arc<dyn KillsTerminals>,
    /// Milliseconds since the Unix epoch. Adapters stamp Statuses with it and the watchers time
    /// Ticks by it, so the two never disagree about when a hold ends.
    clock: Clock,
    /// Since when an Agent without a Terminal has been `done`.
    opened: i64,
}

/// What kills a Terminal's program; real `Terminals` in production, a fake where a test needs a
/// kill that fails without a real process refusing to die.
#[async_trait]
trait KillsTerminals: Send + Sync {
    async fn kill(&self, id: &str) -> Result<(), RpcError>;
}

#[async_trait]
impl KillsTerminals for Terminals {
    async fn kill(&self, id: &str) -> Result<(), RpcError> {
        Terminals::kill(self, id).await
    }
}

/// How long `kill_or_already_gone` waits for the Terminal's own exit event once the program is
/// reaped, before giving up on it. `kill` sends SIGHUP then, after a ~250ms grace period
/// (`portable_pty`'s own `Child::kill`), SIGKILL to the program's single pid, never to its
/// process group; a descendant the program backgrounded before dying inherits the slave end of
/// the PTY and, if it also inherited SIGHUP ignored, outlives the signal that was meant to end
/// it, which would otherwise keep this wait from ever ending. Comfortably above that grace period
/// and the reader thread's scheduling delay under load (`a16_a_running_terminal_is_killed_and_removed`'s
/// stress run).
pub const KILL_WAIT_BOUND: Duration = Duration::from_secs(2);

/// Kill a Terminal's program and return once it is observably not running (A16) or
/// `KILL_WAIT_BOUND` has passed, whichever comes first: `kill` itself returns as soon as the
/// program is reaped, which can race the Terminal's own reader thread noticing the program's end
/// and flipping `terminal.list`'s `running`, and a descendant left holding the PTY open can keep
/// that race from ever resolving (see `KILL_WAIT_BOUND`). `terminal::Shared::subscribe` checks
/// `handle` under the same lock `finish` clears it under, so subscribing before `kill` runs means
/// the exit event is never missed: either the receiver exists before `finish` publishes it, or
/// `subscribe` already found the Terminal gone and returned `NOT_FOUND`. `terminal_id` may already
/// be gone, from `kill` or from the subscribe, which is not a failure.
async fn kill_or_already_gone(
    kill: &dyn KillsTerminals,
    terminals: &Terminals,
    terminal_id: &str,
) -> Result<(), RpcError> {
    let exit = terminals.subscribe(terminal_id).ok();
    match kill.kill(terminal_id).await {
        Err(err) if err.code != code::NOT_FOUND => return Err(err),
        _ => {}
    }
    if let Some(mut exit) = exit {
        let _ = tokio::time::timeout(KILL_WAIT_BOUND, wait_for_exit(&mut exit)).await;
    }
    Ok(())
}

/// Wait for a Terminal's own exit event, bounded by the caller. A lag never loses it, since it is
/// always the last event a Terminal sends. `Terminals` lives in an `Arc` for as long as the Daemon
/// does, so the channel this reads never closes while this can run; `Closed` is unreachable but
/// still has to be matched to satisfy `RecvError`.
async fn wait_for_exit(events: &mut Receiver<EventData>) {
    loop {
        match events.recv().await {
            Ok(EventData::TerminalExited(_)) => return,
            Ok(_) | Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => return,
        }
    }
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
            // Logged, not panicked: a panic here would unwind while `rail` and `runs` are both
            // still locked, poisoning them and taking every later `rail()`/`runs()` call down
            // with it. `rail.node(id)` itself cannot be `NOT_FOUND` (a Starting id's Rail node is
            // removed only by its own failed spawn, which never reaches here), but the write a
            // held rename makes can still fail on its own account.
            match self.apply(
                Some(&mut *rail),
                &mut run,
                id,
                &actor,
                Observation::Signal(payload),
                first_prompt,
            ) {
                Ok((prompt, _tick_at)) => prompts.extend(prompt),
                Err(err) => eprintln!(
                    "agents: could not apply a Signal held for {id} while it was starting: {err}"
                ),
            }
        }
        runs.insert(id.to_owned(), Slot::Running(run));
        prompts
    }

    /// Stop an Agent's program; it stays in the Rail as `done`, unless its program had already
    /// ended, in which case it keeps the Status that ending gave it. A Meta-agent's children
    /// move up to where it was and keep running (A7).
    async fn stop(&self, actor: Actor, id: &str) -> Result<(), RpcError> {
        let (node, run) = {
            let rail = self.rail();
            let node = rail.node(id)?;
            let run = self.runs().get(id).map(|slot| match slot {
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
                self.observe(actor.clone(), id, Observation::Stopped)?;
                kill_or_already_gone(&*self.kill, &self.terminals, &terminal_id).await?;
            }
            Some(None) => return Err(RpcError::conflict(format!("{id} is still starting"))),
            // An earlier Daemon ran it; its Terminal ended with that Daemon.
            None => {}
        }
        if node.kind == NodeKind::Group && self.rail().lift_children(id)? {
            self.bus.emit(actor, EventData::RailChanged);
        }
        Ok(())
    }

    /// Delete a node from the Rail (A16). An Agent or a Meta-agent is stopped first, as `stop`
    /// does (A7's lift of a Meta-agent's children included); a plain Terminal is killed instead,
    /// since `stop` refuses one. Either failing returns its error and deletes nothing, so a
    /// second `rail.remove` can retry. A plain Group's children move to its own parent, at its
    /// place, with the delete itself in one transaction (`Rail::remove`): a failed delete there
    /// leaves them still under it. A Meta-agent's children are already lifted by `stop` by the
    /// time the delete runs, so a failed delete after that just leaves the node `done` on the
    /// Rail with no children of its own left to lose. A successful delete also drops `id` from
    /// `runs`, so a removed Agent's Slot does not sit there forever and a later `agent.signal`
    /// for it is `NOT_FOUND` again, instead of quietly applying to a node no longer on the Rail.
    async fn remove(&self, actor: Actor, id: &str) -> Result<(), RpcError> {
        let node = self.rail().node(id)?;
        if node.kind == NodeKind::Agent || node.meta {
            self.stop(actor.clone(), id).await?;
        } else if node.kind == NodeKind::Terminal
            && let Some(terminal_id) = &node.terminal_id
        {
            kill_or_already_gone(&*self.kill, &self.terminals, terminal_id).await?;
        }
        self.rail().remove(id)?;
        self.runs().remove(id);
        self.bus.emit(actor, EventData::RailChanged);
        Ok(())
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
        let tick_at = match shared.observe(Actor::daemon(), &id, observation) {
            Ok(tick_at) => tick_at,
            // `rail.remove` evicts `id` from `runs` once it deletes the node (A16); an Exit
            // already in flight when that happened has nothing left to watch.
            Err(err) if err.code == code::NOT_FOUND => return,
            Err(err) => panic!("{REGISTERED}: {err}"),
        };
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
    git: worktree::Git,
}

impl Agents {
    /// `dir` is the Project's `.roundup/` directory. `bus` is for events no call caused.
    /// Claude Code is `ROUNDUP_CLAUDE_BIN` (default `claude`); `git` is whatever `PATH` finds.
    pub fn open(dir: &Path, bus: Bus, terminals: Arc<Terminals>) -> Result<Self, OpenError> {
        Self::open_with(
            dir,
            bus,
            terminals,
            Launcher::from_env()?,
            worktree::Git::from_env(),
        )
    }

    pub fn open_with(
        dir: &Path,
        bus: Bus,
        terminals: Arc<Terminals>,
        launcher: Launcher,
        git: worktree::Git,
    ) -> Result<Self, OpenError> {
        Ok(Self {
            shared: Arc::new(Shared {
                rail: Mutex::new(rail::Rail::open(&dir.join("agents.db"))?),
                runs: Mutex::new(HashMap::new()),
                bus,
                kill: Arc::clone(&terminals) as Arc<dyn KillsTerminals>,
                terminals,
                clock: Arc::new(now_ms),
                opened: now_ms(),
            }),
            dir: dir.to_owned(),
            launcher,
            git,
        })
    }

    /// The Project folder: `.roundup/`'s parent, where its git repository (if any) lives.
    fn project_dir(&self) -> &Path {
        self.dir.parent().unwrap_or(&self.dir)
    }

    /// Put a new Agent in the Rail and start Claude Code for it in a Terminal.
    async fn spawn(&self, ctx: &Ctx, params: SpawnParams) -> Result<RailNode, RpcError> {
        let cwd = Path::new(&params.cwd);
        let id = {
            let mut rail = self.shared.rail();
            let node = rail.insert(
                NodeKind::Agent,
                name::UNNAMED,
                params.parent.as_deref(),
                None,
            )?;
            self.shared.mark_starting(&node.id);
            node.id
        };
        let terminal_id = match self.run_agent(&id, cwd, params.prompt).await {
            Ok(terminal_id) => terminal_id,
            Err(err) => {
                let mut rail = self.shared.rail();
                self.shared.runs().remove(&id);
                report_undo("remove the Agent from the Rail", rail.remove(&id));
                return Err(err);
            }
        };
        ctx.emit(EventData::RailChanged);
        let node = self.shared.rail().node(&id)?;
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
        let project = self.project_dir().to_owned();
        if let Err(err) = self.run_agent(id, &project, None).await {
            let mut rail = self.shared.rail();
            self.shared.runs().remove(id);
            report_undo("give the Group back its plain state", rail.release_meta(id));
            return Err(err);
        }
        ctx.emit(EventData::RailChanged);
        Ok(self.shared.present(self.shared.rail().node(id)?))
    }

    /// When the Project's `worktrees` setting is on, make a Worktree for `id` (G2) and map `cwd`
    /// into it; `None` when the setting is off, so `run_agent` uses `cwd` unchanged. Any failure
    /// here leaves no branch, no worktree directory and no Worktree recorded on the node.
    async fn provision_worktree(
        &self,
        id: &str,
        cwd: &Path,
    ) -> Result<Option<(PathBuf, worktree::Worktree)>, RpcError> {
        if !self.shared.rail().get_worktrees()?.on {
            return Ok(None);
        }
        let project = self.project_dir().to_owned();
        let (real_project, real_cwd) = real_paths(&project, cwd)?;
        self.shared.rail().mark_worktree_provisioning(id)?;
        let (git, provisioning_id) = (self.git.clone(), id.to_owned());
        let made =
            match tokio::task::spawn_blocking(move || git.provision(&project, &provisioning_id))
                .await
                .map_err(RpcError::internal)?
            {
                Ok(made) => made,
                Err(err) => {
                    report_undo(
                        "clear the Agent's worktree",
                        self.shared.rail().clear_worktree(id),
                    );
                    return Err(err);
                }
            };
        let mapped = match worktree::map_cwd(&real_project, &real_cwd, &made.path) {
            Some(mapped) if mapped.is_dir() => mapped,
            _ => {
                self.discard_worktree(id, made).await;
                return Err(RpcError::new(
                    code::INVALID_PARAMS,
                    format!(
                        "cwd_not_in_worktree: {} is not in the fresh worktree",
                        cwd.display()
                    ),
                ));
            }
        };
        let recorded = contracts::agent::Worktree {
            path: made.path.to_string_lossy().into_owned(),
            branch: made.branch.clone(),
            base: made.base.clone(),
        };
        let recorded_ok = self.shared.rail().set_worktree(id, &recorded);
        if let Err(err) = recorded_ok {
            self.discard_worktree(id, made).await;
            return Err(err);
        }
        Ok(Some((mapped, made)))
    }

    /// Undo a Worktree `provision_worktree` made and clear the node's record of it: used when a
    /// later step (the cwd check, or starting the Agent itself) fails.
    async fn discard_worktree(&self, id: &str, worktree: worktree::Worktree) {
        let (git, project) = (self.git.clone(), self.project_dir().to_owned());
        let _ = tokio::task::spawn_blocking(move || git.discard(&project, &worktree)).await;
        report_undo(
            "clear the Agent's worktree",
            self.shared.rail().clear_worktree(id),
        );
    }

    /// Start Claude Code for node `id`, marked as starting, in a Terminal recorded in the Rail,
    /// then register and watch it; returns the Terminal's id. A failure leaves no settings file,
    /// no Terminal and no Worktree, and the caller unmarks `id`.
    async fn run_agent(
        &self,
        id: &str,
        cwd: &Path,
        prompt: Option<String>,
    ) -> Result<String, RpcError> {
        let provisioned = self.provision_worktree(id, cwd).await?;
        let effective_cwd = provisioned
            .as_ref()
            .map_or_else(|| cwd.to_owned(), |(cwd, _)| cwd.clone());
        let spawned = match self.start(id, &effective_cwd).await {
            Ok(spawned) => spawned,
            Err(err) => {
                report_undo(
                    "delete the Agent's settings file",
                    Launcher::discard(&self.dir, id),
                );
                if let Some((_, worktree)) = provisioned {
                    self.discard_worktree(id, worktree).await;
                }
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

/// `project` and `cwd` with symlinks and `..` resolved, the way A4's trust check reads them, so a
/// `cwd` is judged by where it is and not by how it is written. Checked before `git` runs.
fn real_paths(project: &Path, cwd: &Path) -> Result<(PathBuf, PathBuf), RpcError> {
    let invalid = |message: String| RpcError::new(code::INVALID_PARAMS, message);
    let real_cwd = cwd
        .canonicalize()
        .map_err(|err| invalid(format!("cwd {} is unusable: {err}", cwd.display())))?;
    let real_project = project
        .canonicalize()
        .map_err(|err| RpcError::internal(format!("{}: {err}", project.display())))?;
    if !real_cwd.starts_with(&real_project) {
        return Err(invalid(format!(
            "cwd {} is outside the project folder {}",
            real_cwd.display(),
            real_project.display()
        )));
    }
    Ok((real_project, real_cwd))
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
        &["agent", "rail", "project"]
    }

    async fn call(&self, ctx: &Ctx, method: &str, value: Value) -> Result<Value, RpcError> {
        let shared = &self.shared;
        match method {
            "agent.spawn" => reply(&self.spawn(ctx, params(value)?).await?),
            "agent.stop" => {
                let NodeId { id } = params(value)?;
                shared.stop(ctx.actor.clone(), &id).await?;
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
            "project.setWorktrees" => {
                let Worktrees { on, check } = params(value)?;
                if check.as_deref() == Some("") {
                    return Err(RpcError::new(
                        code::INVALID_PARAMS,
                        "check must not be empty",
                    ));
                }
                shared.rail().set_worktrees(&Worktrees { on, check })?;
                reply(&())
            }
            "project.get" => {
                let worktrees = shared.rail().get_worktrees()?;
                reply(&ProjectSettings { worktrees })
            }
            "rail.move" => {
                let MoveParams { id, parent, index } = params(value)?;
                shared.rail().move_node(&id, parent.as_deref(), index)?;
                ctx.emit(EventData::RailChanged);
                reply(&())
            }
            "rail.remove" => {
                let NodeId { id } = params(value)?;
                shared.remove(ctx.actor.clone(), &id).await?;
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

    use async_trait::async_trait;
    use contracts::agent::NodeKind;
    use contracts::terminal::{ExitedEvent, SpawnParams, TitleEvent};
    use contracts::{Actor, Event, EventData, Kind};
    use rpc::Bus;
    use serde_json::json;
    use terminal::Terminals;
    use tokio::sync::broadcast;
    use tokio::sync::broadcast::error::TryRecvError;
    use tokio::task::JoinHandle;
    use tokio::time::Instant;

    use super::claude_code::ClaudeCode;
    use super::{Clock, KillsTerminals, Observation, Run, Shared, Slot, rail, shell_name, watch};

    const PATIENCE: Duration = Duration::from_secs(60);
    /// Spelled out, not `STAR_HOLD`, so a changed hold fails these tests.
    const HOLD: Duration = Duration::from_millis(200);

    /// A `Shared` wired to a fresh temp-dir Rail and Terminals, paired with the `Bus` it was built
    /// from so a caller can subscribe to the same events.
    fn shared_over_temp_dir(clock: Clock) -> (tempfile::TempDir, Bus, Arc<Shared>) {
        let dir = tempfile::tempdir().unwrap();
        let bus = Bus::new();
        let terminals = Arc::new(Terminals::open(dir.path(), bus.clone()).unwrap());
        let shared = Arc::new(Shared {
            rail: Mutex::new(rail::Rail::open(&dir.path().join("agents.db")).unwrap()),
            runs: Mutex::new(HashMap::new()),
            bus: bus.clone(),
            kill: Arc::clone(&terminals) as Arc<dyn super::KillsTerminals>,
            terminals,
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

    /// A second connection holding a write transaction makes a write through `Rail`'s own
    /// connection fail with a real SQLite error once rusqlite's default 5 s `busy_timeout`
    /// elapses, without reaching into `Rail`'s private `db` field.
    fn lock_db_for_writes(dir: &std::path::Path) -> rusqlite::Connection {
        let lock = rusqlite::Connection::open(dir.join("agents.db")).unwrap();
        lock.execute_batch("BEGIN IMMEDIATE;").unwrap();
        lock
    }

    /// A held Signal whose `rail.rename` fails (here, because another connection holds the
    /// write lock) must not panic while `finish_starting` still holds `rail` and `runs`: a panic
    /// there would poison both mutexes, so every later `rail()`/`runs()` call would panic too.
    #[test]
    fn a14_a_failed_rail_write_for_a_held_signal_does_not_poison_the_locks() {
        let (dir, _bus, shared) = shared_over_temp_dir(Arc::new(|| 0));
        let id = shared
            .rail()
            .insert(contracts::agent::NodeKind::Agent, "new-agent", None, None)
            .unwrap()
            .id;
        shared.mark_starting(&id);
        if let Some(Slot::Starting { held, .. }) = shared.runs().get_mut(&id) {
            held.push_back((
                Actor::daemon(),
                json!({"hook_event_name": "UserPromptSubmit", "prompt": "fix the build"}),
            ));
        }
        let lock = lock_db_for_writes(dir.path());

        let prompts = shared.finish_starting(&id, |named| Run {
            adapter: ClaudeCode::starting(|| 0),
            prompt: None,
            named,
            terminal_id: "1".into(),
        });

        drop(lock);
        assert_eq!(prompts, Vec::new());
        // Neither lock was poisoned by the failed write: both can still be acquired.
        assert!(matches!(shared.runs().get(&id), Some(Slot::Running(_))));
        shared.rail().tree().unwrap();
    }

    /// How many trials `a14_a_signal_released_once_the_agent_is_registered_never_outruns_the_held_one`
    /// runs. Measured against a `finish_starting` that lets go of `rail` and `runs` right after
    /// the swap into `Running`, then re-locks `runs` once per held Signal to apply it — the
    /// regression this test catches if the one `rail`-then-`runs` lock section is ever split:
    /// over 30 runs of that mutated binary, every run failed, the latest at trial 5536.
    /// `TRIALS` leaves roughly 3.6x that much room, so a reintroduced bug would need to be
    /// dramatically harder to hit than the one measured to slip past a run.
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

    /// A Terminal kill that always fails, so A16's stop-failure clause doesn't depend on a real
    /// process refusing to die: `Terminals` is a concrete type, so this is the one seam that can
    /// make its kill fail on demand.
    struct FailingKill;

    #[async_trait]
    impl KillsTerminals for FailingKill {
        async fn kill(&self, _id: &str) -> Result<(), rpc::RpcError> {
            Err(rpc::RpcError::internal("the kill failed"))
        }
    }

    /// `Shared` wired like `shared_over_temp_dir`, but every kill fails.
    fn shared_with_a_failing_kill() -> Arc<Shared> {
        let dir = tempfile::tempdir().unwrap();
        let bus = Bus::new();
        Arc::new(Shared {
            rail: Mutex::new(rail::Rail::open(&dir.path().join("agents.db")).unwrap()),
            runs: Mutex::new(HashMap::new()),
            bus: bus.clone(),
            terminals: Arc::new(Terminals::open(dir.path(), bus).unwrap()),
            kill: Arc::new(FailingKill),
            clock: Arc::new(|| 0),
            opened: 0,
        })
    }

    /// A Running Agent node, with no watcher: the tests here drive `stop`/`remove` directly, so
    /// nothing reaps its Terminal's events.
    fn running_agent(shared: &Shared, parent: Option<&str>) -> String {
        let id = shared
            .rail()
            .insert(NodeKind::Agent, "a", parent, Some("1"))
            .unwrap()
            .id;
        shared.runs().insert(
            id.clone(),
            Slot::Running(Run {
                adapter: ClaudeCode::starting(|| 0),
                prompt: None,
                named: true,
                terminal_id: "1".into(),
            }),
        );
        id
    }

    #[tokio::test]
    async fn a16_a_failed_stop_returns_the_error_and_deletes_nothing() {
        let shared = shared_with_a_failing_kill();
        let id = running_agent(&shared, None);

        let err = shared.remove(Actor::daemon(), &id).await.unwrap_err();

        assert_eq!(err.code, rpc::code::INTERNAL);
        assert_eq!(shared.rail().tree().unwrap().len(), 1);
    }

    /// A Meta-agent whose kill fails must not have its children lifted either: `stop` returning
    /// early before it calls `lift_children` is what this proves.
    #[tokio::test]
    async fn a16_a_failed_stop_of_a_meta_agent_leaves_its_children_in_place() {
        let shared = shared_with_a_failing_kill();
        let group = shared
            .rail()
            .insert(NodeKind::Group, "g", None, None)
            .unwrap()
            .id;
        shared.rail().reserve_meta(&group).unwrap();
        shared.runs().insert(
            group.clone(),
            Slot::Running(Run {
                adapter: ClaudeCode::starting(|| 0),
                prompt: None,
                named: true,
                terminal_id: "1".into(),
            }),
        );
        let child = shared
            .rail()
            .insert(NodeKind::Group, "child", Some(&group), None)
            .unwrap()
            .id;

        shared.remove(Actor::daemon(), &group).await.unwrap_err();

        let tree = shared.rail().tree().unwrap();
        let child_node = tree.iter().find(|n| n.id == child).unwrap();
        assert_eq!(child_node.parent.as_deref(), Some(group.as_str()));
    }

    /// A plain Terminal node, with no watcher, for a test that drives `remove` directly.
    fn terminal_node(shared: &Shared, parent: Option<&str>) -> String {
        shared
            .rail()
            .insert(NodeKind::Terminal, "t", parent, Some("1"))
            .unwrap()
            .id
    }

    #[tokio::test]
    async fn a16_a_failed_kill_of_a_terminal_returns_the_error_and_deletes_nothing() {
        let shared = shared_with_a_failing_kill();
        let id = terminal_node(&shared, None);

        let err = shared.remove(Actor::daemon(), &id).await.unwrap_err();

        assert_eq!(err.code, rpc::code::INTERNAL);
        assert_eq!(shared.rail().tree().unwrap().len(), 1);
    }

    /// A Terminal kill that fails once, then succeeds, so a test can retry a stop that failed.
    struct FlakyKill(std::sync::atomic::AtomicBool);

    #[async_trait]
    impl KillsTerminals for FlakyKill {
        async fn kill(&self, _id: &str) -> Result<(), rpc::RpcError> {
            if self.0.swap(true, std::sync::atomic::Ordering::SeqCst) {
                Ok(())
            } else {
                Err(rpc::RpcError::internal("the kill failed"))
            }
        }
    }

    /// `Shared` wired like `shared_with_a_failing_kill`, but its kill succeeds from the second
    /// call on, so a retried `remove` can succeed.
    fn shared_with_a_flaky_kill() -> (tempfile::TempDir, Arc<Shared>) {
        let dir = tempfile::tempdir().unwrap();
        let bus = Bus::new();
        let shared = Arc::new(Shared {
            rail: Mutex::new(rail::Rail::open(&dir.path().join("agents.db")).unwrap()),
            runs: Mutex::new(HashMap::new()),
            bus: bus.clone(),
            terminals: Arc::new(Terminals::open(dir.path(), bus).unwrap()),
            kill: Arc::new(FlakyKill(std::sync::atomic::AtomicBool::new(false))),
            clock: Arc::new(|| 0),
            opened: 0,
        });
        (dir, shared)
    }

    /// The Status a failed stop leaves (`done`, from `agent.stop`'s own Stopped observation, set
    /// before the kill that then fails) is exactly what lets a second `rail.remove` retry and
    /// succeed.
    #[tokio::test]
    async fn a16_a_failed_stops_done_status_lets_a_second_remove_retry_and_succeed() {
        let (_dir, shared) = shared_with_a_flaky_kill();
        let id = running_agent(&shared, None);

        let err = shared.remove(Actor::daemon(), &id).await.unwrap_err();

        assert_eq!(err.code, rpc::code::INTERNAL);
        let node = shared.present(shared.rail().node(&id).unwrap());
        assert_eq!(node.status.unwrap().kind, Kind::Done);

        shared.remove(Actor::daemon(), &id).await.unwrap();

        assert!(shared.rail().tree().unwrap().is_empty());
    }

    /// `remove`'s eviction of `id` from `runs` can race a Terminal exit already in flight: the
    /// watcher must end quietly, not panic through `REGISTERED`'s `.expect`.
    #[tokio::test(start_paused = true)]
    async fn a16_a_terminal_exit_after_its_id_is_evicted_from_runs_does_not_panic_the_watcher() {
        let w = Watched::start();
        w.shared.runs().remove("1");

        w.send(EventData::TerminalExited(ExitedEvent {
            id: "1".into(),
            code: Some(0),
        }));

        tokio::time::timeout(PATIENCE, w.watcher)
            .await
            .expect("the watch ends")
            .unwrap();
    }

    /// A lag skips the events it dropped, but must not end the wait on its own: only
    /// `TerminalExited` may. With capacity 1, every send before a read overwrites the one before
    /// it, so the receiver sees exactly one `Lagged`, then the last message sent.
    #[tokio::test]
    async fn wait_for_exit_keeps_waiting_past_a_lag() {
        let (tx, mut rx) = broadcast::channel(1);
        tx.send(EventData::TerminalTitle(TitleEvent {
            id: "1".into(),
            title: "a".into(),
        }))
        .unwrap();
        tx.send(EventData::TerminalTitle(TitleEvent {
            id: "1".into(),
            title: "b".into(),
        }))
        .unwrap();
        tx.send(EventData::TerminalExited(ExitedEvent {
            id: "1".into(),
            code: Some(0),
        }))
        .unwrap();

        super::wait_for_exit(&mut rx).await;

        // Had the lag ended the wait instead of being skipped past, `TerminalExited` would still
        // be sitting unread here.
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    }

    /// An ordinary event (a title change, say output) must not end the wait on its own: only
    /// `TerminalExited` may. A large capacity means both sends land without a lag, so a mutant
    /// that returns on any `Ok(_)` would stop at the title and leave `TerminalExited` unread.
    #[tokio::test]
    async fn wait_for_exit_keeps_waiting_past_an_ordinary_event() {
        let (tx, mut rx) = broadcast::channel(8);
        tx.send(EventData::TerminalTitle(TitleEvent {
            id: "1".into(),
            title: "a".into(),
        }))
        .unwrap();
        tx.send(EventData::TerminalExited(ExitedEvent {
            id: "1".into(),
            code: Some(0),
        }))
        .unwrap();

        super::wait_for_exit(&mut rx).await;

        // Had the title event ended the wait, `TerminalExited` would still be sitting unread here.
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    }

    /// A kill that fails must return its error at once, not wait for an exit event from a program
    /// the kill never touched: a real Terminal that is never killed proves the wait is skipped,
    /// since it would otherwise run out `KILL_WAIT_BOUND` before returning.
    #[tokio::test]
    async fn kill_or_already_gone_returns_a_failed_kills_error_without_waiting() {
        let dir = tempfile::tempdir().unwrap();
        let terminals = Terminals::open(dir.path(), Bus::new()).unwrap();
        let spawned = terminals
            .spawn(SpawnParams {
                cwd: dir.path().to_string_lossy().into_owned(),
                command: Some(vec!["sleep".into(), "30".into()]),
                env: Default::default(),
                cols: 80,
                rows: 24,
            })
            .await
            .unwrap();
        let began = Instant::now();

        let err = super::kill_or_already_gone(&FailingKill, &terminals, &spawned.id)
            .await
            .unwrap_err();

        assert_eq!(err.code, rpc::code::INTERNAL);
        assert!(began.elapsed() < super::KILL_WAIT_BOUND / 2);
    }
}
