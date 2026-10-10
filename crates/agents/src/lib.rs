//! Agents, the Rail tree and the Claude Code adapter. Owner: agents Builder.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use contracts::agent::{
    Channel, ChannelEvent, CreateWorkstreamParams, Landed, MoveParams, NodeId, NodeKind, Order,
    PromptParams, RailNode, RenameParams, SetOrderParams, SignalParams, SpawnParams,
    SpawnTerminalParams, StatusEvent, Stray, StrayEvent, WorktreeState,
};
use contracts::decision::{AnswerParams, AskParams, Outcome, PermissionParams};
use contracts::project::{ProjectSettings, Worktrees};
use contracts::terminal::SpawnParams as TerminalSpawn;
use contracts::{Actor, ActorKind, EventData, Kind, Status};
use rpc::{Bus, Ctx, Module, OpenError, RpcError, code, params, reply};
use serde_json::Value;
use terminal::Terminals;
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::oneshot;
use tokio::time::{Instant, sleep_until};

pub mod claude_code;
mod decision;
mod name;
mod order;
mod rail;
mod scan;
pub mod worktree;

use claude_code::{ClaudeCode, Launcher, Role};

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

impl Observation {
    /// The word a log line names this Observation by; a Signal by its vendor event.
    fn name(&self) -> &str {
        match self {
            Self::Signal(payload) => claude_code::event_name(payload),
            Self::Title(_) => "title",
            Self::Tick => "tick",
            Self::Stopped => "stop",
            Self::Exit { .. } => "exit",
            Self::Answered => "answer",
            Self::Dismissed => "dismissal",
        }
    }
}

/// Turns one vendor's program into an Agent's Status. It emits `error`, `needs-you`, `working`,
/// `idle` and `done`, never `blocked`: that Kind is the Daemon's, from Todos and Routes.
pub trait AgentAdapter {
    /// Fold one Observation into the Status. `None` means no change.
    fn observe(&mut self, observation: Observation) -> Option<Status>;
}

/// Whether an Agent's Terminal is under Takeover, asked before the Daemon writes into it.
type TakeoverProbe = Arc<dyn Fn(&str) -> bool + Send + Sync>;

/// State the Terminal watchers and the RPC calls share.
struct Shared {
    git: worktree::Git,
    project_dir: PathBuf,
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
    /// H2 to H7: the open permission Decisions, in memory only.
    decisions: decision::Decisions,
    /// Itself, so a Signal's fold can start H12's first Steer on a task of its own.
    me: Weak<Shared>,
    /// H11: how long a Steer waits for its `UserPromptSubmit` Signal.
    steer_bound: Duration,
    /// H13: how long an Interrupt waits for the title to change.
    interrupt_bound: Duration,
    /// E6: the Channel of each Agent's current Attempt that has left `pending`, as
    /// `(attempt, channel)`; an entry of an earlier Attempt reads as `pending`.
    channels: Mutex<HashMap<String, (String, Channel)>>,
    /// O4: whether a Takeover of the Agent is active, which keeps a changed order from being
    /// sent to it. Without one, none is.
    takeover: Option<TakeoverProbe>,
    /// E6: how long after a start `agent.channelUp` may take before the Channel is `missing`.
    channel_deadline: Duration,
    /// F5: the scan of the Doors' process trees.
    scan: Scan,
}

/// F5: what the scan of the Doors' process trees keeps.
struct Scan {
    /// How long between two reads of the process table.
    interval: Duration,
    /// The scan loop starts with the first Door and ends with the Daemon.
    started: AtomicBool,
    /// Each started Door's marker: the path its program's arguments hold.
    doors: Mutex<HashMap<String, String>>,
    /// Each Door's strays as last announced; a Door with none has no entry.
    found: Mutex<HashMap<String, Vec<Stray>>>,
}

impl Scan {
    fn new(interval: Duration) -> Self {
        Self {
            interval,
            started: AtomicBool::new(false),
            doors: Mutex::new(HashMap::new()),
            found: Mutex::new(HashMap::new()),
        }
    }
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

/// A22: how long `agent.resume` waits, on the injected clock, for the new Attempt's
/// acknowledging `SessionStart` before it fails with `CONFLICT` and rolls back.
pub const RESUME_ACK_BOUND: Duration = Duration::from_secs(10);

/// A24: how long `rail.startDoor` waits for that acknowledgement. Below A24's 10 000 ms bound on
/// the whole call, which also covers the rollback that follows the wait.
pub const START_DOOR_ACK_BOUND: Duration = Duration::from_secs(8);

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

/// What `Shared::apply` leaves to do once its locks are let go: the first prompt to Steer at the
/// first `SessionStart` (H12), and when, on `clock`, the adapter wants its next
/// `Observation::Tick`.
type Applied = (Option<String>, Option<i64>);

/// H11: how long a Steer waits for its `UserPromptSubmit` Signal before `NOT_ACCEPTED`.
pub const STEER_BOUND: Duration = Duration::from_secs(10);

/// H13: how long an Interrupt waits for the title to change before `NOT_ACKED`.
pub const INTERRUPT_BOUND: Duration = Duration::from_secs(5);

/// E6: how long after a start the Channel may stay `pending`. `ROUNDUP_CHANNEL_DEADLINE_MS`
/// shortens it for a test.
pub const CHANNEL_DEADLINE: Duration = Duration::from_secs(15);

/// Why a Steer (`Agents::prompt`) did not land (H11).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PromptError {
    /// The Agent is not `idle`, or its last Steer is still waiting to be submitted.
    Busy { id: String, kind: Kind },
    /// The text was written (or could not be) and no `UserPromptSubmit` Signal followed; the
    /// Terminal's input line keeps it and nothing is sent again.
    NotAccepted { id: String },
    /// No Agent `id` is running.
    NotFound { id: String },
}

impl std::fmt::Display for PromptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy { id, kind } => write!(f, "agent {id} is {kind:?}, not idle"),
            Self::NotAccepted { id } => write!(f, "agent {id} did not accept the prompt"),
            Self::NotFound { id } => write!(f, "agent {id} is not running"),
        }
    }
}

impl std::error::Error for PromptError {}

impl From<PromptError> for RpcError {
    fn from(error: PromptError) -> Self {
        let message = error.to_string();
        match error {
            PromptError::Busy { .. } => Self::busy(message),
            PromptError::NotAccepted { .. } => Self::not_accepted(message),
            PromptError::NotFound { .. } => Self::not_found(message),
        }
    }
}

impl Shared {
    /// H12: Steer the first prompt of `id`'s `attempt` on a task of its own, since `agent.spawn`
    /// has returned by now. A Steer that fails settles the Status to `error`, never silently.
    fn steer_first_prompt(&self, id: &str, attempt: &str, prompt: String) {
        let Some(shared) = self.me.upgrade() else {
            return;
        };
        let (id, attempt) = (id.to_owned(), attempt.to_owned());
        tokio::spawn(async move {
            if let Err(err) = shared.prompt(&id, &prompt).await {
                eprintln!("agents: the first prompt of {id} failed: {err}");
                shared.refuse_prompt(&id, &attempt);
            }
        });
    }

    /// H19: when the star has not arrived `steer_bound` after the first `SessionStart`, drop the
    /// first prompt unwritten and settle `id` to `error`, as a Steer that failed does.
    fn refuse_unless_gate_opens(&self, id: &str, attempt: &str) {
        let Some(shared) = self.me.upgrade() else {
            return;
        };
        let (id, attempt) = (id.to_owned(), attempt.to_owned());
        tokio::spawn(async move {
            tokio::time::sleep(shared.steer_bound).await;
            let dropped = match shared.runs().get_mut(&id) {
                Some(Slot::Running(run)) if run.attempt == attempt => run.prompt.take().is_some(),
                _ => false,
            };
            if dropped {
                eprintln!("agents: the first prompt of {id} was never written: no star title");
                shared.refuse_prompt(&id, &attempt);
            }
        });
    }

    /// H12: settle `id` to `error` with the label `prompt not accepted`, if `attempt` is still
    /// its current Attempt.
    fn refuse_prompt(&self, id: &str, attempt: &str) {
        let mut runs = self.runs();
        let Some(Slot::Running(run)) = runs.get_mut(id) else {
            return;
        };
        if run.attempt != attempt {
            return;
        }
        if let Some(status) = run.adapter.refuse_prompt() {
            run.status_revision = run
                .status_revision
                .checked_add(1)
                .expect("Status revision exhausted");
            self.bus.emit(
                Actor::daemon(),
                EventData::AgentStatus(StatusEvent {
                    id: id.to_owned(),
                    attempt: run.attempt.clone(),
                    status_revision: run.status_revision.to_string(),
                    status,
                }),
            );
        }
    }

    /// H11, H16: the one place a prompt is written to a Terminal. `text` goes out once, as one
    /// bracketed paste and `\r`, when `id` is `idle`; this returns when its `UserPromptSubmit`
    /// Signal arrives. Nothing is written again on any failure.
    async fn prompt(&self, id: &str, text: &str) -> Result<(), PromptError> {
        let not_found = || PromptError::NotFound { id: id.to_owned() };
        let (terminal_id, submitted) = {
            let mut runs = self.runs();
            let Some(Slot::Running(run)) = runs.get_mut(id) else {
                return Err(match runs.get(id) {
                    Some(Slot::Starting { .. } | Slot::Resuming { .. }) => PromptError::Busy {
                        id: id.to_owned(),
                        kind: Kind::Working,
                    },
                    _ => not_found(),
                });
            };
            if run.ended || run.closing {
                return Err(not_found());
            }
            let kind = run
                .adapter
                .status()
                .map_or(Kind::Working, |status| status.kind);
            if kind != Kind::Idle || run.steer.as_ref().is_some_and(|steer| !steer.is_closed()) {
                return Err(PromptError::Busy {
                    id: id.to_owned(),
                    kind,
                });
            }
            let (steer, submitted) = oneshot::channel();
            run.steer = Some(steer);
            (run.terminal_id.clone(), submitted)
        };
        let written = self
            .terminals
            .write(&terminal_id, &claude_code::steer_bytes(text))
            .await;
        let lost = match written {
            Err(err) if err.code == code::NOT_FOUND => return Err(not_found()),
            Err(err) => {
                eprintln!("agents: could not write the prompt to terminal {terminal_id}: {err}");
                true
            }
            Ok(()) => tokio::time::timeout(self.steer_bound, submitted)
                .await
                .map_or(true, |signalled| signalled.is_err()),
        };
        if !lost {
            return Ok(());
        }
        let mut runs = self.runs();
        let ended = match runs.get_mut(id) {
            Some(Slot::Running(run)) => {
                run.steer = None;
                run.ended || run.closing
            }
            _ => true,
        };
        Err(if ended {
            not_found()
        } else {
            PromptError::NotAccepted { id: id.to_owned() }
        })
    }

