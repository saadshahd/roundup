use super::*;

impl Agents {
    /// `project` is the Project folder and `dir` its data folder (N1). `bus` is for events no call
    /// caused. Claude Code is `ROUNDUP_CLAUDE_BIN` (default `claude`); `git` is whatever `PATH` finds.
    pub fn open(
        project: &Path,
        dir: &Path,
        bus: Bus,
        terminals: Arc<Terminals>,
    ) -> Result<Self, OpenError> {
        Self::open_in(
            project,
            dir,
            bus,
            terminals,
            Launcher::from_env()?,
            worktree::Git::from_env(),
        )
    }

    /// Like `open_in`, with the Project folder taken as `dir`'s parent.
    pub fn open_with(
        dir: &Path,
        bus: Bus,
        terminals: Arc<Terminals>,
        launcher: Launcher,
        git: worktree::Git,
    ) -> Result<Self, OpenError> {
        let project = dir.parent().unwrap_or(dir);
        Self::open_in(project, dir, bus, terminals, launcher, git)
    }

    pub fn open_in(
        project: &Path,
        dir: &Path,
        bus: Bus,
        terminals: Arc<Terminals>,
        launcher: Launcher,
        git: worktree::Git,
    ) -> Result<Self, OpenError> {
        let mut rail = rail::Rail::open(&dir.join(if dir.join("project.db").exists() {
            "project.db"
        } else {
            "agents.db"
        }))?;
        let recovery = git.bounded();
        for (node, plan, ready) in rail.provisioning()? {
            let Some(plan) = plan else {
                eprintln!("agents: {}: {UNPROVEN}; kept", node.id);
                continue;
            };
            let settled = if ready {
                recovery
                    .finish(project, &plan)
                    .and_then(|()| rail.clear_provisioning_owner(&node.id))
            } else {
                recovery.recover(project, &plan).and_then(|()| {
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
        let decisions = decision::Decisions::new(
            None,
            Duration::from_secs(claude_code::PERMISSION_TIMEOUT_SECS),
        )
        .with_store(&dir.join(if dir.join("project.db").exists() {
            "project.db"
        } else {
            "agents.db"
        }))?;
        Ok(Self {
            shared: Arc::new_cyclic(|me| Shared {
                git,
                project_dir: project.to_owned(),
                rail: Mutex::new(rail),
                runs: Mutex::new(HashMap::new()),
                door_lifecycle: tokio::sync::RwLock::new(()),
                bus,
                ending: ending::State {
                    kill: Arc::clone(&terminals) as Arc<dyn KillsTerminals>,
                },
                terminals,
                clock: Arc::new(now_ms),
                rail_view: rail_view::State { opened: now_ms() },
                decisions,
                me: me.clone(),
                resume_effect: None,
                steer_bound: STEER_BOUND,
                steer: crate::SteerSettings {
                    before_steer: None,
                    interrupt_bound: INTERRUPT_BOUND,
                },
                events: events::State {
                    gate_observer: None,
                    channel_deadline: channel_deadline_from_env(),
                    prompt_observer: std::sync::OnceLock::new(),
                    inbox: Default::default(),
                },
                channels: Mutex::new(HashMap::new()),
                scan: Scan::new(scan_interval_from_env()),
                strays: Mutex::new(HashMap::new()),
                takeover: None,
                context: None,
                waits: Mutex::new(HashMap::new()),
            }),
            dir: dir.to_owned(),
            launcher,
        })
    }

    /// Supply the Terminal shutdown dependency before any Agents are shared.
    #[must_use]
    pub fn with_terminal_kills<F, Fut>(self, kill: F) -> Self
    where
        F: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), RpcError>> + Send,
    {
        struct Kills<F>(F);
        #[async_trait]
        impl<F, Fut> KillsTerminals for Kills<F>
        where
            F: Fn(String) -> Fut + Send + Sync,
            Fut: std::future::Future<Output = Result<(), RpcError>> + Send,
        {
            async fn kill(&self, id: &str) -> Result<(), RpcError> {
                (self.0)(id.to_owned()).await
            }
        }
        self.configure(|shared| shared.ending.kill = Arc::new(Kills(kill)))
    }

    /// B6: observe a Resume's Terminal before recording its id, after recording it, and
    /// before cleanup (phases `claim`, `claimed`, `cleanup`). Runs without Agent locks on a
    /// blocking thread; a cleanup refusal leaves the Terminal untouched.
    #[must_use]
    pub fn with_resume_effect(
        self,
        observer: impl Fn(&str, &str) -> Result<(), RpcError> + Send + Sync + 'static,
    ) -> Self {
        self.configure(|shared| shared.resume_effect = Some(Arc::new(observer)))
    }

    /// Change a setting before any call has shared the Agents: the only holder of `shared` is
    /// `self`, plus its own `me`.
    pub(super) fn configure(mut self, set: impl FnOnce(&mut Shared)) -> Self {
        let mut shared = Arc::into_inner(self.shared).expect("the Agents are not shared yet");
        set(&mut shared);
        self.shared = Arc::new_cyclic(|me| {
            shared.me = me.clone();
            shared
        });
        self
    }

    /// H3, H4: answers need `proof`; the App holds it. Without one every `decision.answer` is
    /// `FORBIDDEN`. Call it before the first call is made.
    #[must_use]
    pub fn with_proof(self, proof: Option<String>) -> Self {
        self.configure(|shared| shared.decisions.set_proof(proof))
    }

    /// E3: supply the Context text held by the Daemon, without giving Agents a Todos dependency.
    /// Read once for the first Steer; a failed or timed-out read leaves that Steer unchanged.
    #[must_use]
    pub fn with_context<F, Fut>(self, read: F) -> Self
    where
        F: Fn(Vec<RailNode>, String) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<String, RpcError>> + Send + 'static,
    {
        self.configure(|shared| {
            shared.context = Some(Arc::new(move |nodes, id| Box::pin(read(nodes, id))))
        })
    }

    /// O4: `active` says whether a Takeover of an Agent is active. Call it before the first call.
    #[must_use]
    pub fn with_takeover(self, active: impl Fn(&str) -> bool + Send + Sync + 'static) -> Self {
        self.configure(|shared| shared.takeover = Some(Arc::new(active)))
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

    /// H19: tell `observer`, for each `SessionStart` Signal and star title an Agent's Run folds,
    /// whether that Observation released the Agent's first prompt to its Steer. It runs under
    /// the Agents' locks, so it must not call back into them. Call it before the first call is
    /// made.
    #[must_use]
    pub fn with_gate_observer(self, observer: impl Fn(&str, bool) + Send + Sync + 'static) -> Self {
        self.configure(|shared| shared.events.gate_observer = Some(Arc::new(observer)))
    }

    /// B39: tell `observer` the text of each prompt an Agent submits (`UserPromptSubmit`); `true`
    /// makes `agent.signal` answer that the hook drops the prompt. Set once, after the Daemon
    /// opens Messages.
    pub fn observe_prompts(&self, observer: PromptObserver) {
        let _ = self.shared.events.prompt_observer.set(observer);
    }

    /// B50(c): tell `observer` the Message ids an Agent's transcript records as another
    /// session's message, as its Signals arrive. Set once, after the Daemon opens Messages.
    pub fn observe_inbox(&self, observer: crate::InboxObserver) {
        let _ = self.shared.events.inbox.observer.set(observer);
    }

    /// B51: check Agent `id`'s Claude inbox and keep the outcome as its Rail fact `inbox`.
    pub async fn inbox_check(&self, id: &str) -> contracts::agent::Inbox {
        let state = self.launcher.inbox_check(id).await;
        self.shared.events.inbox.set(id, state);
        state
    }

    /// B50: write notes to Agent `id`'s Claude inbox; a failure is the state to hold them for,
    /// and is the Rail fact too.
    pub fn inbox_write(
        &self,
        id: &str,
        notes: &[claude_code::inbox::Entry],
    ) -> Result<(), contracts::agent::Inbox> {
        self.launcher.inbox_write(id, notes).inspect_err(|state| {
            self.shared.events.inbox.set(id, *state);
        })?;
        // Claude Code may have taken the entries up before they were in flight.
        self.shared.events.inbox.rescan(id);
        Ok(())
    }

    /// H13: wait `bound` instead of `INTERRUPT_BOUND` for an Interrupt's title. Call it before
    /// the first call is made.
    #[must_use]
    pub fn with_interrupt_bound(self, bound: Duration) -> Self {
        self.configure(|shared| shared.steer.interrupt_bound = bound)
    }

    /// B6: observe or refuse a Steer immediately before its physical Terminal write.
    /// Configure before sharing Agents; the observer runs on the Terminal input thread
    /// and must not reenter its input queue.
    #[must_use]
    pub fn with_steer_effect(
        self,
        before: impl Fn(&str) -> Result<(), RpcError> + Send + Sync + 'static,
    ) -> Self {
        self.configure(|shared| shared.steer.before_steer = Some(Arc::new(before)))
    }
}

/// Milliseconds since the Unix epoch.
pub(super) fn now_ms() -> i64 {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(since_epoch.as_millis()).unwrap_or(i64::MAX)
}

#[cfg(test)]
pub(super) mod tests {
    use crate::{ending, events, rail_view};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use async_trait::async_trait;
    use contracts::agent::NodeKind;
    use contracts::terminal::TitleEvent;
    use contracts::{Actor, Event, EventData, Kind};
    use rpc::Bus;
    use serde_json::json;
    use terminal::Terminals;
    use tokio::sync::broadcast;
    use tokio::task::JoinHandle;
    use tokio::time::Instant;

    use crate::claude_code::ClaudeCode;
    use crate::{Clock, KillsTerminals, Observation, Run, Shared, Slot, rail, watch, worktree};

    pub(crate) const PATIENCE: Duration = Duration::from_secs(60);
    /// Spelled out, not `STAR_HOLD`, so a changed hold fails these tests.
    pub(crate) const HOLD: Duration = Duration::from_millis(200);
    /// Agent `1`, watched over a stand-in for its Terminal's events, on tokio's clock, which these
    /// tests pause.
    pub(crate) struct Watched {
        pub(crate) _dir: tempfile::TempDir,
        pub(crate) shared: Arc<Shared>,
        pub(crate) terminal: broadcast::Sender<EventData>,
        pub(crate) events: broadcast::Receiver<Event>,
        pub(crate) watcher: JoinHandle<()>,
    }

    impl Watched {
        pub(crate) fn start() -> Self {
            let begun = Instant::now();
            let clock: Clock = Arc::new(move || begun.elapsed().as_millis() as i64);
            let adapter_clock = Arc::clone(&clock);
            let (dir, bus, shared) = shared_over_temp_dir(clock);
            let run = Run {
                startup_context: true,
                attempt: "1".into(),
                status_revision: 1,
                closing: false,
                ended: false,
                adapter: ClaudeCode::starting(move || adapter_clock()),
                prompt: None,
                first_pending: false,
                first_completion: None,
                first_prompt: None,
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

        pub(crate) fn signal(&self, event: &str) {
            let payload = json!({"hook_event_name": event, "tool_name": "Bash"});
            self.shared
                .observe(Actor::daemon(), "1", "1", Observation::Signal(payload))
                .unwrap();
        }

        pub(crate) fn send(&self, data: EventData) {
            self.terminal.send(data).unwrap();
        }

        pub(crate) fn star(&self) {
            self.send(EventData::TerminalTitle(TitleEvent {
                id: "1".into(),
                title: "\u{2733} Claude Code".into(),
            }));
        }

        /// The Kind of the next `agent.status`.
        pub(crate) async fn next_kind(&mut self) -> Kind {
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
        pub(crate) fn has_news(&mut self) -> bool {
            std::iter::from_fn(|| self.events.try_recv().ok())
                .any(|event| matches!(event.data, EventData::AgentStatus(_)))
        }
    }
    pub(crate) struct FailingKill;

    #[async_trait]
    impl KillsTerminals for FailingKill {
        async fn kill(&self, _id: &str) -> Result<(), rpc::RpcError> {
            Err(rpc::RpcError::internal("the kill failed"))
        }
    }

    pub(crate) struct FlakyKill(std::sync::atomic::AtomicBool);

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

    pub(crate) fn owner_agents(shared: Arc<Shared>, dir: &std::path::Path) -> crate::Agents {
        crate::Agents {
            shared,
            dir: dir.to_owned(),
            launcher: crate::Launcher::new(
                "unused",
                dir.join("claude.json"),
                dir.join("rup"),
                None,
            ),
        }
    }

    /// Each transition competes with an input record, then a fresh read observes its result.
    pub(crate) fn owner_transition(
        initially_attached: bool,
        transition: impl FnOnce(&Shared, &str, &std::path::Path) + Send,
        expected: Option<(&str, &str)>,
    ) {
        let (dir, _, shared) = shared_over_temp_dir(Arc::new(|| 0));
        let id = shared
            .rail()
            .insert(
                NodeKind::Agent,
                "receiver",
                None,
                initially_attached.then_some("terminal"),
            )
            .unwrap()
            .id;
        let attempt = shared.rail().allocate_attempt(&id).unwrap();
        shared.mark_starting(&id, attempt);
        let agents = owner_agents(Arc::clone(&shared), dir.path());
        std::thread::scope(|scope| {
            let (start, started) = std::sync::mpsc::sync_channel(0);
            let (done, completed) = std::sync::mpsc::channel();
            let mut change = None;
            agents
                .with_terminal_owner("terminal", |owner| {
                    assert_eq!(owner.is_some(), initially_attached);
                    let shared = Arc::clone(&shared);
                    let id = id.clone();
                    let path = dir.path();
                    change = Some(scope.spawn(move || {
                        // Successful try_lock would mean the ownership cut is not protected.
                        assert!(matches!(
                            shared.rail.try_lock(),
                            Err(std::sync::TryLockError::WouldBlock)
                        ));
                        assert!(matches!(
                            shared.runs.try_lock(),
                            Err(std::sync::TryLockError::WouldBlock)
                        ));
                        start.send(()).unwrap();
                        transition(&shared, &id, path);
                        done.send(()).unwrap();
                    }));
                    started.recv_timeout(PATIENCE).unwrap();
                    assert!(matches!(
                        completed.try_recv(),
                        Err(std::sync::mpsc::TryRecvError::Empty)
                    ));
                    Ok(())
                })
                .unwrap();
            completed.recv_timeout(PATIENCE).unwrap();
            change.unwrap().join().unwrap();
        });
        agents
            .with_terminal_owner("terminal", |owner| {
                assert_eq!(
                    owner.map(|o| (o.id.as_str(), o.attempt.as_deref().unwrap())),
                    expected
                );
                Ok(())
            })
            .unwrap();
    }

    /// A `Shared` wired to a fresh temp-dir Rail and Terminals, paired with the `Bus` it was built
    /// from so a caller can subscribe to the same events.
    pub(crate) fn shared_over_temp_dir(clock: Clock) -> (tempfile::TempDir, Bus, Arc<Shared>) {
        let dir = tempfile::tempdir().unwrap();
        let bus = Bus::new();
        let terminals = Arc::new(Terminals::open(dir.path(), bus.clone()).unwrap());
        let shared = Arc::new(Shared {
            git: worktree::Git::from_env(),
            project_dir: dir.path().to_owned(),
            rail: Mutex::new(rail::Rail::open(&dir.path().join("agents.db")).unwrap()),
            runs: Mutex::new(HashMap::new()),
            door_lifecycle: tokio::sync::RwLock::new(()),
            bus: bus.clone(),
            ending: ending::State {
                kill: Arc::clone(&terminals) as Arc<dyn crate::KillsTerminals>,
            },
            terminals,
            clock,
            rail_view: rail_view::State { opened: 0 },
            decisions: crate::decision::Decisions::new(None, Duration::from_secs(4)),
            me: std::sync::Weak::new(),
            resume_effect: None,
            steer_bound: crate::STEER_BOUND,
            steer: crate::SteerSettings {
                before_steer: None,
                interrupt_bound: crate::INTERRUPT_BOUND,
            },
            events: events::State {
                gate_observer: None,
                channel_deadline: crate::CHANNEL_DEADLINE,
                prompt_observer: std::sync::OnceLock::new(),
                inbox: Default::default(),
            },
            channels: Mutex::new(HashMap::new()),
            scan: crate::Scan::new(crate::SCAN_INTERVAL),
            strays: Mutex::new(HashMap::new()),
            takeover: None,
            context: None,
            waits: Mutex::new(HashMap::new()),
        });
        (dir, bus, shared)
    }

    /// A second connection holding a write transaction makes a write through `Rail`'s own
    /// connection fail with a real SQLite error once rusqlite's default 5 s `busy_timeout`
    /// elapses, without reaching into `Rail`'s private `db` field.
    pub(crate) fn lock_db_for_writes(dir: &std::path::Path) -> rusqlite::Connection {
        let lock = rusqlite::Connection::open(dir.join("agents.db")).unwrap();
        lock.execute_batch("BEGIN IMMEDIATE;").unwrap();
        lock
    }

    /// `Shared` wired like `shared_over_temp_dir`, but every kill fails.
    pub(crate) fn shared_with_a_failing_kill() -> Arc<Shared> {
        let dir = tempfile::tempdir().unwrap();
        let bus = Bus::new();
        Arc::new(Shared {
            git: worktree::Git::from_env(),
            project_dir: dir.path().to_owned(),
            rail: Mutex::new(rail::Rail::open(&dir.path().join("agents.db")).unwrap()),
            runs: Mutex::new(HashMap::new()),
            door_lifecycle: tokio::sync::RwLock::new(()),
            bus: bus.clone(),
            terminals: Arc::new(Terminals::open(dir.path(), bus).unwrap()),
            ending: ending::State {
                kill: Arc::new(FailingKill),
            },
            clock: Arc::new(|| 0),
            rail_view: rail_view::State { opened: 0 },
            decisions: crate::decision::Decisions::new(None, Duration::from_secs(4)),
            me: std::sync::Weak::new(),
            resume_effect: None,
            steer_bound: crate::STEER_BOUND,
            steer: crate::SteerSettings {
                before_steer: None,
                interrupt_bound: crate::INTERRUPT_BOUND,
            },
            events: events::State {
                gate_observer: None,
                channel_deadline: crate::CHANNEL_DEADLINE,
                prompt_observer: std::sync::OnceLock::new(),
                inbox: Default::default(),
            },
            channels: Mutex::new(HashMap::new()),
            scan: crate::Scan::new(crate::SCAN_INTERVAL),
            strays: Mutex::new(HashMap::new()),
            takeover: None,
            context: None,
            waits: Mutex::new(HashMap::new()),
        })
    }

    /// A Running Agent node, with no watcher: the tests here drive `stop`/`remove` directly, so
    /// nothing reaps its Terminal's events.
    pub(crate) fn running_agent(shared: &Shared, parent: Option<&str>) -> String {
        let id = shared
            .rail()
            .insert(NodeKind::Agent, "a", parent, Some("1"))
            .unwrap()
            .id;
        shared.runs().insert(
            id.clone(),
            Slot::Running(Run {
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
                named: true,
                terminal_id: "1".into(),
                cwd: "/".into(),
            }),
        );
        id
    }

    /// A plain Agent node with a Terminal, with no watcher, for a test that drives `remove` directly.
    pub(crate) fn terminal_node(shared: &Shared, parent: Option<&str>) -> String {
        shared
            .rail()
            .insert(NodeKind::Agent, "t", parent, Some("1"))
            .unwrap()
            .id
    }

    /// `Shared` wired like `shared_with_a_failing_kill`, but its kill succeeds from the second
    /// call on, so a retried `remove` can succeed.
    pub(crate) fn shared_with_a_flaky_kill() -> (tempfile::TempDir, Arc<Shared>) {
        let dir = tempfile::tempdir().unwrap();
        let bus = Bus::new();
        let shared = Arc::new(Shared {
            git: worktree::Git::from_env(),
            project_dir: dir.path().to_owned(),
            rail: Mutex::new(rail::Rail::open(&dir.path().join("agents.db")).unwrap()),
            runs: Mutex::new(HashMap::new()),
            door_lifecycle: tokio::sync::RwLock::new(()),
            bus: bus.clone(),
            terminals: Arc::new(Terminals::open(dir.path(), bus).unwrap()),
            ending: ending::State {
                kill: Arc::new(FlakyKill(std::sync::atomic::AtomicBool::new(false))),
            },
            clock: Arc::new(|| 0),
            rail_view: rail_view::State { opened: 0 },
            decisions: crate::decision::Decisions::new(None, Duration::from_secs(4)),
            me: std::sync::Weak::new(),
            resume_effect: None,
            steer_bound: crate::STEER_BOUND,
            steer: crate::SteerSettings {
                before_steer: None,
                interrupt_bound: crate::INTERRUPT_BOUND,
            },
            events: events::State {
                gate_observer: None,
                channel_deadline: crate::CHANNEL_DEADLINE,
                prompt_observer: std::sync::OnceLock::new(),
                inbox: Default::default(),
            },
            channels: Mutex::new(HashMap::new()),
            scan: crate::Scan::new(crate::SCAN_INTERVAL),
            strays: Mutex::new(HashMap::new()),
            takeover: None,
            context: None,
            waits: Mutex::new(HashMap::new()),
        });
        (dir, shared)
    }

    /// H7(c): `elapsed_ms` after the hook began, on an injected clock and the 4 s `timeout` the
    /// Decisions are built with, the hook's connection closes. Returns the events and the Kind.
    pub(crate) async fn hook_closes_after(
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
}
