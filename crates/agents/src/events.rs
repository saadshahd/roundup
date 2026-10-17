use super::*;

pub(super) struct State {
    /// H19: a test's observer of each gate decision; absent in production.
    pub(super) gate_observer: Option<GateObserver>,
    /// E6: how long after a start `agent.channelUp` may take before the Channel is `missing`.
    pub(super) channel_deadline: Duration,
    /// B39: told each submitted prompt; its answer says whether the hook must drop the prompt.
    pub(super) prompt_observer: std::sync::OnceLock<PromptObserver>,
    /// B50, B51: what the Claude inbox of each Agent is and what its transcript has recorded.
    pub(super) inbox: Arc<InboxState>,
}

/// B50, B51: told the Message ids an Agent's transcript records as received from another session.
pub type InboxObserver = Arc<dyn Fn(&str, &[u32]) + Send + Sync>;

#[derive(Default)]
pub(super) struct InboxState {
    /// B51: the latest check of each Agent's inbox, the Rail fact `inbox`.
    pub(super) facts: Mutex<HashMap<String, contracts::agent::Inbox>>,
    /// B50(c): the transcript file of each Agent and how far it has been read.
    pub(super) transcripts: Mutex<HashMap<String, (PathBuf, u64)>>,
    /// B50(c): a watch on the folder of each Agent's transcript; the file system says when the file
    /// grew, so no timer decides `delivered`.
    watches: Mutex<HashMap<String, notify::RecommendedWatcher>>,
    pub(super) observer: std::sync::OnceLock<InboxObserver>,
}

impl InboxState {
    pub(super) fn fact(&self, agent: &str) -> Option<contracts::agent::Inbox> {
        let facts = self.facts.lock().unwrap_or_else(|e| e.into_inner());
        facts.get(agent).copied()
    }

    pub(super) fn set(&self, agent: &str, state: contracts::agent::Inbox) {
        let mut facts = self.facts.lock().unwrap_or_else(|e| e.into_inner());
        facts.insert(agent.to_owned(), state);
    }

    /// B50(c): read the transcript again from its start, for an entry that Claude Code took up
    /// before the Message it carries was in flight.
    pub(super) fn rescan(self: &Arc<Self>, agent: &str) {
        {
            let mut files = self.transcripts.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((_, offset)) = files.get_mut(agent) {
                *offset = 0;
            }
        }
        self.scan(agent, &serde_json::Value::Null);
    }

    /// B50(c): scan again whenever the transcript at `path` changes. Claude Code records a taken-up
    /// message after the hook that follows its turn, so a Signal alone can come too early. The
    /// file is made after `SessionStart`, so the watch starts at the first scan that finds it.
    fn watch(self: &Arc<Self>, agent: &str, path: &Path) {
        use notify::Watcher;
        let watching = self
            .watches
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(agent);
        // Events name the real path, and a temp folder is a link on macOS.
        let Ok(file) = path.canonicalize() else {
            return;
        };
        if watching {
            return;
        }
        let folder = file.parent().unwrap_or(&file).to_owned();
        let (state, name) = (Arc::downgrade(self), agent.to_owned());
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if event.is_ok_and(|event| {
                (event.kind.is_modify() || event.kind.is_create()) && event.paths.contains(&file)
            }) && let Some(state) = state.upgrade()
            {
                state.scan(&name, &serde_json::Value::Null);
            }
        })
        .and_then(|mut watcher| {
            watcher.watch(&folder, notify::RecursiveMode::NonRecursive)?;
            Ok(watcher)
        });
        match watcher {
            Ok(watcher) => {
                let mut watches = self.watches.lock().unwrap_or_else(|e| e.into_inner());
                watches.insert(agent.to_owned(), watcher);
            }
            Err(err) => eprintln!("rupd: inbox: cannot watch {}: {err}", folder.display()),
        }
    }

    /// B50(c): on an Agent's Signal, tell the observer the ids its transcript newly records.
    pub(super) fn scan(self: &Arc<Self>, agent: &str, payload: &serde_json::Value) {
        let Some(observer) = self.observer.get() else {
            return;
        };
        let (ids, ..) = {
            let mut files = self.transcripts.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(path) = claude_code::inbox::transcript_of(payload) {
                match files.get(agent) {
                    Some((known, _)) if *known == path => {}
                    _ => {
                        self.watches
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .remove(agent);
                        files.insert(agent.to_owned(), (path, 0));
                    }
                }
            }
            if let Some((path, _)) = files.get(agent) {
                self.watch(agent, path);
            }
            let Some((path, offset)) = files.get_mut(agent) else {
                return;
            };
            let (ids, next) = claude_code::inbox::recorded(path, *offset);
            *offset = next;
            (ids, next)
        };
        if !ids.is_empty() {
            observer(agent, &ids);
        }
    }
}