    /// H13, H16: the other place a Terminal is written on an Agent's behalf. One `ESC` goes out
    /// when `id` has a turn to end; this returns when the Agent's title shows it is no longer
    /// working (`Kind::Idle`). Nothing is written again on any failure.
    async fn interrupt(&self, id: &str) -> Result<(), RpcError> {
        let not_found = || RpcError::not_found(format!("agent {id} is not running"));
        let (terminal_id, acked) = {
            let mut runs = self.runs();
            let Some(Slot::Running(run)) = runs.get_mut(id) else {
                return Err(match runs.get(id) {
                    Some(Slot::Starting { .. } | Slot::Resuming { .. }) => RpcError::new(
                        code::NOT_RUNNING,
                        format!("agent {id} has no turn to interrupt yet"),
                    ),
                    _ => not_found(),
                });
            };
            if run.ended || run.closing {
                return Err(not_found());
            }
            let kind = run
                .adapter
                .status()
                .map_or(Kind::Working, |status| status.kind);
            if matches!(kind, Kind::Idle | Kind::Done | Kind::Error) {
                return Err(RpcError::new(
                    code::NOT_RUNNING,
                    format!("agent {id} is {kind:?}, with no turn to interrupt"),
                ));
            }
            if run
                .interrupt
                .as_ref()
                .is_some_and(|waiting| !waiting.is_closed())
            {
                return Err(RpcError::conflict(format!(
                    "agent {id} is already being interrupted"
                )));
            }
            let (waiting, acked) = oneshot::channel();
            run.interrupt = Some(waiting);
            (run.terminal_id.clone(), acked)
        };
        let written = self
            .terminals
            .write(&terminal_id, &claude_code::interrupt_bytes())
            .await;
        let lost = match written {
            Err(err) if err.code == code::NOT_FOUND => return Err(not_found()),
            Err(err) => {
                eprintln!("agents: could not write the interrupt to terminal {terminal_id}: {err}");
                true
            }
            Ok(()) => tokio::time::timeout(self.interrupt_bound, acked)
                .await
                .map_or(true, |signalled| signalled.is_err()),
        };
        if !lost {
            return Ok(());
        }
        let mut runs = self.runs();
        let ended = match runs.get_mut(id) {
            Some(Slot::Running(run)) => {
                run.interrupt = None;
                run.ended || run.closing
            }
            _ => true,
        };
        if ended {
            return Err(not_found());
        }
        eprintln!("agents: {id}: the title did not change after the interrupt");
        Err(RpcError::new(
            code::NOT_ACKED,
            format!("agent {id} did not acknowledge the interrupt"),
        ))
    }

