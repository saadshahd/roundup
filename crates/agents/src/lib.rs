//! Agents, the Rail tree and the Claude Code adapter. Owner: agents Builder.

#[doc(hidden)]
pub mod schema;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use contracts::agent::{
    Channel, ChannelEvent, CreateWorkstreamParams, ExtrasAddParams, KeptItem, KeptReason, Landed,
    MoveParams, NodeId, NodeKind, Order, PromptParams, RailNode, Removal, RemovalKind, RemovedItem,
    RenameParams, RunParams, SetModelParams, SetOrderParams, SignalOutcome, SignalParams,
    SpawnParams, StartDoorParams, StatusEvent, Stray, StrayEvent, WorktreeState,
};
use contracts::decision::{AnswerParams, CaptureParams, Outcome, PermissionParams};
use contracts::project::{Mcp, ProjectModels, ProjectSettings, Worktrees};
use contracts::terminal::{OpenParams, Opened, SpawnParams as TerminalSpawn};
use contracts::{Actor, ActorKind, EventData, Kind, Status};
use rpc::{Bus, Ctx, Module, OpenError, RpcError, code, params, reply};
use serde_json::Value;
use terminal::Terminals;
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::oneshot;
use tokio::time::{Instant, sleep_until};

mod asks;
pub mod claude_code;
mod decision;
pub mod marks;
mod models;
pub mod placement;
pub use asks::Asks;
pub use models::mcp_on;
mod name;
mod order;
mod peer;
mod rail;
mod scan;
pub mod worktree;

use claude_code::{ClaudeCode, Launcher, Pair, Role};

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
type ResumeEffect = Arc<dyn Fn(&str, &str) -> Result<(), RpcError> + Send + Sync>;

type SteerEffect = Arc<dyn Fn(&str) -> Result<(), RpcError> + Send + Sync>;

/// H19: told, under the Agents' locks, each gate decision as `(agent id, released)`.
type GateObserver = Arc<dyn Fn(&str, bool) + Send + Sync>;

/// State the Terminal watchers and the RPC calls share.
/// A33: an `idle` Status of an Agent that waits on blocker `wait` reads `blocked`; every other
/// Kind keeps its own.
fn waiting_on(status: &Status, wait: Option<u32>) -> Status {
    match (status.kind, wait) {
        (Kind::Idle, Some(blocker)) => Status {
            kind: Kind::Blocked,
            label: format!("waits on #{blocker}"),
            since: status.since,
        },
        _ => status.clone(),
    }
}

struct Shared {
    git: worktree::Git,
    project_dir: PathBuf,
    rail: Mutex<rail::Rail>,
    /// A node is marked `Starting`, and unmarked on failure, in the same Rail critical section
    /// that creates or reserves it, so `agent.stop` never finds a starting node unmarked.
    runs: Mutex<HashMap<String, Slot>>,
    /// A41: removal excludes Door starts until its Rail transaction has committed.
    door_lifecycle: tokio::sync::RwLock<()>,
    bus: Bus,
    terminals: Arc<Terminals>,
    /// Milliseconds since the Unix epoch. Adapters stamp Statuses with it and the watchers time
    /// Ticks by it, so the two never disagree about when a hold ends.
    clock: Clock,
    /// H2 to H7: the open permission Decisions, in memory only.
    decisions: decision::Decisions,
    /// Itself, so a Signal's fold can start H12's first Steer on a task of its own.
    me: Weak<Shared>,
    /// E6: the Channel of each Agent's current Attempt that has left `pending`, as
    /// `(attempt, channel)`; an entry of an earlier Attempt reads as `pending`.
    channels: Mutex<HashMap<String, (String, Channel)>>,
    /// O4: whether a Takeover of the Agent is active, which keeps a changed order from being
    /// sent to it. Without one, none is.
    takeover: Option<TakeoverProbe>,
    /// E3: the Daemon supplies mutable Context before the one first Steer.
    context: Option<ContextText>,
    /// A33: the lowest open blocker of the Todos each Agent created that wait; an idle Agent in
    /// it reads `blocked`.
    waits: Mutex<HashMap<String, u32>>,
    /// F5: the scan of the Doors' process trees.
    scan: Scan,
    /// Each Door's strays as last announced; a Door with none has no entry.
    strays: Mutex<HashMap<String, Vec<Stray>>>,
    /// H11: how long a Steer waits for its `UserPromptSubmit` Signal.
    steer_bound: Duration,
    steer: SteerSettings,
    events: events::State,
    rail_view: rail_view::State,
    ending: ending::State,
    resume_effect: Option<ResumeEffect>,
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

/// A guarded effect keeps Agent locks until its IO finishes. A contending async
/// worker yields its reactor duties before waiting, so Signals and effect completion
/// remain schedulable. The uncontended path and non-reactor callers need no handoff.
fn agent_lock<T>(mutex: &Mutex<T>) -> std::sync::LockResult<MutexGuard<'_, T>> {
    match mutex.try_lock() {
        Ok(guard) => Ok(guard),
        Err(std::sync::TryLockError::Poisoned(err)) => Err(err),
        Err(std::sync::TryLockError::WouldBlock) => {
            if tokio::runtime::Handle::try_current().is_ok_and(|handle| {
                handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread
            }) {
                tokio::task::block_in_place(|| mutex.lock())
            } else {
                mutex.lock()
            }
        }
    }
}

