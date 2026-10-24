use super::*;

pub(super) struct State {
    /// H19: a test's observer of each gate decision; absent in production.
    pub(super) gate_observer: Option<GateObserver>,
    /// E6: how long after a start `agent.channelUp` may take before the Channel is `missing`.
    pub(super) channel_deadline: Duration,
    /// B39: told each submitted prompt; its answer says whether the hook must drop the prompt.
    /// B50, B51: what the Claude inbox of each Agent is and what its transcript has recorded.
    pub(super) inbox: Arc<InboxState>,
    /// U3780a: the current or last tool call of each Agent.
    pub(super) tool_calls: Mutex<HashMap<String, claude_code::toolcall::Tracked>>,
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
        let before = facts.insert(agent.to_owned(), state);
        if state == contracts::agent::Inbox::Mismatch && before != Some(state) {
            eprintln!(
                "agents: Agent {agent}: its Claude Code speaks a messaging protocol this build does not; notes to it stay pending"
            );
        }
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
            Err(err) => eprintln!("inbox: cannot watch {}: {err}", folder.display()),
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

impl Shared {
    /// E6: `agent.channelUp` from Agent `id`. Nothing to report when no program of the Agent
    /// runs, and a second call while the Channel is `up` emits nothing.
    pub(super) fn channel_up(&self, actor: &Actor, id: &str) -> Result<(), RpcError> {
        self.rail().node(id)?;
        if actor.kind != ActorKind::Agent || actor.id != id {
            return Err(RpcError::forbidden(format!(
                "only {} may report its Channel",
                self.rail().name(id)?
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
        let name = self.rail().name(id)?;
        let first_prompt = match &observation {
            Observation::Signal(payload) => claude_code::submitted_prompt(payload)
                .filter(|prompt| !order::is_clarification_steer(prompt))
                .map(name::from_prompt),
            _ => None,
        };
        // Taken before `runs`, as every other path does, so a rename and a prompt cannot deadlock.
        // U3566c: any Observation may end a turn, and the finish count is saved with its Status.
        let mut rail = Some(self.rail());
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
                return Err(RpcError::conflict(format!("{name} is stopping")));
            }
            match runs.get_mut(id) {
                Some(Slot::Running(run)) => {
                    if run.ended {
                        return Err(RpcError::not_found(format!("{name} has ended")));
                    }
                    // A21: a Signal naming a conversation of the current Attempt saves it
                    // before being applied; a failed save fails this call, settles the Status
                    // to `error` and stops the Terminal, since `agent.spawn` has already
                    // returned by the time a Running Agent can reach here (the parked window's
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
                                    ended: false,
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
                // A14: a Signal here is parked, in arrival order, for when the Agent is
                // registered, instead of `NOT_FOUND`; anything else cannot arrive this early
                // (`watch` only starts once the Agent is `Running`).
                Some(Slot::Starting { parked, .. }) => {
                    let Observation::Signal(payload) = observation else {
                        return Err(RpcError::not_found(&name));
                    };
                    hold(id, parked, actor, payload);
                    return Ok(None);
                }
                // A22: a non-ack Signal is parked exactly as `Starting`'s; the ack promotes to
                // `Running` and wakes the pending `agent.resume` (`finish_resuming`).
                Some(Slot::Resuming { starred, .. }) => {
                    if let Observation::Title(title) = &observation {
                        *starred |= title.starts_with('✳');
                        return Ok(None);
                    }
                    let Observation::Signal(payload) = observation else {
                        return Err(RpcError::not_found(&name));
                    };
                    self.finish_resuming(rail.as_deref_mut(), &mut runs, id, actor, payload)?
                }
                None => return Err(RpcError::not_found(&name)),
                Some(Slot::Closing | Slot::ResumeCleanup { .. }) => {
                    return Err(RpcError::conflict(format!("{name} is stopping")));
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
        let exited = matches!(observation, Observation::Exit { .. });
        if matches!(observation, Observation::Exit { .. } | Observation::Stopped) {
            self.timing
                .lock()
                .expect("measurement timing lock")
                .end_attempt(id, &run.attempt);
        }
        // H19: the first Steer waits for the first `SessionStart` and the first star title.
        let session_start = matches!(&observation, Observation::Signal(payload)
            if claude_code::is_session_start(payload));
        let star = matches!(&observation, Observation::Title(title) if title.starts_with('✳'));
        if session_start {
            if !run.session_started {
                self.timing
                    .lock()
                    .expect("measurement timing lock")
                    .acknowledge_attempt(id, &run.attempt);
            }
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
            && run
                .steer
                .as_ref()
                .and_then(|steer| steer.slash_written.as_ref())
                .is_none_or(|gate| *gate.lock().expect("slash-command write gate"))
        {
            // H12 permits a late submission after refusal. Keep that acknowledgement even
            // after the Steer's timeout so a coordinator retry need not Resume again.
            if matches!(run.first_prompt, Some(FirstPrompt::Written)) {
                run.first_prompt = Some(FirstPrompt::Acknowledged);
            }
            if let Some(steer) = run.steer.take() {
                let _ = steer.submitted.send(());
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
        let acks = matches!(observation, Observation::Title(_) | Observation::Tick);
        if matches!(observation, Observation::Exit { .. } | Observation::Stopped) {
            run.interrupt = None;
        }
        if let Observation::Signal(payload) = &observation {
            // Announced while `runs` is parked, as the Status is, so calls leave in arrival order.
            let mut calls = self
                .events
                .tool_calls
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let mut current = calls.remove(id);
            if claude_code::toolcall::fold(&mut current, payload, &run.cwd, store::now_ms())
                && let Some(tracked) = &current
            {
                self.bus.emit(
                    actor.clone(),
                    EventData::AgentToolCall(Box::new(contracts::agent::ToolCallEvent {
                        id: id.to_owned(),
                        call: tracked.call.clone(),
                    })),
                );
            }
            if let Some(tracked) = current {
                calls.insert(id.to_owned(), tracked);
            }
        }
        let was_kind = run.adapter.status().map(|status| status.kind);
        let was_finished = run.adapter.finished();
        let signal = matches!(observation, Observation::Signal(_));
        let changed = run.adapter.observe(observation).or_else(|| {
            if !exited || run.ended {
                return None;
            }
            let mut status = run.adapter.status()?.clone();
            status.since = store::now_ms();
            Some(status)
        });
        if signal
            && (session_start
                || changed
                    .as_ref()
                    .is_some_and(|status| Some(status.kind) != was_kind))
            && run
                .steer
                .as_ref()
                .and_then(|steer| steer.slash_written.as_ref())
                .is_some_and(|gate| *gate.lock().expect("slash-command write gate"))
            && let Some(steer) = run.steer.take()
        {
            let _ = steer.submitted.send(());
        }
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
        // U3566c: a counted finish is saved before its Status is published.
        if run.adapter.finished() != was_finished
            && let Some(rail) = rail.as_deref_mut()
        {
            rail.set_finished(id, run.adapter.finished())?;
        }
        // Announced while `runs` is parked, so announcements leave in the order the Status
        // changed; `emit` never blocks.
        if let Some(status) = changed {
            let status = waiting_on(&status, self.wait_of(id));
            run.status_revision = run
                .status_revision
                .checked_add(1)
                .expect("Status revision exhausted");
            let event = StatusEvent {
                ended: run.ended,
                id: id.to_owned(),
                attempt: run.attempt.clone(),
                status_revision: run.status_revision.to_string(),
                status,
            };
            if !run.ended {
                self.timing
                    .lock()
                    .expect("measurement timing lock")
                    .observe_status(&event, run.session_started);
            }
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
            let before = rail.node(id)?.name;
            let node = rail.rename(id, &name::unique(&name, taken))?;
            // The Rail lock also orders publication against RPC renames.
            if node.name != before {
                self.bus.emit(
                    actor.clone(),
                    EventData::RailRenamed {
                        id: node.id,
                        name: node.name,
                    },
                );
            }
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

/// `name` when it is a whole number of milliseconds, else `default`.
fn bound_from_env(name: &str, default: Duration) -> Duration {
    match std::env::var(name) {
        Ok(ms) => ms.parse().map(Duration::from_millis).unwrap_or_else(|_| {
            eprintln!("agents: {name}={ms:?} is not milliseconds; ignored");
            default
        }),
        Err(_) => default,
    }
}

/// E6: `ROUNDUP_CHANNEL_DEADLINE_MS`, else `CHANNEL_DEADLINE`.
pub(super) fn channel_deadline_from_env() -> Duration {
    bound_from_env("ROUNDUP_CHANNEL_DEADLINE_MS", CHANNEL_DEADLINE)
}

/// H13: `ROUNDUP_INTERRUPT_BOUND_MS`, else `INTERRUPT_BOUND`.
pub(super) fn interrupt_bound_from_env() -> Duration {
    bound_from_env("ROUNDUP_INTERRUPT_BOUND_MS", INTERRUPT_BOUND)
}

/// Feed one Terminal's titles and its exit to the Agent behind it, and a Tick at the time its
/// adapter asks for one. Only a parked star asks, so the watcher of an idle Agent never wakes.
pub(super) async fn watch(
    shared: Arc<Shared>,
    id: String,
    attempt: String,
    mut events: Receiver<Arc<EventData>>,
) {
    let mut tick: Option<Instant> = None;
    loop {
        let observation = tokio::select! {
            event = events.recv() => match event.map(Arc::unwrap_or_clone) {
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
mod tests;