    fn rail(&self) -> MutexGuard<'_, rail::Rail> {
        self.rail.lock().expect("rail lock")
    }

    fn runs(&self) -> MutexGuard<'_, HashMap<String, Slot>> {
        self.runs.lock().expect("runs lock")
    }

    fn mark_starting(&self, id: &str, attempt: String) {
        let since = (self.clock)();
        self.runs().insert(
            id.to_owned(),
            Slot::Starting {
                attempt,
                since,
                named: false,
                held: VecDeque::new(),
            },
        );
    }

    /// An Agent, or a Door, has a Status: its live one; `working` while it starts; a stashed
    /// one while A22's resume is pending acknowledgement (its own `terminal_id` is not presented
    /// until then either, so this leaves `node.terminal_id` as `rail` already had it); else
    /// `done`, because the Daemon that ran it is gone and its Terminal with it. Every node that
    /// leaves the module passes through here.
    fn present(
        &self,
        mut node: RailNode,
        rail: &rail::Rail,
        runs: &HashMap<String, Slot>,
    ) -> RailNode {
        if node.kind != NodeKind::Terminal {
            node.status_revision = match runs.get(&node.id) {
                Some(Slot::Running(run)) => Some(run.status_revision.to_string()),
                Some(Slot::Starting { .. }) => Some("1".into()),
                Some(Slot::Resuming { old, .. }) => {
                    old.as_ref().map(|run| run.status_revision.to_string())
                }
                _ => None,
            };
            let live = match runs.get(&node.id) {
                Some(Slot::Running(run)) => run.adapter.status().cloned(),
                Some(Slot::Starting { since, .. }) => Some(Status {
                    kind: Kind::Working,
                    label: "starting".into(),
                    since: *since,
                }),
                Some(Slot::Resuming { old, .. }) => {
                    old.as_ref().and_then(|run| run.adapter.status().cloned())
                }
                None | Some(Slot::Closing) => None,
            };
            node.status = Some(live.unwrap_or_else(|| Status {
                kind: Kind::Done,
                label: "terminal gone".into(),
                since: self.opened,
            }));
            // A22: exited (no live process, and no start in flight), with A21 data to resume.
            let exited = match runs.get(&node.id) {
                Some(Slot::Running(run)) => run.ended,
                Some(Slot::Starting { .. } | Slot::Resuming { .. } | Slot::Closing) => false,
                None => true,
            };
            node.can_resume = exited && rail.has_conversation(&node.id).unwrap_or(false);
            let channels = self.channels.lock().expect("channels lock");
            node.channel = channel_in(&channels, &node.id, runs.get(&node.id));
            node.stray = self
                .scan
                .found
                .lock()
                .expect("scan found lock")
                .get(&node.id)
                .cloned()
                .unwrap_or_default();
        }
        node
    }

    /// E6: `agent.channelUp` from Agent `id`. Nothing to report when no program of the Agent
    /// runs, and a second call while the Channel is `up` emits nothing.
    fn channel_up(&self, actor: &Actor, id: &str) -> Result<(), RpcError> {
        let node = self.rail().node(id)?;
        if node.kind == NodeKind::Terminal {
            return Err(RpcError::not_found(format!("agent {id}")));
        }
        if actor.kind != ActorKind::Agent || actor.id != id {
            return Err(RpcError::forbidden(format!(
                "only agent {id} may report its Channel"
            )));
        }
        let attempt = self.runs().get(id).map(|slot| slot.attempt().to_owned());
        if let Some(attempt) = attempt {
            self.set_channel(actor.clone(), id, &attempt, Channel::Up);
        }
        Ok(())
    }

    /// E6: move the Channel of `attempt` to `channel` and say so, unless it is there already or
    /// the Attempt is no longer the live one.
    fn set_channel(&self, by: Actor, id: &str, attempt: &str, channel: Channel) {
        let changed = {
            let runs = self.runs();
            let slot = runs.get(id).filter(|slot| slot.attempt() == attempt);
            let mut channels = self.channels.lock().expect("channels lock");
            let current = channel_in(&channels, id, slot);
            // `up` is final for an Attempt; `missing` never replaces it.
            let moves = match (current, channel) {
                (None, _) | (Some(Channel::Up), _) => false,
                (Some(now), next) => now != next,
            };
            if moves {
                channels.insert(id.to_owned(), (attempt.to_owned(), channel));
            }
            moves
        };
        if changed {
            self.bus.emit(
                by,
                EventData::AgentChannel(ChannelEvent {
                    id: id.to_owned(),
                    channel,
                }),
            );
        }
    }

    /// E6: after `channel_deadline`, an Attempt still `pending` is `missing`.
    fn watch_channel(self: &Arc<Self>, id: &str, attempt: &str) {
        let (shared, id, attempt) = (Arc::clone(self), id.to_owned(), attempt.to_owned());
        tokio::spawn(async move {
            tokio::time::sleep(shared.channel_deadline).await;
            shared.set_channel(Actor::daemon(), &id, &attempt, Channel::Missing);
        });
    }

    /// F5: scan Door `id`, whose program's arguments hold `marker`, from the next tick on.
    fn scan_door(self: &Arc<Self>, id: &str, marker: String) {
        self.scan
            .doors
            .lock()
            .expect("scan doors lock")
            .insert(id.to_owned(), marker);
        if self.scan.started.swap(true, Ordering::SeqCst) {
            return;
        }
        let (me, interval) = (self.me.clone(), self.scan.interval);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                let Some(shared) = me.upgrade() else { break };
                shared.scan_once().await;
            }
        });
    }

    /// F5: read the process table once if a Door runs, and announce each Door whose strays changed.
    async fn scan_once(&self) {
        let live: Vec<(String, String)> = {
            let runs = self.runs();
            let mut doors = self.scan.doors.lock().expect("scan doors lock");
            doors.retain(|id, _| {
                matches!(runs.get(id), Some(Slot::Running(run)) if !run.ended && !run.closing)
            });
            doors
                .iter()
                .map(|(id, marker)| (id.clone(), marker.clone()))
                .collect()
        };
        let table = if live.is_empty() {
            Vec::new()
        } else {
            match tokio::task::spawn_blocking(scan::read_table).await {
                Ok(Ok(table)) => scan::parse(&table),
                Ok(Err(err)) => return eprintln!("agents: cannot read the process table: {err}"),
                Err(err) => return eprintln!("agents: the process scan stopped: {err}"),
            }
        };
        let programs = claude_code::programs();
        let mut now: HashMap<String, Vec<Stray>> = live
            .iter()
            .map(|(id, marker)| (id.clone(), scan::strays(&table, marker, programs)))
            .collect();
        let changed: Vec<(String, Vec<Stray>)> = {
            let mut found = self.scan.found.lock().expect("scan found lock");
            let gone: Vec<String> = found
                .keys()
                .filter(|id| !now.contains_key(*id))
                .cloned()
                .collect();
            now.extend(gone.into_iter().map(|id| (id, Vec::new())));
            let mut changed = Vec::new();
            for (id, stray) in now {
                if found.get(&id).map_or(&[][..], Vec::as_slice) == stray.as_slice() {
                    continue;
                }
                if stray.is_empty() {
                    found.remove(&id);
                } else {
                    found.insert(id.clone(), stray.clone());
                }
                changed.push((id, stray));
            }
            changed
        };
        for (id, stray) in changed {
            self.bus.emit(
                Actor::daemon(),
                EventData::AgentStray(StrayEvent { id, stray }),
            );
        }
    }

    fn node(&self, id: &str) -> Result<RailNode, RpcError> {
        let rail = self.rail();
        let runs = self.runs();
        Ok(self.present(rail.node(id)?, &rail, &runs))
    }

    fn tree(&self) -> Result<Vec<RailNode>, RpcError> {
        let rail = self.rail();
        let runs = self.runs();
        Ok(rail
            .tree()?
            .into_iter()
            .map(|node| self.present(node, &rail, &runs))
            .collect())
    }

    /// Fold `observation` into Agent `id`'s Status. A change is announced as `agent.status`, and
    /// the first `SessionStart` Steers the prompt the Agent was spawned with. Returns when, on `clock`, the
    /// adapter wants its next `Observation::Tick`.
    fn observe(
        &self,
        actor: Actor,
        id: &str,
        attempt: &str,
        observation: Observation,
    ) -> Result<Option<i64>, RpcError> {
        let first_prompt = match &observation {
            Observation::Signal(payload) => {
                claude_code::submitted_prompt(payload).map(name::from_prompt)
            }
            _ => None,
        };
        let names_conversation = matches!(&observation, Observation::Signal(payload)
            if claude_code::conversation_id(payload).is_some());
        // Taken before `runs`, as every other path does, so a rename and a prompt cannot deadlock.
        let mut rail = (first_prompt.is_some()
            || names_conversation
            || matches!(observation, Observation::Exit { .. }))
        .then(|| self.rail());
        let (prompt, tick_at) = {
            let mut runs = self.runs();
            // A20: an Observation from an earlier Attempt is ignored, never an error to its sender.
            if runs.get(id).is_some_and(|slot| slot.attempt() != attempt) {
                eprintln!(
                    "agents: {id}: ignored {} from earlier Attempt {attempt}",
                    observation.name()
                );
                return Ok(None);
            }
            if runs.get(id).is_some_and(Slot::closing) {
                return Err(RpcError::conflict(format!("agent {id} is stopping")));
            }
            match runs.get_mut(id) {
                Some(Slot::Running(run)) => {
                    if run.ended {
                        return Err(RpcError::not_found(format!("agent {id} has ended")));
                    }
                    // A21: a Signal naming a conversation of the current Attempt saves it
                    // before being applied; a failed save fails this call, settles the Status
                    // to `error` and stops the Terminal, since `agent.spawn` has already
                    // returned by the time a Running Agent can reach here (the held window's
                    // own save is `finish_starting`'s; A22's resuming is `finish_resuming`'s).
                    if let Observation::Signal(payload) = &observation
                        && let Some(rail) = rail.as_deref_mut()
                        && let Err(err) = save_signal_conversation(rail, run, id, payload)
                    {
                        if let Some(status) =
                            run.adapter.fail(format!("cannot save conversation: {err}"))
                        {
                            run.status_revision = run
                                .status_revision
                                .checked_add(1)
                                .expect("Status revision exhausted");
                            self.bus.emit(
                                actor.clone(),
                                EventData::AgentStatus(StatusEvent {
                                    id: id.to_owned(),
                                    attempt: run.attempt.clone(),
                                    status_revision: run.status_revision.to_string(),
                                    status,
                                }),
                            );
                        }
                        self.spawn_kill(run.terminal_id.clone());
                        return Err(err);
                    }
                    self.apply(
                        rail.as_deref_mut(),
                        run,
                        id,
                        &actor,
                        observation,
                        first_prompt,
                    )?
                }
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
                // A22: a non-ack Signal is held exactly as `Starting`'s; the ack promotes to
                // `Running` and wakes the pending `agent.resume` (`finish_resuming`).
                Some(Slot::Resuming { .. }) => {
                    let Observation::Signal(payload) = observation else {
                        return Err(RpcError::not_found(format!("agent {id}")));
                    };
                    self.finish_resuming(rail.as_deref_mut(), &mut runs, id, actor, payload)?
                }
                None => return Err(RpcError::not_found(format!("agent {id}"))),
                Some(Slot::Closing) => {
                    return Err(RpcError::conflict(format!("agent {id} is stopping")));
                }
            }
        };
        if let Some(prompt) = prompt {
            self.steer_first_prompt(id, attempt, prompt);
        }
        Ok(tick_at)
    }

    /// Fold `observation` into `run`, which `observe` has already found `Running`; `rail`, when
    /// given, is locked by the caller, never here, so a caller draining several Signals under
    /// one `rail`-then-`runs` lock section (`finish_starting`) is never asked to lock it twice.
    fn apply(
        &self,
        mut rail: Option<&mut rail::Rail>,
        run: &mut Run,
        id: &str,
        actor: &Actor,
        observation: Observation,
        first_prompt: Option<Option<String>>,
    ) -> Result<Applied, RpcError> {
        let exited = matches!(observation, Observation::Exit { .. });
        match &observation {
            // H6: an Agent that ends takes its Decision with it.
            Observation::Exit { .. } => {
                self.decisions
                    .clear(&self.bus, id, Outcome::AgentGone, "the Agent ended");
            }
            Observation::Stopped => {
                self.decisions
                    .clear(&self.bus, id, Outcome::AgentGone, "the Agent ended");
            }
            // H7(a): the tool ran or the prompt moved on, so the dialog was answered in the
            // Terminal.
            Observation::Signal(payload) if claude_code::ends_decision(payload) => {
                self.decisions
                    .clear(&self.bus, id, Outcome::Terminal, "answered in the Terminal");
            }
            _ => {}
        }
        // H19: the first Steer waits for the first `SessionStart` and the first star title.
        if matches!(&observation, Observation::Signal(payload)
            if claude_code::is_session_start(payload))
        {
            if !run.session_started && run.prompt.is_some() {
                self.refuse_unless_gate_opens(id, &run.attempt);
            }
            run.session_started = true;
        }
        if matches!(&observation, Observation::Title(title) if title.starts_with('✳')) {
            run.starred = true;
        }
        // H11: the Steer's wait ends with the Signal that names it, or with the Agent.
        if matches!(&observation, Observation::Signal(payload)
            if claude_code::submitted_prompt(payload).is_some())
        {
            if let Some(steer) = run.steer.take() {
                let _ = steer.send(());
            }
        } else if matches!(observation, Observation::Exit { .. } | Observation::Stopped) {
            run.steer = None;
        }
        // H13: the Interrupt's wait ends with the title (or the Dismissal of its dialog) that
        // leaves the Agent idle, or with the Agent.
        let acks = matches!(
            observation,
            Observation::Title(_) | Observation::Tick | Observation::Dismissed
        );
        if matches!(observation, Observation::Exit { .. } | Observation::Stopped) {
            run.interrupt = None;
        }
        let changed = run.adapter.observe(observation);
        if acks
            && changed
                .as_ref()
                .is_some_and(|status| status.kind == Kind::Idle)
            && let Some(waiting) = run.interrupt.take()
        {
            let _ = waiting.send(());
        }
        if exited {
            run.ended = true;
            if let Some(rail) = rail.as_deref_mut() {
                rail.detach_terminal(id)?;
            }
        }
        // Announced while `runs` is held, so announcements leave in the order the Status
        // changed; `emit` never blocks.
        if let Some(status) = changed {
            run.status_revision = run
                .status_revision
                .checked_add(1)
                .expect("Status revision exhausted");
            let event = StatusEvent {
                id: id.to_owned(),
                attempt: run.attempt.clone(),
                status_revision: run.status_revision.to_string(),
                status,
            };
            self.bus.emit(actor.clone(), EventData::AgentStatus(event));
        }
        if let (Some(rail), Some(name)) = (rail, first_prompt)
            && !std::mem::replace(&mut run.named, true)
            && let Some(name) = name
            && rail.node(id)?.kind == NodeKind::Agent
        {
            let parent = rail.node(id)?.parent;
            let siblings = rail.tree()?;
            let taken = siblings
                .iter()
                .filter(|node| node.id != id && node.parent == parent)
                .map(|node| node.name.as_str());
            rail.rename(id, &name::unique(&name, taken))?;
            self.bus.emit(actor.clone(), EventData::RailChanged);
        }
        let prompt = run.prompt.take_if(|_| run.session_started && run.starred);
        Ok((prompt, run.adapter.tick_at()))
    }

    /// Swap `id` from `Starting` to `Running` (the Run `build` makes, told whether a
    /// `rail.rename` already settled its name) and apply every Signal held for it, in arrival
    /// order, all under one `rail`-then-`runs` lock section: a Signal that arrives once the
    /// Agent is registered can never be applied ahead of one still held for it (A14), and
    /// `agent.signal` never finds `id` missing between the two (it is never `NOT_FOUND` for an
    /// id that was spawned). Returns the first prompt, if an applied `SessionStart` released it,
    /// to Steer once the locks are let go.
    fn finish_starting(
        &self,
        id: &str,
        attempt: &str,
        build: impl FnOnce(bool) -> Run,
    ) -> Result<Vec<String>, RpcError> {
        let mut rail = self.rail();
        let mut runs = self.runs();
        if !matches!(runs.get(id), Some(Slot::Starting { attempt: active, .. }) if active == attempt)
        {
            return Err(RpcError::conflict(format!(
                "agent {id}: stale start completion"
            )));
        }
        let Some(Slot::Starting { named, held, .. }) = runs.remove(id) else {
            unreachable!("spawn marks {id} Starting before run_agent runs")
        };
        let mut run = build(named);
        let mut prompts = Vec::new();
        for (actor, payload) in held {
            // A21: a held Signal naming a conversation is saved before it is applied, same as a
            // Running Agent's; here, before `agent.spawn` has returned, a failed save fails the
            // whole spawn instead, as any other failed spawn does (A14), so the Terminal
            // `run_agent`'s own cleanup kills is this one, never left running.
            if let Err(err) = save_signal_conversation(&mut rail, &run, id, &payload) {
                // Put `id` back so the caller's own failed-spawn cleanup (which expects to find
                // its Attempt still in `runs`) still removes the orphaned node and kills the
                // Terminal `start` already recorded, exactly as any other failed spawn.
                runs.insert(id.to_owned(), Slot::Running(run));
                return Err(err);
            }
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
        Ok(prompts)
    }

    /// A22: `id`'s slot is `Resuming` and `payload` is a Signal (`observe`'s own match already
    /// checked both). A non-ack Signal is held exactly as `Starting`'s (A14); so is an ack that
    /// arrives before `agent.resume` has recorded the Terminal it started (a race only an
    /// implausibly fast hook could win, but one `promote_resuming` must never be asked to attach
    /// a Terminal id it does not have): `promote_held_resume_ack` is what notices, once that id
    /// is recorded, that the wait is already over. Otherwise the ack — a `SessionStart` naming
    /// source `resume` and the saved conversation id — promotes it now.
    fn finish_resuming(
        &self,
        rail: Option<&mut rail::Rail>,
        runs: &mut HashMap<String, Slot>,
        id: &str,
        actor: Actor,
        payload: Value,
    ) -> Result<Applied, RpcError> {
        let Some(Slot::Resuming {
            attempt,
            since,
            old,
            conversation_id,
            cwd,
            terminal_id,
            mut held,
            done,
        }) = runs.remove(id)
        else {
            unreachable!("observe matched a Resuming slot for {id}");
        };
        if !is_resume_ack(&payload, &conversation_id) || terminal_id.is_none() {
            hold(id, &mut held, actor, payload);
            runs.insert(
                id.to_owned(),
                Slot::Resuming {
                    attempt,
                    since,
                    old,
                    conversation_id,
                    cwd,
                    terminal_id,
                    held,
                    done,
                },
            );
            return Ok((None, None));
        }
        let terminal_id = terminal_id.expect("checked above");
        let rail = rail.expect("rail locked for a conversation Signal");
        self.promote_resuming(
            rail,
            runs,
            id,
            attempt,
            old,
            cwd,
            terminal_id,
            held,
            done,
            actor,
            payload,
        )
    }

    /// A22: once a pending resume's Terminal id is recorded (`agent.resume`'s own `claimed`
    /// step), promote it at once if its ack arrived and was held only because that id was not
    /// yet known — the one case `finish_resuming` cannot act on by itself — instead of leaving it
    /// held until the 10 s timeout rolls a resume back that in fact already succeeded.
    fn promote_held_resume_ack(&self, id: &str) {
        let mut rail = self.rail();
        let mut runs = self.runs();
        let found = match runs.get(id) {
            Some(Slot::Resuming {
                conversation_id,
                held,
                ..
            }) => held
                .iter()
                .position(|(_, payload)| is_resume_ack(payload, conversation_id)),
            _ => None,
        };
        let Some(position) = found else { return };
        let Some(Slot::Resuming {
            attempt,
            old,
            cwd,
            terminal_id,
            mut held,
            done,
            ..
        }) = runs.remove(id)
        else {
            unreachable!("checked above, under the same lock")
        };
        let (ack_actor, ack_payload) = held
            .remove(position)
            .expect("position came from this held, under the same lock");
        let terminal_id = terminal_id.expect("promote_held_resume_ack only runs once it is Some");
        if let Err(err) = self.promote_resuming(
            &mut rail,
            &mut runs,
            id,
            attempt,
            old,
            cwd,
            terminal_id,
            held,
            done,
            ack_actor,
            ack_payload,
        ) {
            eprintln!("agents: could not promote {id}'s already-held resume ack: {err}");
        }
    }

    /// A22's ack confirmed for `id`: attach the Terminal `agent.resume` started (never before
    /// this moment), re-save the conversation (A21), apply every earlier held Signal in arrival
    /// order and then the ack itself, and wake the pending `agent.resume`. On failure, restore
    /// `old` and stop the Terminal, same as a failed launch.
    #[allow(clippy::too_many_arguments)]
    fn promote_resuming(
        &self,
        rail: &mut rail::Rail,
        runs: &mut HashMap<String, Slot>,
        id: &str,
        attempt: String,
        old: Option<Box<Run>>,
        cwd: String,
        terminal_id: String,
        held: VecDeque<(Actor, Value)>,
        done: oneshot::Sender<Result<(), RpcError>>,
        actor: Actor,
        payload: Value,
    ) -> Result<Applied, RpcError> {
        if let Err(err) = rail.attach_terminal(id, &terminal_id) {
            if let Some(old) = old {
                runs.insert(id.to_owned(), Slot::Running(*old));
            }
            let _ = done.send(Err(RpcError::conflict(format!("cannot resume: {err}"))));
            self.spawn_kill(terminal_id);
            return Err(err);
        }
        let clock = Arc::clone(&self.clock);
        let mut run = Run {
            attempt,
            status_revision: 1,
            closing: false,
            ended: false,
            adapter: ClaudeCode::starting(move || clock()),
            prompt: None,
            steer: None,
            session_started: false,
            starred: false,
            interrupt: None,
            named: true,
            terminal_id: terminal_id.clone(),
            cwd,
        };
        // A22 never retypes a prompt (`run.prompt` is always `None`), so unlike `finish_starting`
        // a held Signal here can never produce one to type; only logged on failure.
        for (held_actor, held_payload) in held {
            let first_prompt = claude_code::submitted_prompt(&held_payload).map(name::from_prompt);
            if let Err(err) = self.apply(
                Some(rail),
                &mut run,
                id,
                &held_actor,
                Observation::Signal(held_payload),
                first_prompt,
            ) {
                eprintln!("agents: could not apply a Signal held for {id} while it resumed: {err}");
            }
        }
        if let Err(err) = save_signal_conversation(rail, &run, id, &payload) {
            let _ = rail.detach_terminal(id);
            if let Some(old) = old {
                runs.insert(id.to_owned(), Slot::Running(*old));
            }
            let _ = done.send(Err(RpcError::conflict(format!("cannot resume: {err}"))));
            self.spawn_kill(terminal_id);
            return Err(err);
        }
        let applied = self.apply(
            Some(rail),
            &mut run,
            id,
            &actor,
            Observation::Signal(payload),
            None,
        );
        runs.insert(id.to_owned(), Slot::Running(run));
        let _ = done.send(Ok(()));
        applied
    }

    fn release_closing(&self, id: &str, ended: bool) {
        let mut runs = self.runs();
        if let Some(Slot::Running(run)) = runs.get_mut(id) {
            run.closing = false;
            run.ended |= ended;
        } else {
            runs.remove(id);
        }
    }

    /// A21: stop a Terminal in the background, for a failure that must not block the caller
    /// (`observe`'s own `Err` return) and has no actor to attribute a `rail.changed` to.
    fn spawn_kill(&self, terminal_id: String) {
        let kill = Arc::clone(&self.kill);
        let terminals = Arc::clone(&self.terminals);
        tokio::spawn(async move {
            if let Err(err) = kill_or_already_gone(&*kill, &terminals, &terminal_id).await {
                eprintln!(
                    "agents: could not stop terminal {terminal_id} after a conversation save \
                     failure: {err}"
                );
            }
        });
    }

    /// A22: give up on `id`'s pending resume of Attempt `attempt`, if it is still the one
    /// pending (the ack may have already won the race under this same lock, in which case this
    /// is a no-op: whichever of the two claims the slot first decides the outcome). Stops the
    /// Terminal it started, if it had, and restores the stashed old Run, or removes the id from
    /// `runs` entirely when there was none to restore (A12's reopened case).
    async fn rollback_resume(&self, id: &str, attempt: &str) {
        let claimed = {
            let mut runs = self.runs();
            match runs.get(id) {
                Some(Slot::Resuming {
                    attempt: active, ..
                }) if active == attempt => runs.remove(id),
                _ => None,
            }
        };
        let Some(Slot::Resuming {
            old, terminal_id, ..
        }) = claimed
        else {
            return;
        };
        if let Some(terminal_id) = terminal_id {
            report_undo(
                "stop a resume's Terminal on rollback",
                kill_or_already_gone(&*self.kill, &self.terminals, &terminal_id).await,
            );
        }
        let mut runs = self.runs();
        match old {
            Some(run) => {
                runs.insert(id.to_owned(), Slot::Running(*run));
            }
            None => {
                runs.remove(id);
            }
        }
        drop(runs);
        self.bus.emit(Actor::daemon(), EventData::RailChanged);
    }

    async fn stop(&self, actor: Actor, id: &str) -> Result<(), RpcError> {
        self.end(actor, id, false).await
    }

    async fn remove(&self, actor: Actor, id: &str) -> Result<(), RpcError> {
        self.end(actor, id, true).await
    }

    /// G5: stop the Agent and delete its node, Worktree directory and branch whatever their
    /// state.
    async fn discard(&self, actor: Actor, id: &str) -> Result<(), RpcError> {
        if self.rail().node(id)?.worktree.is_none() {
            return Err(RpcError::new(code::NOT_FOUND, format!("no_worktree: {id}")));
        }
        self.end_with(actor, id, true, true).await
    }

    async fn end(&self, actor: Actor, id: &str, remove: bool) -> Result<(), RpcError> {
        self.end_with(actor, id, remove, false).await
    }

    /// `discarding` drops the Worktree whatever its state; otherwise a remove keeps unlanded work.
    async fn end_with(
        &self,
        actor: Actor,
        id: &str,
        remove: bool,
        discarding: bool,
    ) -> Result<(), RpcError> {
        let checked_node = if remove && !discarding {
            Some(self.rail().node(id)?)
        } else {
            None
        };
        if let Some(worktree) = checked_node
            .as_ref()
            .and_then(|node| node.worktree.as_ref())
        {
            let git = self.git.clone();
            let project = self.project_dir.clone();
            let worktree = worktree::Worktree::from(worktree);
            tokio::task::spawn_blocking(move || git.require_landed(&project, &worktree))
                .await
                .map_err(RpcError::internal)??;
        }
        let (terminal_id, worktree) = {
            let rail = self.rail();
            let mut node = rail.node(id)?;
            if checked_node.as_ref().is_some_and(|checked| {
                checked.attempt != node.attempt || checked.worktree != node.worktree
            }) {
                return Err(RpcError::conflict(format!(
                    "{id} changed during Worktree precheck"
                )));
            }
            if !remove && node.kind == NodeKind::Terminal {
                return Err(RpcError::conflict(format!("{id} is not an Agent")));
            }
            let mut runs = self.runs();
            if !remove
                && node.terminal_id.is_none()
                && runs
                    .get(id)
                    .is_none_or(|s| matches!(s, Slot::Running(r) if r.ended))
            {
                return Ok(());
            }
            match runs.get_mut(id) {
                Some(Slot::Starting { .. }) => {
                    return Err(RpcError::conflict(format!("{id} is still starting")));
                }
                // A22: whichever of a resume, a stop or a remove claims the id first wins.
                Some(Slot::Resuming { .. }) => {
                    return Err(RpcError::conflict(format!("{id} is resuming")));
                }
                Some(Slot::Running(run)) => {
                    if run.closing {
                        return Err(RpcError::conflict(format!("{id} is stopping")));
                    }
                    run.closing = true;
                    node.terminal_id = Some(run.terminal_id.clone());
                }
                None => {
                    runs.insert(id.to_owned(), Slot::Closing);
                }
                Some(Slot::Closing) => {
                    return Err(RpcError::conflict(format!("{id} is being removed")));
                }
            }
            (node.terminal_id, remove.then_some(node.worktree).flatten())
        };
        if let Some(Slot::Running(run)) = self.runs().get_mut(id) {
            self.apply(None, run, id, &actor, Observation::Stopped, None)?;
        }
        let killed = match terminal_id {
            Some(terminal_id) => {
                kill_or_already_gone(&*self.kill, &self.terminals, &terminal_id).await
            }
            None => Ok(()),
        };
        if killed.is_ok()
            && let Some(worktree) = worktree
        {
            let git = self.git.clone();
            let project = self.project_dir.clone();
            let worktree = worktree::Worktree::from(&worktree);
            let removed = tokio::task::spawn_blocking(move || {
                if discarding {
                    git.discard(&project, &worktree)
                } else {
                    git.remove_landed(&project, &worktree)
                }
            })
            .await
            .map_err(RpcError::internal)?;
            if let Err(err) = removed {
                self.rail().detach_terminal(id)?;
                self.release_closing(id, true);
                self.bus.emit(actor, EventData::RailChanged);
                return Err(err);
            }
        }
        let mut rail = self.rail();
        let mut runs = self.runs();
        if let Err(err) = killed {
            if let Some(Slot::Running(run)) = runs.get_mut(id) {
                run.closing = false;
            } else {
                runs.remove(id);
            }
            return Err(err);
        }
        let result = if remove {
            rail.remove(id)
        } else {
            rail.detach_terminal(id)
        };
        if remove {
            runs.remove(id);
        } else if let Some(Slot::Running(run)) = runs.get_mut(id) {
            run.closing = false;
            run.ended = true;
        } else {
            runs.remove(id);
        }
        result?;
        self.bus.emit(actor, EventData::RailChanged);
        Ok(())
    }
}