type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;

type ContextText = Arc<
    dyn Fn(
            Vec<RailNode>,
            String,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, RpcError>> + Send>>
        + Send
        + Sync,
>;

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

/// B6: keep a refusal before bytes distinct from an effect without completion evidence.
/// F5: how long between two reads of the process table. `ROUNDUP_SCAN_MS` shortens it for a test.
pub const SCAN_INTERVAL: Duration = Duration::from_millis(2000);

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
        /// A35 reserves this prompt before the acknowledging Status can admit Messages.
        prompt: Option<String>,
        /// H19: retain a ready title arriving before the ack, so the first prompt can enter.
        starred: bool,
        /// The Terminal this launch started, once `agent.resume` has spawned it; `None` until
        /// then, so a Signal cannot arrive for it any earlier.
        terminal_id: Option<String>,
        /// Signals held exactly as `Starting`'s, awaiting the ack among them (A14).
        held: VecDeque<(Actor, Value)>,
        /// Promotion or timeout claims this sender under the runs lock. Failure sends only
        /// after cleanup finishes; a timer that loses the claim still awaits this sender.
        done: tokio::sync::oneshot::Sender<Result<(), RpcError>>,
    },
    Running(Run),
    /// A failed Resume retains its Terminal until cleanup is confirmed or explicitly stopped.
    ResumeCleanup {
        attempt: String,
        terminal_id: String,
        pending: bool,
        error: RpcError,
    },
    Closing,
}

impl Slot {
    fn attempt(&self) -> &str {
        match self {
            Self::Starting { attempt, .. }
            | Self::Resuming { attempt, .. }
            | Self::ResumeCleanup { attempt, .. } => attempt,
            Self::Running(run) => &run.attempt,
            Self::Closing => "",
        }
    }
    fn closing(&self) -> bool {
        matches!(self, Self::Closing | Self::ResumeCleanup { .. })
            || matches!(self, Self::Running(run) if run.closing)
    }

    /// The node's name is final: no prompt renames it.
    fn settle_name(&mut self) {
        match self {
            Self::Running(run) => run.named = true,
            Self::Starting { named, .. } => *named = true,
            Self::Resuming { .. } | Self::ResumeCleanup { .. } | Self::Closing => {}
        }
    }
}

/// B6: an accounted first Steer. None of these outcomes authorizes another initial prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirstSteerCompletion {
    /// The physical write finished and UserPromptSubmit acknowledged it.
    Submitted,
    /// The prompt was dropped or refused without writing bytes.
    Refused,
    /// The physical effect ended, but submission (or a partial write) is unknown.
    Uncertain,
}

/// An Agent's program and what is left to tell it.
enum FirstPrompt {
    Reserved,
    Written,
    Acknowledged,
}