/// B39: `(agent id, submitted prompt)` to whether the prompt repeats Messages already delivered.
pub type PromptObserver = Arc<dyn Fn(&str, &str) -> bool + Send + Sync>;

impl Shared {
    /// E6: `agent.channelUp` from Agent `id`. Nothing to report when no program of the Agent
    /// runs, and a second call while the Channel is `up` emits nothing.
    pub(super) fn channel_up(&self, actor: &Actor, id: &str) -> Result<(), RpcError> {
        self.rail().node(id)?;
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
    pub(super) fn set_channel(&self, by: Actor, id: &str, attempt: &str, channel: Channel) {
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
    pub(super) fn watch_channel(self: &Arc<Self>, id: &str, attempt: &str) {
        let (shared, id, attempt) = (Arc::clone(self), id.to_owned(), attempt.to_owned());
        tokio::spawn(async move {
            tokio::time::sleep(shared.events.channel_deadline).await;
            shared.set_channel(Actor::daemon(), &id, &attempt, Channel::Missing);
        });
    }

    /// Fold `observation` into Agent `id`'s Status. A change is announced as `agent.status`, and
    /// the first `SessionStart` Steers the prompt the Agent was spawned with. Returns when, on `clock`, the
    /// adapter wants its next `Observation::Tick`.
    pub(super) fn observe(
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
                                EventData::AgentStatus(Box::new(StatusEvent {
                                    id: id.to_owned(),
                                    attempt: run.attempt.clone(),
                                    status_revision: run.status_revision.to_string(),
                                    status,
                                })),
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
                Some(Slot::Resuming { starred, .. }) => {
                    if let Observation::Title(title) = &observation {
                        *starred |= title.starts_with('✳');
                        return Ok(None);
                    }
                    let Observation::Signal(payload) = observation else {
                        return Err(RpcError::not_found(format!("agent {id}")));
                    };
                    self.finish_resuming(rail.as_deref_mut(), &mut runs, id, actor, payload)?
                }
                None => return Err(RpcError::not_found(format!("agent {id}"))),
                Some(Slot::Closing | Slot::ResumeCleanup { .. }) => {
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
    pub(super) fn apply(
        &self,
        mut rail: Option<&mut rail::Rail>,
        run: &mut Run,
        id: &str,
        actor: &Actor,
        observation: Observation,
        first_prompt: Option<Option<String>>,
    ) -> Result<Applied, RpcError> {
        if let Observation::Signal(payload) = &observation
            && claude_code::ends_decision(payload)
            && self.decisions.waits_in_terminal(id)
        {
            return Ok((None, run.adapter.tick_at()));
        }
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
        let session_start = matches!(&observation, Observation::Signal(payload)
            if claude_code::is_session_start(payload));
        let star = matches!(&observation, Observation::Title(title) if title.starts_with('✳'));
        if session_start {
            if !run.session_started && run.prompt.is_some() {
                self.refuse_unless_gate_opens(id, &run.attempt);
            }
            run.session_started = true;
        }
        if star {
            run.starred = true;
        }
        // H11: the Steer's wait ends with the Signal that names it, or with the Agent.
        if matches!(&observation, Observation::Signal(payload)
            if claude_code::submitted_prompt(payload).is_some())
        {
            // H12 permits a late submission after refusal. Keep that acknowledgement even
            // after the Steer's timeout so a coordinator retry need not Resume again.
            if matches!(run.first_prompt, Some(FirstPrompt::Written)) {
                run.first_prompt = Some(FirstPrompt::Acknowledged);
            }
            if let Some(steer) = run.steer.take() {
                let _ = steer.send(());
            }
        } else if matches!(observation, Observation::Exit { .. } | Observation::Stopped) {
            run.steer = None;
            // No writer owns a prompt still waiting for H19. Drop it before settling;
            // a released/queued writer must publish its own completion instead.
            if run.prompt.take().is_some() {
                run.first_pending = false;
                if let Some(completion) = &run.first_completion {
                    completion.send_replace(Some(FirstSteerCompletion::Refused));
                }
            }
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
            let status = waiting_on(&status, self.wait_of(id));
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
            self.bus
                .emit(actor.clone(), EventData::AgentStatus(event.into()));
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
        if (session_start || star)
            && let Some(observer) = &self.events.gate_observer
        {
            observer(id, prompt.is_some());
        }
        Ok((prompt, run.adapter.tick_at()))
    }
}

/// E6: the Channel of the live Attempt in `slot`; `None` when no program runs (a Workstream whose Door
/// never started, an Agent with no live Terminal). An entry of an earlier Attempt reads `pending`.
pub(super) fn channel_in(
    channels: &HashMap<String, (String, Channel)>,
    id: &str,
    slot: Option<&Slot>,
) -> Option<Channel> {
    let slot = slot?;
    let live = match slot {
        Slot::Running(run) => !run.ended && !run.closing,
        Slot::Starting { .. } | Slot::Resuming { .. } => true,
        Slot::Closing | Slot::ResumeCleanup { .. } => false,
    };
    live.then(|| match channels.get(id) {
        Some((attempt, channel)) if attempt == slot.attempt() => *channel,
        _ => Channel::Pending,
    })
}

/// E6: `ROUNDUP_CHANNEL_DEADLINE_MS` when it is a whole number of milliseconds, else
/// `CHANNEL_DEADLINE`.
pub(super) fn channel_deadline_from_env() -> Duration {
    match std::env::var("ROUNDUP_CHANNEL_DEADLINE_MS") {
        Ok(ms) => ms.parse().map(Duration::from_millis).unwrap_or_else(|_| {
            eprintln!("agents: ROUNDUP_CHANNEL_DEADLINE_MS={ms:?} is not milliseconds; ignored");
            CHANNEL_DEADLINE
        }),
        Err(_) => CHANNEL_DEADLINE,
    }
}

/// Feed one Terminal's titles and its exit to the Agent behind it, and a Tick at the time its
/// adapter asks for one. Only a held star asks, so the watcher of an idle Agent never wakes.
pub(super) async fn watch(
    shared: Arc<Shared>,
    id: String,
    attempt: String,
    mut events: Receiver<EventData>,
) {
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

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    const TRIALS: usize = 20_000;
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use contracts::terminal::ExitedEvent;
    use contracts::{Actor, EventData, Kind};

    use serde_json::json;

    use tokio::time::Instant;

    use crate::claude_code::ClaudeCode;
    use crate::{Observation, Run, Slot};

    use crate::tests::*;
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
            startup_context: true,
            attempt: "1".into(),
            status_revision: 1,
            closing: false,
            ended: false,
            adapter: ClaudeCode::starting(|| 0),
            prompt: None,
            first_pending: false,
            first_completion: None,
            first_prompt: None,
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
            startup_context: true,
            attempt: "1".into(),
            status_revision: 1,
            closing: false,
            ended: false,
            adapter: ClaudeCode::starting(|| 0),
            prompt: None,
            first_pending: false,
            first_completion: None,
            first_prompt: None,
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
                            startup_context: true,
                            attempt: "1".into(),
                            status_revision: 1,
                            closing: false,
                            ended: false,
                            adapter: ClaudeCode::starting(|| 0),
                            prompt: None,
                            first_pending: false,
                            first_completion: None,
                            first_prompt: None,
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
    #[tokio::test]
    async fn b37_a7_old_title_tick_exit_and_signal_do_not_change_a_new_run() {
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
            Observation::Signal(json!({"hook_event_name":"StopFailure"})),
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
}