/// E6: the Channel of the live Attempt in `slot`; `None` when no program runs (a Workstream whose Door
/// never started, an Agent with no live Terminal). An entry of an earlier Attempt reads `pending`.
fn channel_in(
    channels: &HashMap<String, (String, Channel)>,
    id: &str,
    slot: Option<&Slot>,
) -> Option<Channel> {
    let slot = slot?;
    let live = match slot {
        Slot::Running(run) => !run.ended && !run.closing,
        Slot::Starting { .. } | Slot::Resuming { .. } => true,
        Slot::Closing => false,
    };
    live.then(|| match channels.get(id) {
        Some((attempt, channel)) if attempt == slot.attempt() => *channel,
        _ => Channel::Pending,
    })
}

/// E6: `ROUNDUP_CHANNEL_DEADLINE_MS` when it is a whole number of milliseconds, else
/// `CHANNEL_DEADLINE`.
fn channel_deadline_from_env() -> Duration {
    match std::env::var("ROUNDUP_CHANNEL_DEADLINE_MS") {
        Ok(ms) => ms.parse().map(Duration::from_millis).unwrap_or_else(|_| {
            eprintln!("agents: ROUNDUP_CHANNEL_DEADLINE_MS={ms:?} is not milliseconds; ignored");
            CHANNEL_DEADLINE
        }),
        Err(_) => CHANNEL_DEADLINE,
    }
}

/// F5: how long between two reads of the process table. `ROUNDUP_SCAN_MS` shortens it for a test.
pub const SCAN_INTERVAL: Duration = Duration::from_millis(2000);

/// F5: `ROUNDUP_SCAN_MS` when it is a positive whole number of milliseconds, else `SCAN_INTERVAL`.
fn scan_interval_from_env() -> Duration {
    match std::env::var("ROUNDUP_SCAN_MS") {
        Ok(ms) => match ms.parse::<u64>() {
            Ok(ms) if ms > 0 => Duration::from_millis(ms),
            _ => {
                eprintln!("agents: ROUNDUP_SCAN_MS={ms:?} is not positive milliseconds; ignored");
                SCAN_INTERVAL
            }
        },
        Err(_) => SCAN_INTERVAL,
    }
}

/// A node's place in `Shared::runs`.
enum Slot {
    /// Its Terminal is starting; `since` is when, on `clock`, it was marked.
    Starting {
        attempt: String,
        since: i64,
        /// A `rail.rename` came while it started; its first prompt must not undo it.
        named: bool,
        /// Signals that reached `agent.signal` in this window, in arrival order, applied once
        /// the Agent is registered (A14).
        held: VecDeque<(Actor, Value)>,
    },
    /// A22: a resume's launch is pending acknowledgement; `since` is when, on `clock`, it began.
    Resuming {
        attempt: String,
        since: i64,
        /// The stashed exited Run, presented and restored on rollback; `None` when there was
        /// nothing to restore (a reopened Agent with no live Run, A12).
        old: Option<Box<Run>>,
        /// The saved conversation id the ack must name.
        conversation_id: String,
        /// The directory the new Attempt runs in, saved alongside the conversation id (A21)
        /// once the ack confirms it.
        cwd: String,
        /// The Terminal this launch started, once `agent.resume` has spawned it; `None` until
        /// then, so a Signal cannot arrive for it any earlier.
        terminal_id: Option<String>,
        /// Signals held exactly as `Starting`'s, awaiting the ack among them (A14).
        held: VecDeque<(Actor, Value)>,
        /// Signalled once by whichever of the ack or the 10 s timeout resolves this resume
        /// first; the loser finds the slot already gone and does nothing (A22's serialization).
        done: tokio::sync::oneshot::Sender<Result<(), RpcError>>,
    },
    Running(Run),
    Closing,
}