struct Run {
    /// A42 keeps a new Ask's first Steer byte for byte; other starts include E3 Context.
    startup_context: bool,
    attempt: String,
    status_revision: i64,
    closing: bool,
    ended: bool,
    adapter: ClaudeCode,
    /// H12: sent as a Steer at the first `SessionStart`, then gone.
    prompt: Option<String>,
    /// H12: no Message overtakes the first Steer before its completion or refusal.
    first_pending: bool,
    /// B6: retained first Steer result; only its physical writer may settle an active effect.
    first_completion: Option<tokio::sync::watch::Sender<Option<FirstSteerCompletion>>>,
    /// Retained after the first Steer acknowledgement so a dropped coordinator can retry.
    first_prompt: Option<FirstPrompt>,
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
/// Facts for binding Messages to the receiver of a Terminal at the lifecycle cut.
/// These are not a Rail presentation or permission to Steer the receiver.
#[derive(Debug, PartialEq, Eq)]
pub struct TerminalOwner {
    pub id: String,
    /// Persisted Attempt, matching `RailNode.attempt`, including during pending resume.
    pub attempt: Option<String>,
    /// Active Status revision; absence permanently ends this attached receiver (B36).
    pub status_revision: Option<String>,
    /// B9: no Status, or a done/error Status, ends this receiver.
    pub ended: bool,
}

/// The user's exact Ask preserves command position; ordinary instructions keep H20's lead.
pub enum Steer<'a> {
    Instruction(&'a str),
    User(&'a str),
}
impl<'a, T: AsRef<str> + ?Sized> From<&'a T> for Steer<'a> {
    fn from(text: &'a T) -> Self {
        Self::Instruction(text.as_ref())
    }
}

pub struct Agents {
    shared: Arc<Shared>,
    dir: PathBuf,
    launcher: Launcher,
}

mod dispatch;
mod door;
mod ending;
mod events;
mod model_wiring;
mod rail_view;
mod registration;
mod removal;
mod resume;
mod spawn;
mod steer;
mod store;
mod worktree_wiring;

use door::{Scan, scan_interval_from_env};
use ending::{Ending, RailPublication};
pub use events::{InboxObserver, PromptObserver};
use events::{channel_deadline_from_env, channel_in, watch};
use rail_view::{valid_explicit_order, valid_order};
use registration::{hold, save_signal_conversation};
use steer::SteerError;
use worktree_wiring::report_undo;

/// G6: a legacy Worktree record from before ownership proofs; nothing may clean it up.
const UNPROVEN: &str = "unfinished legacy provisioning has no ownership proof";

struct SteerSettings {
    before_steer: Option<SteerEffect>,
    /// H13: how long an Interrupt waits for the title to change.
    interrupt_bound: Duration,
}

impl Shared {
    /// H11, H16: the one place a prompt is written to a Terminal. `first` marks the H12 first
    /// Steer, which goes before any Message. `text` goes out once, as one
    /// bracketed paste and then `\r` as a write of its own, when `id` is `idle`; this returns when its `UserPromptSubmit`
    /// Signal arrives. `first` binds the initial Steer to its Attempt.
    /// Nothing is written again on any failure.
    async fn prompt(
        &self,
        id: &str,
        text: &str,
        first: Option<&str>,
        original: Option<&str>,
        verbatim: bool,
    ) -> Result<(), SteerError> {
        let not_found = || PromptError::NotFound { id: id.to_owned() };
        let (terminal_id, submitted) = loop {
            let mut completion = {
                let mut runs = self.runs();
                let Some(Slot::Running(run)) = runs.get_mut(id) else {
                    return Err(SteerError::Refused(match runs.get(id) {
                        Some(Slot::Starting { .. } | Slot::Resuming { .. }) => PromptError::Busy {
                            id: id.to_owned(),
                            kind: Kind::Working,
                        },
                        _ => not_found(),
                    }));
                };
                if first.is_some_and(|attempt| attempt != run.attempt)
                    // B6: a held prompt never reaches a replacement Attempt.
                    || original.is_some_and(|attempt| attempt != run.attempt)
                    || run.ended
                    || run.closing
                    || run.adapter.is_ended()
                {
                    return Err(SteerError::Refused(not_found()));
                }
                let kind = run
                    .adapter
                    .status()
                    .map_or(Kind::Working, |status| status.kind);
                let ahead = run.first_pending && first.is_none();
                // An idle Signal can precede the first Steer's task resuming from its ack.
                // Subscribe under the admission lock so Busy cannot miss the final wake-up.
                if ahead
                    && matches!(kind, Kind::Idle | Kind::Error)
                    && matches!(run.first_prompt, Some(FirstPrompt::Acknowledged))
                    && let Some(completion) = &run.first_completion
                {
                    completion.subscribe()
                } else {
                    if ahead
                        || !matches!(kind, Kind::Idle | Kind::Error)
                        || run.steer.as_ref().is_some_and(|steer| !steer.is_closed())
                    {
                        return Err(SteerError::Refused(PromptError::Busy {
                            id: id.to_owned(),
                            kind,
                        }));
                    }
                    let (steer, submitted) = oneshot::channel();
                    run.steer = Some(steer);
                    // The first Steer stays ahead of Messages through physical completion.
                    if first.is_some() && matches!(run.first_prompt, Some(FirstPrompt::Reserved)) {
                        run.first_prompt = Some(FirstPrompt::Written);
                    }
                    break (run.terminal_id.clone(), submitted);
                }
            };
            completion
                .wait_for(Option::is_some)
                .await
                .map_err(|_| SteerError::Refused(not_found()))?;
        };
        let bytes = if verbatim {
            claude_code::verbatim_bytes(text)
        } else {
            claude_code::steer_bytes(text)
        };
        let before = self.steer.before_steer.clone();
        let agent = id.to_owned();
        // B6: await the physical effect before starting the submit timeout. An enqueue
        // acknowledgement could otherwise leave bytes to execute after completion.
        let written = self
            .terminals
            .write_with(&terminal_id, move |writer| {
                if let Some(before) = before {
                    before(&agent)?;
                }
                claude_code::write_submitted(writer, &bytes).map_err(|err| {
                    RpcError::unknown_outcome(format!("Steer bytes may have been written: {err}"))
                })
            })
            .await;
        let failure = match written {
            Err(err) if err.code == code::NOT_FOUND => {
                return Err(SteerError::Refused(not_found()));
            }
            Err(err) => {
                eprintln!("agents: could not write the prompt to terminal {terminal_id}: {err}");
                let reason = PromptError::NotAccepted { id: id.to_owned() };
                if err.code == code::UNKNOWN_OUTCOME {
                    SteerError::Uncertain(reason)
                } else {
                    SteerError::Refused(reason)
                }
            }
            Ok(()) => match tokio::time::timeout(self.steer_bound, submitted).await {
                Ok(Ok(())) => return Ok(()),
                _ => SteerError::Uncertain(PromptError::NotAccepted { id: id.to_owned() }),
            },
        };
        let mut runs = self.runs();
        let ended = match runs.get_mut(id) {
            Some(Slot::Running(run)) if run.terminal_id == terminal_id => {
                run.steer = None;
                run.ended || run.closing
            }
            _ => true,
        };
        Err(if ended {
            match failure {
                SteerError::Refused(_) => SteerError::Refused(not_found()),
                SteerError::Uncertain(_) => SteerError::Uncertain(not_found()),
            }
        } else {
            failure
        })
    }
    /// H13, H16: the other place a Terminal is written on an Agent's behalf. One `ESC` goes out
    /// when `id` has a turn to end; this returns when the Agent's title shows it is no longer
    /// working (`Kind::Idle`). Physical completion precedes the title timeout; `NOT_ACKED`
    /// means the Escape was written but its outcome is uncertain. Nothing is retried.
    async fn interrupt(&self, id: &str, original: Option<&str>) -> Result<(), RpcError> {
        let not_found = || RpcError::not_found(format!("agent {id} is not running"));
        let (terminal_id, acked) = {
            let mut runs = self.runs();
            // B6: a held Interrupt never ends a replacement Attempt's turn.
            if let Some(original) = original
                && runs.get(id).is_none_or(|slot| slot.attempt() != original)
            {
                return Err(RpcError::conflict(format!(
                    "{id}: original Attempt changed or ended before Interrupt"
                )));
            }
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
        // B6: an enqueue acknowledgement cannot bound the Escape's effect. Await its
        // physical completion so no timeout result leaves a future write behind.
        let written = self
            .terminals
            .write_with(&terminal_id, |writer| {
                writer
                    .write_all(&claude_code::interrupt_bytes())
                    .and_then(|()| writer.flush())
                    .map_err(|err| {
                        RpcError::unknown_outcome(format!(
                            "Interrupt bytes may have been written: {err}"
                        ))
                    })
            })
            .await;
        let error = match written {
            Err(err) => {
                eprintln!("agents: could not write the interrupt to terminal {terminal_id}: {err}");
                if err.code == code::UNKNOWN_OUTCOME {
                    RpcError::unknown_outcome(format!(
                        "agent {id}: Interrupt outcome uncertain; do not retry: {err}"
                    ))
                } else {
                    RpcError::new(
                        err.code,
                        format!("agent {id}: Interrupt refused; no bytes written: {err}"),
                    )
                }
            }
            Ok(()) => match tokio::time::timeout(self.steer.interrupt_bound, acked).await {
                Ok(Ok(())) => return Ok(()),
                _ => {
                    eprintln!("agents: {id}: the title did not change after the interrupt");
                    RpcError::new(
                        code::NOT_ACKED,
                        format!(
                            "agent {id} did not acknowledge the interrupt; Escape written; outcome uncertain; do not retry"
                        ),
                    )
                }
            },
        };
        if let Some(Slot::Running(run)) = self.runs().get_mut(id) {
            run.interrupt = None;
        }
        Err(error)
    }
}

#[cfg(test)]
use store::tests;