impl Slot {
    fn attempt(&self) -> &str {
        match self {
            Self::Starting { attempt, .. } | Self::Resuming { attempt, .. } => attempt,
            Self::Running(run) => &run.attempt,
            Self::Closing => "",
        }
    }
    fn closing(&self) -> bool {
        matches!(self, Self::Closing) || matches!(self, Self::Running(run) if run.closing)
    }

    /// The node's name is final: no prompt renames it.
    fn settle_name(&mut self) {
        match self {
            Self::Running(run) => run.named = true,
            Self::Starting { named, .. } => *named = true,
            Self::Resuming { .. } | Self::Closing => {}
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

/// A21: if `payload` is a Signal naming a conversation, save it and `run`'s effective cwd,
/// replacing any earlier save for `id`. A payload naming none, or one that fails UUID
/// validation, is a no-op: it never names a conversation at all.
fn save_signal_conversation(
    rail: &mut rail::Rail,
    run: &Run,
    id: &str,
    payload: &Value,
) -> Result<(), RpcError> {
    let Some(conversation_id) = claude_code::conversation_id(payload) else {
        return Ok(());
    };
    rail.save_conversation(id, &conversation_id, &run.cwd)
}

/// A22: whether `payload` is the acknowledgement a pending resume waits for: a `SessionStart`
/// that began with `--resume` and names the saved conversation id.
fn is_resume_ack(payload: &Value, conversation_id: &str) -> bool {
    claude_code::session_source(payload) == Some("resume")
        && claude_code::conversation_id(payload).as_deref() == Some(conversation_id)
}

/// An Agent's program and what is left to tell it.
struct Run {
    attempt: String,
    status_revision: i64,
    closing: bool,
    ended: bool,
    adapter: ClaudeCode,
    /// H12: sent as a Steer at the first `SessionStart`, then gone.
    prompt: Option<String>,
    /// H19: the first `SessionStart` Signal has arrived.
    session_started: bool,
    /// H19: the first title with the star glyph has arrived.
    starred: bool,
    /// H11: the Steer written and awaiting its `UserPromptSubmit` Signal.
    steer: Option<tokio::sync::oneshot::Sender<()>>,
    /// H13: the Interrupt written and awaiting the title that acknowledges it.
    interrupt: Option<tokio::sync::oneshot::Sender<()>>,
    /// The first prompt has been submitted, or a `rail.rename` came first: either way the name
    /// is settled and no prompt renames the node.
    named: bool,
    terminal_id: String,
    /// A21: the effective working directory this launch runs in, after Worktree mapping; saved
    /// to `agents.db` alongside the conversation id the first Signal to name one carries.
    cwd: String,
}

/// `spawn` registers an Agent before its watcher starts, so the watcher always finds it.
const REGISTERED: &str = "a watched Agent is registered";
/// G6: a legacy Worktree record from before ownership proofs; nothing may clean it up.
const UNPROVEN: &str = "unfinished legacy provisioning has no ownership proof";

/// Feed one Terminal's titles and its exit to the Agent behind it, and a Tick at the time its
/// adapter asks for one. Only a held star asks, so the watcher of an idle Agent never wakes.
async fn watch(shared: Arc<Shared>, id: String, attempt: String, mut events: Receiver<EventData>) {
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
        let tick_at = match shared.observe(Actor::daemon(), &id, &attempt, observation) {
            Ok(tick_at) => tick_at,
            // `rail.remove` evicts `id` from `runs` once it deletes the node (A16); an Exit
            // already in flight when that happened has nothing left to watch.
            Err(err) if matches!(err.code, code::NOT_FOUND | code::CONFLICT) => return,
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
        let project = dir.parent().unwrap_or(dir);
        let mut rail = rail::Rail::open(&dir.join("agents.db"))?;
        for (node, plan, ready) in rail.provisioning()? {
            let Some(plan) = plan else {
                eprintln!("agents: {}: {UNPROVEN}; kept", node.id);
                continue;
            };
            let settled = if ready {
                git.finish(project, &plan)
                    .and_then(|()| rail.clear_provisioning_owner(&node.id))
            } else {
                git.recover(project, &plan).and_then(|()| {
                    Launcher::discard(dir, &node.id).map_err(RpcError::internal)?;
                    if node.kind == NodeKind::Agent {
                        rail.remove(&node.id)
                    } else {
                        rail.clear_worktree(&node.id)
                    }
                })
            };
            // G6: a record recovery refuses keeps its work and its row; the Project still opens.
            if let Err(error) = settled {
                eprintln!("agents: {}: {}; kept", node.id, error.message);
            }
        }
        Ok(Self {
            shared: Arc::new_cyclic(|me| Shared {
                git,
                project_dir: dir.parent().unwrap_or(dir).to_owned(),
                rail: Mutex::new(rail),
                runs: Mutex::new(HashMap::new()),
                bus,
                kill: Arc::clone(&terminals) as Arc<dyn KillsTerminals>,
                terminals,
                clock: Arc::new(now_ms),
                opened: now_ms(),
                decisions: decision::Decisions::new(
                    None,
                    Duration::from_secs(claude_code::PERMISSION_TIMEOUT_SECS),
                ),
                me: me.clone(),
                steer_bound: STEER_BOUND,
                interrupt_bound: INTERRUPT_BOUND,
                channels: Mutex::new(HashMap::new()),
                channel_deadline: channel_deadline_from_env(),
                scan: Scan::new(scan_interval_from_env()),
                takeover: None,
            }),
            dir: dir.to_owned(),
            launcher,
        })
    }

    /// H3, H4: answers need `proof`; the App holds it. Without one every `decision.answer` is
    /// `FORBIDDEN`. Call it before the first call is made.
    #[must_use]
    pub fn with_proof(self, proof: Option<String>) -> Self {
        self.configure(|shared| shared.decisions.set_proof(proof))
    }

    /// O4: `active` says whether a Takeover of an Agent is active. Call it before the first call.
    #[must_use]
    pub fn with_takeover(self, active: impl Fn(&str) -> bool + Send + Sync + 'static) -> Self {
        self.configure(|shared| shared.takeover = Some(Arc::new(active)))
    }

    /// O4: replace the order of an Agent or a Door, and Steer it the new one unless the caller is
    /// the Agent itself or a Takeover is active.
    async fn set_order(&self, ctx: &Ctx, params: SetOrderParams) -> Result<RailNode, RpcError> {
        let SetOrderParams { id, order } = params;
        valid_order(&order)?;
        let node = self.shared.rail().node(&id)?;
        let allowed = match ctx.actor.kind {
            ActorKind::User => true,
            ActorKind::Agent => {
                ctx.actor.id == id || node.parent.as_deref() == Some(ctx.actor.id.as_str())
            }
            ActorKind::Ext => false,
        };
        if !allowed {
            return Err(RpcError::forbidden(format!(
                "{} may not set the order of agent {id}",
                ctx.actor.id
            )));
        }
        let node = self.shared.rail().set_order(&id, &order)?;
        ctx.touch(contracts::Verb::Wrote, &format!("agent:{id}"))?;
        ctx.emit(EventData::RailChanged);
        let takeover = self
            .shared
            .takeover
            .as_ref()
            .is_some_and(|active| active(&id));
        if ctx.actor.id != id && !takeover {
            let shared = Arc::clone(&self.shared);
            let text = order::steer(&order);
            tokio::spawn(async move {
                if let Err(err) = shared.prompt(&id, &text).await {
                    eprintln!("agents: the new order of {id} was not steered: {err}");
                }
            });
        }
        self.shared.node(&node.id)
    }

    /// Change a setting before any call has shared the Agents: the only holder of `shared` is
    /// `self`, plus its own `me`.
    fn configure(mut self, set: impl FnOnce(&mut Shared)) -> Self {
        let mut shared = Arc::into_inner(self.shared).expect("the Agents are not shared yet");
        set(&mut shared);
        self.shared = Arc::new_cyclic(|me| {
            shared.me = me.clone();
            shared
        });
        self
    }

    /// Stamp Statuses and time Ticks by `clock`, milliseconds since the Unix epoch, instead of
    /// the system's. Call it before the first call is made.
    #[must_use]
    pub fn with_clock(self, clock: impl Fn() -> i64 + Send + Sync + 'static) -> Self {
        self.configure(|shared| shared.clock = Arc::new(clock))
    }

    /// H11: wait `bound` instead of `STEER_BOUND` for a Steer's `UserPromptSubmit`. Call it
    /// before the first call is made.
    #[must_use]
    pub fn with_steer_bound(self, bound: Duration) -> Self {
        self.configure(|shared| shared.steer_bound = bound)
    }

    /// H13: wait `bound` instead of `INTERRUPT_BOUND` for an Interrupt's title. Call it before
    /// the first call is made.
    #[must_use]
    pub fn with_interrupt_bound(self, bound: Duration) -> Self {
        self.configure(|shared| shared.interrupt_bound = bound)
    }

    /// H11: Steer Agent `id` with `text`. The one function that writes a prompt to a Terminal;
    /// `agent.prompt` and the mailbox series' in-process delivery both call it.
    pub async fn prompt(&self, id: &str, text: &str) -> Result<(), PromptError> {
        self.shared.prompt(id, text).await
    }

    /// H11, H13: the user, or a Door for an Agent in its Workstream, may Steer or Interrupt; any other
    /// caller may not.
    fn may_steer(&self, actor: &Actor, id: &str, verb: &str) -> Result<(), RpcError> {
        let allowed = match actor.kind {
            ActorKind::User => true,
            ActorKind::Agent => self
                .shared
                .rail()
                .node(id)
                .is_ok_and(|node| node.parent.as_deref() == Some(actor.id.as_str())),
            ActorKind::Ext => false,
        };
        if allowed {
            Ok(())
        } else {
            Err(RpcError::forbidden(format!(
                "{} may not {verb} agent {id}",
                actor.id
            )))
        }
    }

    /// The Project folder: `.roundup/`'s parent, where its git repository (if any) lives.
    fn project_dir(&self) -> &Path {
        &self.shared.project_dir
    }

    /// Put a new Agent in the Rail and start Claude Code for it in a Terminal.
    async fn spawn(&self, ctx: &Ctx, mut params: SpawnParams) -> Result<RailNode, RpcError> {
        // F3: an Agent may start a child only if it is a Door, and the child's Home is that Door.
        if ctx.actor.kind == ActorKind::Agent {
            let door = self
                .shared
                .rail()
                .node(&ctx.actor.id)
                .is_ok_and(|node| node.kind == NodeKind::Workstream);
            if !door {
                return Err(RpcError::forbidden(format!(
                    "{} is not a Door and may not start an Agent",
                    ctx.actor.id
                )));
            }
            params.parent = Some(ctx.actor.id.clone());
        }
        let cwd = Path::new(&params.cwd);
        let order = match (params.prompt, params.order) {
            (Some(_), Some(_)) => {
                return Err(RpcError::new(
                    code::INVALID_PARAMS,
                    "prompt and order are exclusive",
                ));
            }
            (Some(prompt), None) => Order::work(&prompt),
            (None, Some(order)) => order,
            (None, None) => rail::default_order(NodeKind::Agent),
        };
        valid_order(&order)?;
        let (id, attempt) = {
            let mut rail = self.shared.rail();
            let node = rail.insert_ordered(
                NodeKind::Agent,
                name::UNNAMED,
                params.parent.as_deref(),
                None,
                Some(&order),
            )?;
            let attempt = rail.allocate_attempt(&node.id)?;
            self.shared.mark_starting(&node.id, attempt.clone());
            (node.id, attempt)
        };
        match self.run_agent(&id, &attempt, cwd).await {
            Ok(_) => {}
            Err(err) => {
                let mut rail = self.shared.rail();
                let mut runs = self.shared.runs();
                if runs.get(&id).is_some_and(|slot| slot.attempt() == attempt) {
                    runs.remove(&id);
                    if rail.node(&id)?.worktree.is_none() {
                        report_undo("remove the Agent from the Rail", rail.remove(&id));
                    }
                }
                return Err(err);
            }
        };
        ctx.emit(EventData::RailChanged);
        self.shared.node(&id)
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
            .spawn_behind(&node.id, Path::new(&params.cwd), None, true)
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

    /// A7: start a Workstream's Door. A23: a Door with A21 data resumes that conversation exactly as
    /// `resume` does (so a saved conversation that cannot resume fails and never starts fresh);
    /// any other Door, its first start included, starts fresh.
    async fn start_door(&self, ctx: &Ctx, id: &str) -> Result<RailNode, RpcError> {
        let saved = {
            let rail = self.shared.rail();
            if rail.node(id)?.kind != NodeKind::Workstream {
                return Err(RpcError::conflict(format!("{id} is not a Workstream")));
            }
            rail.has_conversation(id)?
        };
        if saved {
            return self.resume_saved(ctx, id, START_DOOR_ACK_BOUND).await;
        }
        let attempt = {
            let mut rail = self.shared.rail();
            let node = rail.node(id)?;
            if node.kind != NodeKind::Workstream {
                return Err(RpcError::conflict(format!("{id} is not a Workstream")));
            }
            let mut runs = self.shared.runs();
            if let Some(slot) = runs.get(id) {
                let live = match slot {
                    Slot::Running(run) => {
                        run.closing
                            || self
                                .shared
                                .terminals
                                .list()
                                .iter()
                                .any(|t| t.id == run.terminal_id && t.running)
                    }
                    _ => true,
                };
                if live {
                    return Err(RpcError::conflict(format!("Door {id} is already active")));
                }
            }
            let attempt = rail.allocate_attempt(id)?;
            runs.insert(
                id.to_owned(),
                Slot::Starting {
                    attempt: attempt.clone(),
                    since: (self.shared.clock)(),
                    named: true,
                    held: VecDeque::new(),
                },
            );
            attempt
        };
        let project = self.project_dir().to_owned();
        if let Err(err) = self.run_agent(id, &attempt, &project).await {
            let mut rail = self.shared.rail();
            let mut runs = self.shared.runs();
            if runs.get(id).is_some_and(|slot| slot.attempt() == attempt) {
                runs.remove(id);
                rail.detach_terminal(id)?;
            }
            return Err(err);
        }
        ctx.emit(EventData::RailChanged);
        self.shared.node(id)
    }

    /// A22: resume an exited Agent's saved conversation in a new Attempt. Returns only once the
    /// new Attempt's `SessionStart` has acknowledged it (`finish_resuming`) or the attempt is
    /// given up on: `rollback_resume` stops its Terminal, if one started, and restores the old
    /// node. `can_resume` reads false for the whole pending window (A20's Starting/Resuming
    /// mechanism already keeps `agent.stop` and `rail.remove` from claiming the id meanwhile).
    async fn resume(&self, ctx: &Ctx, id: &str) -> Result<RailNode, RpcError> {
        {
            let rail = self.shared.rail();
            let node = rail.node(id)?;
            if node.kind != NodeKind::Agent {
                return Err(RpcError::conflict(format!("{id} is not an Agent")));
            }
        }
        self.resume_saved(ctx, id, RESUME_ACK_BOUND).await
    }

    /// A22's launch, acknowledgement, rollback and serialization for an Agent (`agent.resume`) or
    /// a Door (A23's `rail.startDoor`) whose kind the caller has checked.
    async fn resume_saved(
        &self,
        ctx: &Ctx,
        id: &str,
        ack_bound: Duration,
    ) -> Result<RailNode, RpcError> {
        let (attempt, conversation_id, worktree, saved_cwd, done_rx) = {
            let mut rail = self.shared.rail();
            let node = rail.node(id)?;
            let mut runs = self.shared.runs();
            match runs.get(id) {
                Some(Slot::Starting { .. }) => {
                    return Err(RpcError::conflict(format!("{id} is still starting")));
                }
                Some(Slot::Resuming { .. }) => {
                    return Err(RpcError::conflict(format!("{id} is already resuming")));
                }
                Some(Slot::Closing) => {
                    return Err(RpcError::conflict(format!("{id} is stopping")));
                }
                Some(Slot::Running(run)) if !run.ended => {
                    return Err(RpcError::conflict(format!("{id} is running")));
                }
                None | Some(Slot::Running(_)) => {}
            }
            let Some((conversation_id, saved_cwd)) = rail.conversation(id)? else {
                return Err(RpcError::conflict(format!(
                    "{id} has no saved conversation"
                )));
            };
            let attempt = rail.allocate_attempt(id)?;
            let old = match runs.remove(id) {
                Some(Slot::Running(run)) => Some(Box::new(run)),
                _ => None,
            };
            let (done_tx, done_rx) = oneshot::channel();
            runs.insert(
                id.to_owned(),
                Slot::Resuming {
                    attempt: attempt.clone(),
                    since: (self.shared.clock)(),
                    old,
                    conversation_id: conversation_id.clone(),
                    cwd: saved_cwd.clone(),
                    terminal_id: None,
                    held: VecDeque::new(),
                    done: done_tx,
                },
            );
            (attempt, conversation_id, node.worktree, saved_cwd, done_rx)
        };
        ctx.emit(EventData::RailChanged);

        let verified = match &worktree {
            Some(worktree) => {
                let (git, project, worktree) = (
                    self.shared.git.clone(),
                    self.project_dir().to_owned(),
                    worktree::Worktree::from(worktree),
                );
                tokio::task::spawn_blocking(move || git.verify_resumable(&project, &worktree))
                    .await
                    .map_err(RpcError::internal)?
            }
            None if Path::new(&saved_cwd).is_dir() => Ok(()),
            None => Err(RpcError::conflict(format!("worktree_missing: {saved_cwd}"))),
        };
        if let Err(err) = verified {
            self.shared.rollback_resume(id, &attempt).await;
            return Err(err);
        }

        // A21 already saved the effective cwd the original run launched in: when a Worktree is
        // retained, that is some path under it (A21's own cwd is captured after Worktree
        // mapping, any subdirectory kept), never the Worktree's root by itself. `worktree` above
        // is only for `verify_resumable`'s Git check; the directory to launch into is always
        // `saved_cwd` as recorded.
        let effective_cwd = PathBuf::from(&saved_cwd);
        let spawned = match self
            .start(id, &attempt, &effective_cwd, Some(&conversation_id), false)
            .await
        {
            Ok(spawned) => spawned,
            Err(err) => {
                report_undo(
                    "delete the Agent's settings file",
                    Launcher::discard(&self.dir, id),
                );
                self.shared.rollback_resume(id, &attempt).await;
                return Err(err);
            }
        };
        let terminal_id = spawned.id.clone();
        let claimed = {
            let mut runs = self.shared.runs();
            match runs.get_mut(id) {
                Some(Slot::Resuming {
                    attempt: active,
                    terminal_id: slot,
                    ..
                }) if active == &attempt => {
                    *slot = Some(terminal_id.clone());
                    true
                }
                _ => false,
            }
        };
        if !claimed {
            // A14's attempt check already keeps a stop/remove from claiming a Resuming id;
            // nothing else can have. Kept so this can never leave a Terminal unaccounted for if
            // that ever changes.
            report_undo(
                "kill a resume Terminal that lost its race",
                self.shared.terminals.kill(&terminal_id).await,
            );
            return Err(RpcError::conflict(format!(
                "{id}: resume no longer pending"
            )));
        }
        // An ack that raced ahead of this very claim (held because the Terminal id was not yet
        // recorded) is acted on now, rather than left held until the 10 s timeout.
        self.shared.promote_held_resume_ack(id);
        tokio::spawn(watch(
            Arc::clone(&self.shared),
            id.to_owned(),
            attempt.clone(),
            spawned.events,
        ));

        let outcome = tokio::select! {
            ack = done_rx => ack.unwrap_or_else(|_| {
                Err(RpcError::internal(format!("{id}: resume ack channel dropped")))
            }),
            () = tokio::time::sleep(ack_bound) => {
                self.shared.rollback_resume(id, &attempt).await;
                Err(RpcError::conflict(format!(
                    "{id}: no acknowledgement within {}s",
                    ack_bound.as_secs()
                )))
            }
        };
        outcome?;
        ctx.emit(EventData::RailChanged);
        self.shared.node(id)
    }

    /// G3: how far the Agent's Worktree is from its Base. Reads only.
    async fn worktree_state(&self, id: &str) -> Result<WorktreeState, RpcError> {
        let record = self.shared.node(id)?.worktree;
        let Some(record) = record else {
            return Err(RpcError::new(code::NOT_FOUND, format!("no_worktree: {id}")));
        };
        let (git, project) = (self.shared.git.clone(), self.project_dir().to_owned());
        let worktree = worktree::Worktree::from(&record);
        tokio::task::spawn_blocking(move || git.state(&project, &worktree))
            .await
            .map_err(|err| RpcError::internal(format!("worktree_failed: {err}")))?
    }

    /// G4: land the Agent's branch on its Base, after the Project's `check` passes in the Worktree.
    async fn land(&self, id: &str) -> Result<Landed, RpcError> {
        let record = self.shared.node(id)?.worktree;
        let Some(record) = record else {
            return Err(RpcError::new(code::NOT_FOUND, format!("no_worktree: {id}")));
        };
        let check = self.shared.rail().get_worktrees()?.check;
        let (git, project) = (self.shared.git.clone(), self.project_dir().to_owned());
        let worktree = worktree::Worktree::from(&record);
        let base =
            tokio::task::spawn_blocking(move || git.land(&project, &worktree, check.as_deref()))
                .await
                .map_err(|err| RpcError::internal(format!("worktree_failed: {err}")))??;
        Ok(Landed { base })
    }

    /// When the Project's `worktrees` setting is on, make a Worktree for `id` (G2) and map `cwd`
    /// into it; `None` when the setting is off, so `run_agent` uses `cwd` unchanged. Any failure
    /// retains its ownership record when cleanup cannot safely remove the new Worktree.
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
        let attempt = self
            .shared
            .rail()
            .node(id)?
            .attempt
            .expect("reserved Attempt");
        let plan = self.shared.git.plan(&project, id, &attempt)?;
        self.shared.rail().mark_worktree_provisioning(id, &plan)?;
        let (git, provisioning_plan) = (self.shared.git.clone(), plan.clone());
        let made =
            match tokio::task::spawn_blocking(move || git.provision(&project, &provisioning_plan))
                .await
                .map_err(RpcError::internal)?
            {
                Ok(made) => made,
                Err(err) => {
                    if self.shared.git.recover(self.project_dir(), &plan).is_ok() {
                        self.shared.rail().clear_worktree(id)?;
                    }
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
        Ok(Some((mapped, made)))
    }

    /// Undo a Worktree `provision_worktree` made and clear the node's record of it: used when a
    /// later step (the cwd check, or starting the Agent itself) fails.
    async fn discard_worktree(&self, id: &str, worktree: worktree::Worktree) {
        let (git, project) = (self.shared.git.clone(), self.project_dir().to_owned());
        let pending = match self.shared.rail().provisioning() {
            Ok(records) => records.into_iter().find(|(node, _, _)| node.id == id),
            Err(err) => {
                eprintln!("agents: could not read Worktree ownership for {id}: {err}");
                return;
            }
        };
        let undone = tokio::task::spawn_blocking(move || match pending {
            Some((_, Some(plan), _)) => git.recover(&project, &plan),
            Some((_, None, _)) => Err(RpcError::internal(format!("worktree_failed: {UNPROVEN}"))),
            None => git.remove_landed(&project, &worktree),
        })
        .await
        .map_err(RpcError::internal)
        .and_then(|result| result);
        match undone {
            Ok(()) => report_undo(
                "clear the Agent's worktree",
                self.shared.rail().clear_worktree(id),
            ),
            Err(err) => eprintln!("agents: could not undo Worktree for {id}: {err}"),
        }
    }

    /// Start Claude Code for node `id`, marked as starting, in a Terminal recorded in the Rail,
    /// then register and watch it; returns the Terminal's id. Failed cleanup keeps its Worktree
    /// ownership record so reopening can recover it without deleting unrelated work.
    async fn run_agent(&self, id: &str, attempt: &str, cwd: &Path) -> Result<String, RpcError> {
        let node = self.shared.rail().node(id)?;
        // O2, O3: the first Steer is the node's order, whichever way it was set.
        let prompt = node.work.as_ref().map(order::steer);

        let retained = node.worktree.map(|w| PathBuf::from(w.path));
        if let Some(path) = &retained
            && !path.is_dir()
        {
            return Err(RpcError::internal(format!(
                "worktree_missing: {}",
                path.display()
            )));
        }
        let provisioned = if retained.is_none() {
            self.provision_worktree(id, cwd).await?
        } else {
            None
        };
        let effective_cwd = provisioned.as_ref().map_or_else(
            || retained.unwrap_or_else(|| cwd.to_owned()),
            |(cwd, _)| cwd.clone(),
        );
        let spawned = match self.start(id, attempt, &effective_cwd, None, true).await {
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
        let run_cwd = effective_cwd.to_string_lossy().into_owned();
        let prompts = self.shared.finish_starting(id, attempt, |named| Run {
            attempt: attempt.to_owned(),
            status_revision: 1,
            closing: false,
            ended: false,
            adapter: ClaudeCode::starting(move || clock()),
            prompt,
            steer: None,
            session_started: false,
            starred: false,
            interrupt: None,
            named,
            terminal_id,
            cwd: run_cwd,
        });
        let prompts = match prompts {
            Ok(prompts) => prompts,
            Err(err) => {
                report_undo(
                    "kill obsolete start Terminal",
                    self.shared.terminals.kill(&spawned.id).await,
                );
                return Err(err);
            }
        };
        for prompt in prompts {
            self.shared.steer_first_prompt(id, attempt, prompt);
        }
        tokio::spawn(watch(
            Arc::clone(&self.shared),
            id.to_owned(),
            attempt.to_owned(),
            spawned.events,
        ));
        Ok(spawned.id)
    }

    /// `resume` is A22's saved conversation id, added to argv as `--resume <id>`; `None` starts
    /// fresh. `attach` is false only for A22's pending resume, which must not record a
    /// `terminal_id` in the Rail until its launch is acknowledged.
    async fn start(
        &self,
        id: &str,
        attempt: &str,
        cwd: &Path,
        resume: Option<&str>,
        attach: bool,
    ) -> Result<terminal::Spawned, RpcError> {
        let role = match self.shared.rail().node(id)?.kind {
            NodeKind::Workstream => Role::Door,
            _ => Role::Agent,
        };
        let started = attempt.to_owned();
        // Waiting for Claude's config lock can take seconds; it must not hold a runtime thread.
        let (launcher, dir, node, folder, attempt, resume) = (
            self.launcher.clone(),
            self.dir.clone(),
            id.to_owned(),
            cwd.to_owned(),
            attempt.to_owned(),
            resume.map(str::to_owned),
        );
        let argv = tokio::task::spawn_blocking(move || {
            launcher.prepare(&dir, &node, &attempt, &folder, resume.as_deref(), role)
        })
        .await
        .map_err(RpcError::internal)??;
        let pending = self
            .shared
            .rail()
            .provisioning()?
            .into_iter()
            .find(|(node, _, _)| node.id == id);
        if let Some((node, plan, _)) = pending {
            let plan =
                plan.ok_or_else(|| RpcError::internal(format!("worktree_failed: {UNPROVEN}")))?;
            self.shared
                .rail()
                .set_worktree(id, &node.worktree.expect("provisioned Worktree"))?;
            self.shared.git.finish(self.project_dir(), &plan)?;
            self.shared.rail().clear_provisioning_owner(id)?;
        }
        let spawned = self.spawn_behind(id, cwd, Some(argv), attach).await?;
        self.shared.watch_channel(id, &started);
        if role == Role::Door
            && let Some(marker) = claude_code::settings_marker(&self.dir, id)
        {
            self.shared
                .scan_door(id, marker.to_string_lossy().into_owned());
        }
        Ok(spawned)
    }

    /// Start `command` (the login shell when `None`) in a Terminal and, when `attach`, record it
    /// as the one behind node `id`.
    async fn spawn_behind(
        &self,
        id: &str,
        cwd: &Path,
        command: Option<Vec<String>>,
        attach: bool,
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
        if attach {
            let attached = self.shared.rail().attach_terminal(id, &spawned.id);
            if let Err(err) = attached {
                report_undo(
                    "kill the Agent's Terminal",
                    self.shared.terminals.kill(&spawned.id).await,
                );
                return Err(err);
            }
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

/// O1: an order of the right shape, or `INVALID_PARAMS`.
fn valid_order(order: &Order) -> Result<(), RpcError> {
    order
        .validate()
        .map_err(|message| RpcError::new(code::INVALID_PARAMS, message))
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
        &["agent", "rail", "project", "decision"]
    }

    async fn call(&self, ctx: &Ctx, method: &str, value: Value) -> Result<Value, RpcError> {
        let shared = &self.shared;
        match method {
            "agent.spawn" => reply(&self.spawn(ctx, params(value)?).await?),
            "agent.prompt" => {
                let PromptParams { id, text } = params(value)?;
                self.may_steer(&ctx.actor, &id, "Steer")?;
                self.prompt(&id, &text).await?;
                reply(&())
            }
            "agent.interrupt" => {
                let NodeId { id } = params(value)?;
                self.may_steer(&ctx.actor, &id, "Interrupt")?;
                shared.interrupt(&id).await?;
                reply(&())
            }
            "agent.stop" => {
                let NodeId { id } = params(value)?;
                shared.stop(ctx.actor.clone(), &id).await?;
                reply(&())
            }
            "agent.worktreeState" => {
                let NodeId { id } = params(value)?;
                reply(&self.worktree_state(&id).await?)
            }
            "agent.land" => {
                let NodeId { id } = params(value)?;
                reply(&self.land(&id).await?)
            }
            "agent.discard" => {
                let NodeId { id } = params(value)?;
                shared.discard(ctx.actor.clone(), &id).await?;
                reply(&())
            }
            "rail.startDoor" => {
                let NodeId { id } = params(value)?;
                reply(&self.start_door(ctx, &id).await?)
            }
            "agent.resume" => {
                let NodeId { id } = params(value)?;
                reply(&self.resume(ctx, &id).await?)
            }
            "agent.setOrder" => reply(&self.set_order(ctx, params(value)?).await?),
            "agent.channelUp" => {
                let NodeId { id } = params(value)?;
                shared.channel_up(&ctx.actor, &id)?;
                reply(&())
            }
            "agent.signal" => {
                let SignalParams {
                    id,
                    attempt,
                    payload,
                } = params(value)?;
                if contracts::agent::parse_positive_ordinal(&attempt).is_none() {
                    return Err(RpcError::new(
                        code::INVALID_PARAMS,
                        "attempt must be a canonical positive decimal within signed 64-bit range",
                    ));
                }
                shared.observe(
                    ctx.actor.clone(),
                    &id,
                    &attempt,
                    Observation::Signal(payload),
                )?;
                reply(&())
            }
            "agent.permission" => {
                let PermissionParams { id, payload } = params(value)?;
                reply(&shared.permission(ctx.actor.clone(), &id, payload).await?)
            }
            "agent.ask" => {
                let ask: AskParams = params(value)?;
                reply(&shared.ask(ctx.actor.clone(), ask).await?)
            }
            "decision.list" => reply(&shared.decisions.list()),
            "decision.answer" => {
                let AnswerParams { id, answer, proof } = params(value)?;
                shared.answer(ctx.actor.clone(), &id, &answer, proof.as_deref())?;
                reply(&())
            }
            "rail.tree" => reply(&shared.tree()?),
            "rail.createWorkstream" => {
                let CreateWorkstreamParams {
                    name,
                    parent,
                    order,
                } = params(value)?;
                if let Some(order) = &order {
                    valid_order(order)?;
                }
                let node = shared.rail().insert_ordered(
                    NodeKind::Workstream,
                    &name,
                    parent.as_deref(),
                    None,
                    order.as_ref(),
                )?;
                ctx.emit(EventData::RailChanged);
                reply(&shared.node(&node.id)?)
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
                reply(&shared.node(&node.id)?)
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
    use super::{
        Clock, KillsTerminals, Observation, Run, Shared, Slot, rail, shell_name, watch, worktree,
    };

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
            git: worktree::Git::from_env(),
            project_dir: dir.path().to_owned(),
            rail: Mutex::new(rail::Rail::open(&dir.path().join("agents.db")).unwrap()),
            runs: Mutex::new(HashMap::new()),
            bus: bus.clone(),
            kill: Arc::clone(&terminals) as Arc<dyn super::KillsTerminals>,
            terminals,
            clock,
            opened: 0,
            decisions: super::decision::Decisions::new(None, Duration::from_secs(4)),
            me: std::sync::Weak::new(),
            steer_bound: super::STEER_BOUND,
            interrupt_bound: super::INTERRUPT_BOUND,
            channels: Mutex::new(HashMap::new()),
            channel_deadline: super::CHANNEL_DEADLINE,
            scan: super::Scan::new(super::SCAN_INTERVAL),
            takeover: None,
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
                attempt: "1".into(),
                status_revision: 1,
                closing: false,
                ended: false,
                adapter: ClaudeCode::starting(move || adapter_clock()),
                prompt: None,
                steer: None,
                session_started: false,
                starred: false,
                interrupt: None,
                named: false,
                terminal_id: "1".into(),
                cwd: "/".into(),
            };
            shared.runs().insert("1".into(), Slot::Running(run));
            let (terminal, terminal_events) = broadcast::channel(16);
            let events = bus.subscribe();
            let watcher = tokio::spawn(watch(
                Arc::clone(&shared),
                "1".into(),
                "1".into(),
                terminal_events,
            ));
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
                .observe(Actor::daemon(), "1", "1", Observation::Signal(payload))
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
                attempt: "1".into(),
                since: 0,
                named: false,
                held: VecDeque::new(),
            },
        );

        let err = shared
            .observe(Actor::daemon(), "1", "1", Observation::Tick)
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
        shared.mark_starting(&id, "1".into());
        if let Some(Slot::Starting { held, .. }) = shared.runs().get_mut(&id) {
            held.push_back((
                Actor::daemon(),
                json!({"hook_event_name": "UserPromptSubmit", "prompt": "fix the build"}),
            ));
        }
        let lock = lock_db_for_writes(dir.path());

        let prompts = shared.finish_starting(&id, "1", |named| Run {
            attempt: "1".into(),
            status_revision: 1,
            closing: false,
            ended: false,
            adapter: ClaudeCode::starting(|| 0),
            prompt: None,
            steer: None,
            session_started: false,
            starred: false,
            interrupt: None,
            named,
            terminal_id: "1".into(),
            cwd: "/".into(),
        });

        drop(lock);
        assert_eq!(prompts.unwrap(), Vec::<String>::new());
        // Neither lock was poisoned by the failed write: both can still be acquired.
        assert!(matches!(shared.runs().get(&id), Some(Slot::Running(_))));
        shared.rail().tree().unwrap();
    }

    /// A21: a held `SessionStart` whose conversation save fails must abort `finish_starting`
    /// outright (never apply it, never drain anything after it) rather than log and continue as
    /// a failed rename does; and it must put `id` back as `Running` so the caller's own
    /// failed-spawn cleanup (which only removes an Attempt it still finds in `runs`) still runs.
    #[test]
    fn a21_a_failed_held_save_reinserts_so_the_caller_can_clean_up() {
        let (dir, _bus, shared) = shared_over_temp_dir(Arc::new(|| 0));
        let id = shared
            .rail()
            .insert(contracts::agent::NodeKind::Agent, "new-agent", None, None)
            .unwrap()
            .id;
        shared.mark_starting(&id, "1".into());
        if let Some(Slot::Starting { held, .. }) = shared.runs().get_mut(&id) {
            held.push_back((
                Actor::daemon(),
                json!({
                    "hook_event_name": "SessionStart",
                    "session_id": "11111111-1111-1111-1111-111111111111",
                }),
            ));
            // Never reached if the save aborts the drain as it must.
            held.push_back((Actor::daemon(), json!({"hook_event_name": "Stop"})));
        }
        let lock = lock_db_for_writes(dir.path());

        let result = shared.finish_starting(&id, "1", |named| Run {
            attempt: "1".into(),
            status_revision: 1,
            closing: false,
            ended: false,
            adapter: ClaudeCode::starting(|| 0),
            prompt: None,
            steer: None,
            session_started: false,
            starred: false,
            interrupt: None,
            named,
            terminal_id: "1".into(),
            cwd: "/".into(),
        });

        drop(lock);
        assert!(result.is_err());
        let runs = shared.runs();
        let Some(Slot::Running(run)) = runs.get(&id) else {
            panic!("put back as Running for the caller's cleanup to find")
        };
        // The Stop held after the SessionStart was never drained: the abort is immediate.
        assert_eq!(run.adapter.status().unwrap().kind, Kind::Working);
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
                    attempt: "1".into(),
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
                    shared.finish_starting(&id, "1", |named| {
                        holding_tx.send(()).unwrap();
                        go_rx.recv().unwrap();
                        Run {
                            attempt: "1".into(),
                            status_revision: 1,
                            closing: false,
                            ended: false,
                            adapter: ClaudeCode::starting(|| 0),
                            prompt: None,
                            steer: None,
                            session_started: false,
                            starred: false,
                            interrupt: None,
                            named,
                            terminal_id: id.clone(),
                            cwd: "/".into(),
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
                        "1",
                        Observation::Signal(
                            json!({"hook_event_name": "Stop", "tool_name": "Bash"}),
                        ),
                    )
                })
            };
            about_rx.recv().unwrap();
            go_tx.send(()).unwrap();

            registering.join().unwrap().unwrap();
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
            git: worktree::Git::from_env(),
            project_dir: dir.path().to_owned(),
            rail: Mutex::new(rail::Rail::open(&dir.path().join("agents.db")).unwrap()),
            runs: Mutex::new(HashMap::new()),
            bus: bus.clone(),
            terminals: Arc::new(Terminals::open(dir.path(), bus).unwrap()),
            kill: Arc::new(FailingKill),
            clock: Arc::new(|| 0),
            opened: 0,
            decisions: super::decision::Decisions::new(None, Duration::from_secs(4)),
            me: std::sync::Weak::new(),
            steer_bound: super::STEER_BOUND,
            interrupt_bound: super::INTERRUPT_BOUND,
            channels: Mutex::new(HashMap::new()),
            channel_deadline: super::CHANNEL_DEADLINE,
            scan: super::Scan::new(super::SCAN_INTERVAL),
            takeover: None,
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
                attempt: "1".into(),
                status_revision: 1,
                closing: false,
                ended: false,
                adapter: ClaudeCode::starting(|| 0),
                prompt: None,
                steer: None,
                session_started: false,
                starred: false,
                interrupt: None,
                named: true,
                terminal_id: "1".into(),
                cwd: "/".into(),
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

    /// A Door whose kill fails must not have its children lifted either: `stop` returning
    /// early before it calls `lift_children` is what this proves.
    #[tokio::test]
    async fn a16_a_failed_stop_of_a_door_leaves_its_children_in_place() {
        let shared = shared_with_a_failing_kill();
        let group = shared
            .rail()
            .insert(NodeKind::Workstream, "g", None, None)
            .unwrap()
            .id;
        shared.rail().allocate_attempt(&group).unwrap();
        shared.runs().insert(
            group.clone(),
            Slot::Running(Run {
                attempt: "1".into(),
                status_revision: 1,
                closing: false,
                ended: false,
                adapter: ClaudeCode::starting(|| 0),
                prompt: None,
                steer: None,
                session_started: false,
                starred: false,
                interrupt: None,
                named: true,
                terminal_id: "1".into(),
                cwd: "/".into(),
            }),
        );
        let child = shared
            .rail()
            .insert(NodeKind::Workstream, "child", Some(&group), None)
            .unwrap()
            .id;

        shared.remove(Actor::daemon(), &group).await.unwrap_err();

        let tree = shared.rail().tree().unwrap();
        let child_node = tree.iter().find(|n| n.id == child).unwrap();
        assert_eq!(child_node.parent.as_deref(), Some(group.as_str()));
    }

    #[tokio::test]
    async fn a7_old_title_tick_exit_and_signal_do_not_change_a_new_run() {
        let mut watched = Watched::start();
        {
            let mut runs = watched.shared.runs();
            let Some(Slot::Running(run)) = runs.get_mut("1") else {
                panic!("Running")
            };
            run.attempt = "2".into();
            run.terminal_id = "new-terminal".into();
        }
        for observation in [
            Observation::Title("old title".into()),
            Observation::Tick,
            Observation::Exit { code: Some(1) },
            Observation::Signal(json!({"hook_event_name":"Stop"})),
        ] {
            // A20: ignored, never an error to the sender.
            let ignored = watched
                .shared
                .observe(Actor::daemon(), "1", "1", observation)
                .unwrap();
            assert_eq!(ignored, None);
        }
        assert!(watched.events.try_recv().is_err());
        let runs = watched.shared.runs();
        let Some(Slot::Running(run)) = runs.get("1") else {
            panic!("Running")
        };
        assert_eq!(run.attempt, "2");
        assert_eq!(run.terminal_id, "new-terminal");
        assert!(!run.ended);
        assert_eq!(run.adapter.status().unwrap().kind, Kind::Working);
    }

    #[tokio::test]
    async fn a7_status_revisions_order_transitions_even_when_the_clock_ties() {
        let (_dir, bus, shared) = shared_over_temp_dir(Arc::new(|| 0));
        let id = running_agent(&shared, None);
        shared.rail().allocate_attempt(&id).unwrap();
        let mut events = bus.subscribe();
        for (index, event) in [
            "UserPromptSubmit",
            "Stop",
            "StopFailure",
            "UserPromptSubmit",
        ]
        .iter()
        .enumerate()
        {
            shared
                .observe(
                    Actor::daemon(),
                    &id,
                    "1",
                    Observation::Signal(json!({"hook_event_name":event})),
                )
                .unwrap();
            let status = loop {
                if let EventData::AgentStatus(status) = events.try_recv().unwrap().data {
                    break status;
                }
            };
            assert_eq!(status.status_revision, (index + 2).to_string());
            assert_eq!(status.status.since, 0);
            let node = shared.node(&id).unwrap();
            assert_eq!(node.attempt.as_deref(), Some(status.attempt.as_str()));
            assert_eq!(
                node.status_revision.as_deref(),
                Some(status.status_revision.as_str())
            );
            assert_eq!(node.status, Some(status.status));
        }
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
            git: worktree::Git::from_env(),
            project_dir: dir.path().to_owned(),
            rail: Mutex::new(rail::Rail::open(&dir.path().join("agents.db")).unwrap()),
            runs: Mutex::new(HashMap::new()),
            bus: bus.clone(),
            terminals: Arc::new(Terminals::open(dir.path(), bus).unwrap()),
            kill: Arc::new(FlakyKill(std::sync::atomic::AtomicBool::new(false))),
            clock: Arc::new(|| 0),
            opened: 0,
            decisions: super::decision::Decisions::new(None, Duration::from_secs(4)),
            me: std::sync::Weak::new(),
            steer_bound: super::STEER_BOUND,
            interrupt_bound: super::INTERRUPT_BOUND,
            channels: Mutex::new(HashMap::new()),
            channel_deadline: super::CHANNEL_DEADLINE,
            scan: super::Scan::new(super::SCAN_INTERVAL),
            takeover: None,
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
        let node = shared.node(&id).unwrap();
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
    /// H7(c): `elapsed_ms` after the hook began, on an injected clock and the 4 s `timeout` the
    /// Decisions are built with, the hook's connection closes. Returns the events and the Kind.
    async fn hook_closes_after(
        elapsed_ms: i64,
    ) -> (Vec<EventData>, Vec<contracts::decision::Decision>, Kind) {
        use std::sync::atomic::{AtomicI64, Ordering};
        let now = Arc::new(AtomicI64::new(1_000));
        let clock = {
            let now = Arc::clone(&now);
            Arc::new(move || now.load(Ordering::SeqCst))
        };
        let (_dir, bus, shared) = shared_over_temp_dir(clock);
        let id = running_agent(&shared, None);
        let mut events = bus.subscribe();
        let payload = json!({"hook_event_name": "PermissionRequest", "tool_name": "Bash",
                             "tool_input": {"command": "ls"}});
        let hook = {
            let (shared, id) = (Arc::clone(&shared), id.clone());
            tokio::spawn(async move { shared.permission(Actor::daemon(), &id, payload).await })
        };
        while shared.decisions.list().is_empty() {
            tokio::task::yield_now().await;
        }

        now.fetch_add(elapsed_ms, Ordering::SeqCst);
        hook.abort();
        let _ = hook.await;

        let seen = std::iter::from_fn(|| events.try_recv().ok())
            .map(|event| event.data)
            .filter(|data| {
                matches!(
                    data,
                    EventData::DecisionOpened(_) | EventData::DecisionCleared(_)
                )
            })
            .collect();
        let kind = match shared.runs().get(&id) {
            Some(Slot::Running(run)) => run.adapter.status().unwrap().kind,
            _ => panic!("the Agent is running"),
        };
        (seen, shared.decisions.list(), kind)
    }

    #[tokio::test]
    async fn h7_a_close_at_the_hook_timeout_leaves_the_decision_open_and_unanswerable() {
        let (seen, open, kind) = hook_closes_after(4_000).await;

        assert_eq!(open.len(), 1);
        assert!(!open[0].answerable);
        assert_eq!(kind, Kind::NeedsYou);
        assert_eq!(
            seen.len(),
            1,
            "decision.opened is not repeated and nothing clears: {}",
            seen.len()
        );
        assert!(matches!(seen[0], EventData::DecisionOpened(_)));
    }

    #[tokio::test]
    async fn h7_a_close_before_the_hook_timeout_is_a_no_or_an_esc() {
        let (seen, open, kind) = hook_closes_after(3_999).await;

        assert!(open.is_empty());
        assert_eq!(kind, Kind::Idle);
        assert!(
            matches!(&seen[1], EventData::DecisionCleared(c) if c.outcome == contracts::decision::Outcome::Terminal)
        );
    }
}
