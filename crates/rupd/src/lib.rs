//! The Daemon: every other part of roundup is a client of it.
//! It owns the connection loop, the module router, the event bus and the Provenance log.

mod attachment;
mod builtin;
mod clear;
mod context;
pub mod data;
mod decision_notes;
mod digest;
mod event_log;
mod handshake;
mod output;
mod removal;
mod resize;
mod resources;
mod rewind;
mod storage;
mod terminal_question;
mod waits;
mod wake;

pub use handshake::{HANDSHAKE_BOUND, Handshake, HandshakeError, read_handshake};

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::Arc;

use contracts::agent::NodeId;
use contracts::terminal::TerminalInfo;
use contracts::{Actor, ActorKind};
use provenance::Touches;
use rpc::{Bus, Ctx, Module, OpenError, RpcError, code};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;

/// How many list-and-kill rounds shutdown makes before it gives up on a stream of new Terminals.
const STOP_ROUNDS: usize = 5;

pub struct Daemon {
    modules: HashMap<&'static str, Arc<dyn Module>>,
    bus: Bus,
    todos: Arc<todos::Todos>,
    pads: Arc<pads::Pads>,
    /// B6: spawn publication and its Door-owned first Steer are registered together.
    door_first_steers: Arc<tokio::sync::Mutex<HashMap<String, String>>>,
    held_actions: resize::HeldActions,
    /// B6: Takeover acquisition excludes in-flight Door effects on this child.
    child_effects: std::sync::Mutex<HashMap<String, std::sync::Weak<tokio::sync::RwLock<()>>>>,
    /// A31: the Project folder, where the Skills of an Agent with no Worktree are read.
    project: std::path::PathBuf,
    touches: Arc<Touches>,
    /// B19: which Pads each Door's earlier digests already named.
    digests: Arc<messages::digest::Seen>,
    /// B21, B22: where a push is sent from, once `open` has made it.
    sender: Arc<std::sync::OnceLock<Arc<messages::Messages>>>,
    /// A32: the clears under way, and what they act through.
    clears: clear::Clears,
    /// A35: the Marks a rewind reads, and the rewinds under way.
    checkpoints: Arc<checkpoints::Checkpoints>,
    rewinding: rewind::Rewinding,
    /// B21: subscribed at `open`, so no change is missed before the pusher runs; taken once, when
    /// a Tokio runtime is there to run it.
    pushes: std::sync::Mutex<Option<tokio::sync::broadcast::Receiver<contracts::Event>>>,
    /// B41: subscribed at `open`, taken once with `pushes`.
    wake_events: std::sync::Mutex<Option<tokio::sync::broadcast::Receiver<contracts::Event>>>,
    /// B41: serialises the decisions on Wake requests.
    wake_gate: Arc<tokio::sync::Mutex<wake::Gate>>,
    /// H21: subscribed at `open`, taken once with `pushes`.
    notes: std::sync::Mutex<Option<tokio::sync::broadcast::Receiver<contracts::Event>>>,
    /// H21: knows the Agent behind a calling process.
    agents: Arc<agents::Agents>,
    /// X14: where a connection's selected output streams come from.
    terminals: Arc<terminal::Terminals>,
    /// A33: like `pushes`, with what keeps the Agents' waits current.
    waits: std::sync::Mutex<Option<Waiter>>,
    /// U419: where `attachment.save` keeps a pasted image.
    attachments: attachment::Attachments,
    /// D8: samples only inside `project.resources`.
    resources: resources::Sampler,
    /// B6's isolated observer pauses a Door action or guarded call before its effect; absent in production.
    prompt_effect_gate: Option<std::path::PathBuf>,
}

type Waiter = (
    tokio::sync::broadcast::Receiver<contracts::Event>,
    Arc<agents::Agents>,
    Arc<todos::Todos>,
);

/// Per-connection state.
#[derive(Clone)]
pub struct Conn {
    actor: Actor,
    /// H21: the pid of the process on the other end, as the OS reports it.
    peer: Option<u32>,
    events: mpsc::UnboundedSender<Value>,
    /// X14: where this connection's selected Terminal output goes; `None` for a call with no socket.
    output: Option<output::Port>,
}

impl Daemon {
    /// `dir` is the Project's data folder (N1); it is created if missing. The Project folder is
    /// taken as its parent.
    pub fn open(dir: &Path) -> Result<Self, OpenError> {
        Self::open_with_proof(dir, None)
    }

    /// Open `project`, keeping its data in `data` (N1) after moving an older Project's files
    /// there (N3).
    pub fn open_project(
        project: &Path,
        data: &Path,
        proof: Option<String>,
    ) -> Result<Self, OpenError> {
        let dir = data::project_data(data, project)?;
        data::migrate(project, &dir).map_err(OpenError::from)?;
        Self::open_in(project, &dir, proof)
    }

    /// Like `open`, with the `proof` an attached App handed over (H4, H18); without one every
    /// `decision.answer` is `FORBIDDEN`.
    pub fn open_with_proof(dir: &Path, proof: Option<String>) -> Result<Self, OpenError> {
        Self::open_in(dir.parent().unwrap_or(dir), dir, proof)
    }

    fn open_in(project: &Path, dir: &Path, proof: Option<String>) -> Result<Self, OpenError> {
        Self::open_stores(project, dir, proof, |agents| agents)
    }

    fn open_stores(
        project: &Path,
        dir: &Path,
        proof: Option<String>,
        configure: impl FnOnce(agents::Agents) -> agents::Agents,
    ) -> Result<Self, OpenError> {
        std::fs::create_dir_all(dir)?;
        storage::open(dir)?;
        let bus = Bus::with_log(Arc::new(event_log::SqliteLog::open(dir)?));
        let terminals = Arc::new(
            terminal::Terminals::open_with(dir, bus.clone(), &agents::claude_code::MARKERS)?
                .with_input_writer(terminal_input_observer())
                .with_exit_hold(terminal_exit_observer()),
        );
        // O4: the Agents ask whether a Takeover is active before they Steer a changed order; the
        // Messages that know are made after them.
        let messages = Arc::new(std::sync::OnceLock::<Arc<messages::Messages>>::new());
        let probe = Arc::clone(&messages);
        let touches = Arc::new(Touches::open(&dir.join("project.db"))?);
        let todos = Arc::new(todos::Todos::open(dir, bus.clone())?);
        let context_todos = Arc::clone(&todos);
        let context_bus = bus.clone();
        let context_touches = Arc::clone(&touches);
        let agents = Arc::new(
            configure(agents::Agents::open(
                project,
                dir,
                bus.clone(),
                Arc::clone(&terminals),
            )?)
            .with_proof(proof)
            .with_context(move |nodes, id| {
                let todos = Arc::clone(&context_todos);
                let ctx = Ctx {
                    actor: Actor::daemon(),
                    bus: context_bus.clone(),
                    touches: Arc::clone(&context_touches),
                };
                async move {
                    let todos: Vec<contracts::todo::Todo> =
                        serde_json::from_value(todos.call(&ctx, "todo.list", Value::Null).await?)
                            .map_err(RpcError::internal)?;
                    let context = context::compose(&nodes, &todos, &id)?;
                    Ok(context::text(&nodes, &context))
                }
            })
            .with_prompt_effect(prompt_effect_observer())
            .with_steer_effect(steer_effect_observer())
            .with_resume_effect(resume_effect_observer())
            .with_takeover(move |agent| {
                probe
                    .get()
                    .is_some_and(|messages| messages.takeover_active(agent))
            }),
        );
        let agents_module: Arc<dyn messages::TerminalOwners> = agents.clone();
        let checkpoints = Arc::new(checkpoints::Checkpoints::open(dir)?.with_marks(agents.clone()));
        let pads = Arc::new(pads::Pads::open(dir, bus.clone())?);
        let mut daemon = Self {
            modules: HashMap::new(),
            todos: Arc::clone(&todos),
            pads: Arc::clone(&pads),
            door_first_steers: Default::default(),
            child_effects: Default::default(),
            held_actions: resize::HeldActions::open(dir)?,
            touches,
            bus: bus.clone(),
            project: std::path::absolute(project).unwrap_or_else(|_| project.to_path_buf()),
            attachments: attachment::Attachments::open(dir),
            digests: Arc::new(messages::digest::Seen::open(&dir.join("project.db"))?),
            sender: Arc::clone(&messages),
            agents: Arc::clone(&agents),
            terminals: Arc::clone(&terminals),
            notes: std::sync::Mutex::new(Some(bus.subscribe())),
            wake_events: std::sync::Mutex::new(Some(bus.subscribe())),
            wake_gate: Default::default(),
            clears: clear::Clears::new(Arc::clone(&agents)),
            checkpoints: Arc::clone(&checkpoints),
            rewinding: Default::default(),
            pushes: std::sync::Mutex::new(Some(bus.subscribe())),
            resources: Default::default(),
            prompt_effect_gate: std::env::var_os("RUPD_PROMPT_EFFECT_GATE").map(Into::into),
            waits: std::sync::Mutex::new(Some((
                bus.subscribe(),
                Arc::clone(&agents),
                Arc::clone(&todos),
            ))),
        };
        daemon.register(todos);
        daemon.register(pads);
        daemon.register(checkpoints);
        // B2: the one function that types a Message is `Agents::prompt` (H11).
        let typist = Arc::clone(&agents);
        let deliver: messages::Deliver = Arc::new(move |id, text| {
            let typist = Arc::clone(&typist);
            Box::pin(async move {
                typist
                    .prompt(&id, &text)
                    .await
                    .map_err(|refused| match refused {
                        agents::PromptError::Busy { .. } => messages::Refusal::Busy,
                        agents::PromptError::NotAccepted { .. } => messages::Refusal::NotAccepted,
                        agents::PromptError::NotFound { .. } => messages::Refusal::NotFound,
                    })
            })
        });
        let opened = Arc::new(messages::Messages::open_with(
            dir,
            bus,
            agents_module,
            deliver,
            messages_env(Arc::clone(&daemon.touches)),
        )?);
        // B39: the Agent's `UserPromptSubmit` Signal confirms the Messages its prompt carried.
        let confirming = Arc::downgrade(&opened);
        agents.observe_prompts(Arc::new(move |id, prompt| {
            confirming
                .upgrade()
                .is_some_and(|messages| messages.confirm_prompt(id, prompt))
        }));
        // B50: a `note` is written to the Agent's Claude inbox; its transcript confirms it.
        opened.set_inbox(agents.clone());
        let recording = Arc::downgrade(&opened);
        agents.observe_inbox(Arc::new(move |id, ids| {
            if let Some(messages) = recording.upgrade() {
                messages.confirm_inbox(id, ids);
            }
        }));
        let _ = messages.set(Arc::clone(&opened));
        daemon.register(opened);
        let placement = Arc::new(agents.placement_call());
        daemon.register(Arc::new(agents::Asks::open(
            dir,
            daemon.bus.clone(),
            Arc::clone(&agents),
            placement,
        )?));
        daemon.register(agents);
        daemon.register(terminals);
        daemon.start_pushes();
        Ok(daemon)
    }

    /// B21: start sending children's changes to their Doors, at most once.
    fn start_pushes(&self) {
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        let waiter = self
            .waits
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some((events, agents, todos)) = waiter {
            let (touches, bus) = (Arc::clone(&self.touches), self.bus.clone());
            tokio::spawn(waits::keep(events, agents, move || {
                let ctx = Ctx {
                    actor: Actor::daemon(),
                    bus: bus.clone(),
                    touches: Arc::clone(&touches),
                };
                let todos = Arc::clone(&todos);
                async move {
                    let list = todos.call(&ctx, "todo.list", Value::Null).await.ok()?;
                    serde_json::from_value(list).ok()
                }
            }));
        }
        let events = self
            .pushes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        let notes = self
            .notes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let (Some(events), Some(sender)) = (notes, self.sender.get()) {
            tokio::spawn(decision_notes::tell_doors(
                self.reader(),
                Arc::clone(sender),
                events,
            ));
        }
        let wake_events = self
            .wake_events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let (Some(events), Some(waker)) = (wake_events, self.waker()) {
            tokio::spawn(waker.run(events));
        }
        if let (Some(events), Some(sender)) = (events, self.sender.get()) {
            tokio::spawn(digest::push_changes(
                self.reader(),
                Arc::clone(sender),
                Arc::clone(&self.digests),
                events,
            ));
        }
    }

    fn waker(&self) -> Option<wake::Waker> {
        Some(wake::Waker {
            reader: self.reader(),
            messages: Arc::clone(self.sender.get()?),
            digests: Arc::clone(&self.digests),
            bus: self.bus.clone(),
            gate: Arc::clone(&self.wake_gate),
        })
    }

    fn reader(&self) -> digest::Reader {
        digest::Reader::new(
            &self.modules,
            self.bus.clone(),
            Arc::clone(&self.touches),
            std::env::var_os("RUPD_DIGEST_OBSERVER").map(std::path::PathBuf::from),
        )
    }

    fn register(&mut self, module: Arc<dyn Module>) {
        for namespace in module.namespaces() {
            self.modules.insert(namespace, Arc::clone(&module));
        }
    }

    /// Answer one request line. Notifications (no `id`) get no answer.
    async fn dispatch(&self, conn: &mut Conn, line: &str, actor: Option<Actor>) -> Option<Value> {
        let Ok(request) = serde_json::from_str::<Value>(line) else {
            return Some(failure(
                &Value::Null,
                &RpcError::new(code::PARSE_ERROR, "parse error"),
            ));
        };
        let id = request.get("id").cloned();
        let method = request["method"].as_str().unwrap_or_default();
        let params = request.get("params").cloned().unwrap_or(Value::Null);
        if method.starts_with("decision.") {
            // Preserve the wire reader's authenticated identity while earlier actions finish.
            conn.actor = actor.unwrap_or_else(|| self.decision_actor(conn));
        } else if matches!(
            method,
            "rail.rename"
                | "agent.stop"
                | "agent.interrupt"
                | "agent.prompt"
                | "agent.setOrder"
                | "agent.resume"
                | "agent.actionHistory"
                | "workstream.rewind"
                | "workstream.rewindPlan"
        ) {
            conn.actor = actor.unwrap_or_else(|| self.action_actor(conn));
        }
        let outcome = self.route(conn, method, params).await;
        let id = id?;
        Some(match outcome {
            Ok(result) => {
                let mut envelope = serde_json::Map::new();
                envelope.insert("jsonrpc".into(), Value::String("2.0".into()));
                envelope.insert("id".into(), id);
                envelope.insert("result".into(), result);
                Value::Object(envelope)
            }
            Err(err) => failure(&id, &err),
        })
    }

    async fn route(&self, conn: &mut Conn, method: &str, params: Value) -> Result<Value, RpcError> {
        self.start_pushes();
        // A child may not turn its Terminal keystrokes into a user's answer by
        // claiming `user`; H21's process identity applies to this input path too.
        let input_actor = if method == "terminal.write" {
            let actor = self.terminal_writer(conn);
            if actor.kind == ActorKind::Agent {
                conn.actor = actor.clone();
            }
            Some(actor)
        } else {
            None
        };
        let namespace = method.split('.').next().unwrap_or_default();
        if matches!(method, "daemon.identify" | "events.subscribe") {
            return builtin::call(self, conn, namespace, method, &params)
                .expect("connection-state methods are built in");
        }
        if matches!(method, "terminal.output" | "terminal.outputStop") {
            return self.terminal_output(conn, method, params).await;
        }
        if method == "agent.actionHistory" {
            return self.action_history(conn, params).await;
        }
        if method == "terminal.resizeHistory" {
            return self.resize_history(conn, params).await;
        }
        // These Door calls account for their physical effects before returning, including
        // uncertain outcomes. Begin must not grant the child while an effect can still run.
        let children = self
            .door_effect_children(&conn.actor, method, &params)
            .await?;
        if matches!(method, "workstream.rewind" | "workstream.rewindPlan") {
            // The wire reader selects only for an Agent; the Door's authority is decided here for every caller.
            let children = self.rewind_children(&conn.actor, method, &params).await?;
            return self
                .rewind_call(&conn.actor, method, params, children)
                .await;
        }
        let child = children.first();
        let _ordered_effect = if matches!(
            method,
            "terminal.resize"
                | "terminal.close"
                | "terminal.kill"
                | "terminal.write"
                | "rail.rename"
                | "agent.stop"
        ) {
            if let Some(child) = child {
                let guard = self.child_effects(child).write_owned().await;
                if method == "terminal.resize" {
                    if let Some(receipt) = self.hold_resize(conn, child, params.clone()).await? {
                        return Ok(receipt);
                    }
                } else if let Some(receipt) = self
                    .hold_child_action(conn, child, method, params.clone())
                    .await?
                {
                    return Ok(receipt);
                }
                Some(guard)
            } else {
                None
            }
        } else {
            None
        };
        let mut door_effects = Vec::new();
        if !matches!(
            method,
            "terminal.resize"
                | "terminal.close"
                | "terminal.kill"
                | "terminal.write"
                | "rail.rename"
                | "agent.stop"
        ) {
            // Multi-child reads acquire in one stable order, so two reads cannot deadlock.
            for child in &children {
                let guard = loop {
                    // A Door's read of a taken-over child waits for the Takeover's end,
                    // behind the actions released then.
                    if is_child_read(method) {
                        self.wait_takeover_end(child).await;
                    }
                    let guard = if !self.sender.get().is_some_and(|m| m.takeover_active(child))
                        && self.held_actions.waiting(child)?
                    {
                        let guard = self.child_effects(child).write_owned().await;
                        // A newer begin may have won while this call waited for exclusion.
                        if !self.sender.get().is_some_and(|m| m.takeover_active(child)) {
                            self.release_actions(child).await?;
                        }
                        guard.downgrade()
                    } else {
                        // With no older queue, Interrupt must still reach an in-flight Steer.
                        self.child_effects(child).read_owned().await
                    };
                    // Begin needs the write lock, so a begin since the wait shows here.
                    if is_child_read(method)
                        && self.sender.get().is_some_and(|m| m.takeover_active(child))
                    {
                        continue;
                    }
                    break guard;
                };
                // Begin needs the write lock, so this check under the read guard is final.
                let guard = if matches!(
                    method,
                    "agent.interrupt"
                        | "agent.prompt"
                        | "agent.setOrder"
                        | "agent.resume"
                        | "decision.answer"
                ) && self.sender.get().is_some_and(|m| m.takeover_active(child))
                {
                    drop(guard);
                    let write = self.child_effects(child).write_owned().await;
                    if let Some(receipt) = self
                        .hold_child_action(conn, child, method, params.clone())
                        .await?
                    {
                        return Ok(receipt);
                    }
                    write.downgrade()
                } else {
                    guard
                };
                door_effects.push(guard);
            }
        }
        let _takeover = if method == "takeover.begin" && conn.actor.kind == ActorKind::User {
            let contracts::message::TakeoverParams { agent } = rpc::params(params.clone())?;
            Some(self.child_effects(&agent).write_owned().await)
        } else {
            None
        };
        // A Door's spawn publishes the child before its detached first Steer completes.
        // Hold registration through that publication so begin cannot miss the effect.
        let mut first_steers = if method == "agent.spawn" && conn.actor.kind == ActorKind::Agent {
            Some(self.door_first_steers.lock().await)
        } else {
            None
        };
        if method == "takeover.begin" && conn.actor.kind == ActorKind::User {
            let contracts::message::TakeoverParams { agent } = rpc::params(params.clone())?;
            let attempt = self.door_first_steers.lock().await.get(&agent).cloned();
            if let Some(attempt) = attempt
                && self
                    .rail()
                    .await?
                    .iter()
                    .any(|node| node.id == agent && node.attempt.as_deref() == Some(&attempt))
            {
                self.agents
                    .first_steer_completion(&agent, &attempt)
                    .await
                    .map_err(|err| {
                        RpcError::conflict(format!(
                            "cannot acquire Takeover of {agent}: first Steer completion is unknown: {err}"
                        ))
                    })?;
                self.door_first_steers.lock().await.remove(&agent);
            }
        }
        if matches!(method, "agent.context" | "agent.brief") {
            return self.awareness(conn, method, params).await;
        }
        if matches!(
            method,
            "decision.list" | "decision.answer" | "decision.capture" | "decision.answers"
        ) {
            return self.decision_call(conn, method, params).await;
        }
        self.before_prompt_effect(&conn.actor, method, &params)
            .await?;
        if method == "attachment.save" {
            return self.attachments.save(params);
        }
        if method == "project.resources" {
            return self.project_resources().await;
        }
        match method {
            "workstream.clear" => return self.clear(&conn.actor, params).await,
            "workstream.handoff" => return self.handoff(&conn.actor, params).await,
            _ => {}
        }
        if method == "agent.skills" {
            return self.skills(params).await;
        }
        if method == "checkpoint.write" {
            self.require_door(&conn.actor).await?;
        }
        if method == "agent.digest" {
            return self.digest(conn, params).await;
        }
        if method == "agent.wake" {
            let waker = self
                .waker()
                .ok_or_else(|| RpcError::internal("messages are not open"))?;
            return rpc::reply(&waker.ask(&conn.actor, params).await?);
        }
        if let Some(outcome) = builtin::call(self, conn, namespace, method, &params) {
            return outcome;
        }
        // X12: a Terminal the user opens is owned by a Workstream, which only the Rail knows.
        let namespace = if method == "terminal.open" {
            "agent"
        } else {
            namespace
        };
        let module = self
            .modules
            .get(namespace)
            .ok_or_else(|| RpcError::method_not_found(method))?;
        if matches!(method, "todo.move" | "pad.move") {
            if method == "pad.move" && conn.actor.kind == ActorKind::Agent {
                return Err(RpcError::forbidden("an Agent may not move a pad"));
            }
            self.require_workstream(&params).await?;
        }
        if matches!(
            method,
            "terminal.snapshot"
                | "terminal.screen"
                | "terminal.read"
                | "terminal.search"
                | "terminal.write"
                | "terminal.resize"
                | "terminal.kill"
                | "terminal.close"
        ) {
            self.guard_terminal_read(&conn.actor, method, &params)
                .await?;
        }
        if method == "terminal.show" {
            self.guard_terminal(&conn.actor, method, &params, true)
                .await?;
        }
        if method == "terminal.open" {
            self.guard_terminal_open(&conn.actor, &params)?;
        }
        if method == "route.set" {
            self.guard_route_set(&conn.actor, &params)?;
        }
        if method == "terminal.write" {
            let write: contracts::terminal::WriteParams = rpc::params(params.clone())?;
            {
                use base64::Engine;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(&write.data)
                    .map_err(|err| {
                        RpcError::new(code::INVALID_PARAMS, format!("data is not base64: {err}"))
                    })?;
                let guarded = write.identity.is_some();
                if conn.actor.kind == ActorKind::User && !guarded {
                    self.note_user_input(&params).await;
                }
                self.agents
                    .write_terminal(
                        &conn.actor,
                        input_actor
                            .as_ref()
                            .expect("Terminal input has an authenticated source"),
                        &write.id,
                        bytes,
                        write.identity,
                    )
                    .await?;
                // A refused guarded submit wrote no bytes and cannot release Messages
                // waiting behind the user's input. Ordinary writes retain their queued cut.
                if conn.actor.kind == ActorKind::User && guarded {
                    self.note_user_input(&params).await;
                }
                return rpc::reply(&());
            }
        }
        if method == "rail.remove" {
            let NodeId { id } = rpc::params(params.clone())?;
            if self
                .rail()
                .await?
                .iter()
                .any(|n| n.id == id && n.kind == contracts::agent::NodeKind::Workstream)
            {
                return self.remove_workstream(&conn.actor, &id).await;
            }
        }
        let outcome = module
            .call(&self.ctx(conn.actor.clone()), method, params.clone())
            .await;
        if method == "takeover.end" && outcome.is_ok() {
            let contracts::message::TakeoverParams { agent } = rpc::params(params.clone())?;
            // Message release can await an already-recorded prompt. A new Takeover must
            // remain independent of that typing, and it wins over this older release.
            let _release = self.child_effects(&agent).write_owned().await;
            if !self
                .sender
                .get()
                .is_some_and(|messages| messages.takeover_active(&agent))
            {
                self.release_actions(&agent).await?;
            }
        }
        if let (Some(first_steers), Ok(node)) = (&mut first_steers, &outcome)
            && let (Some(id), Some(attempt)) = (node["id"].as_str(), node["attempt"].as_str())
        {
            first_steers.insert(id.to_owned(), attempt.to_owned());
            let first_steers = Arc::clone(&self.door_first_steers);
            let agents = Arc::clone(&self.agents);
            let (id, attempt) = (id.to_owned(), attempt.to_owned());
            tokio::spawn(async move {
                if agents.first_steer_completion(&id, &attempt).await.is_ok() {
                    let mut first_steers = first_steers.lock().await;
                    if first_steers.get(&id) == Some(&attempt) {
                        first_steers.remove(&id);
                    }
                }
            });
        }
        if matches!(method, "todo.create" | "pad.create") && conn.actor.kind == ActorKind::Agent {
            return self.home_in_workstream(method, &conn.actor, outcome?).await;
        }
        if method == "rail.remove" && outcome.is_ok() {
            self.reset_orphaned_homes().await?;
            self.reset_orphaned_pad_homes().await?;
            return outcome;
        }
        outcome
    }

    /// A known Agent process cannot gain the user's control or receipt access by identifying
    /// as the user. The wire reader captures this before an earlier action can delay dispatch.
    fn action_actor(&self, conn: &Conn) -> Actor {
        let actor = self.terminal_writer(conn);
        if actor.kind == ActorKind::Agent {
            actor
        } else {
            conn.actor.clone()
        }
    }

    fn terminal_writer(&self, conn: &Conn) -> Actor {
        match conn.peer.and_then(|pid| self.agents.agent_of_peer(pid)) {
            Some(agent) => Actor {
                kind: ActorKind::Agent,
                id: agent,
                parent: None,
            },
            None if conn.actor.kind == ActorKind::Ext => conn.actor.clone(),
            None => Actor::user(),
        }
    }

    /// Return once no Takeover holds `child`: subscribe first, so an end cannot slip past.
    async fn wait_takeover_end(&self, child: &str) {
        let (mut events, _) = self.subscribe();
        while self.sender.get().is_some_and(|m| m.takeover_active(child)) {
            match events.recv().await {
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            }
        }
    }

    /// Aggregate reads must respect each child's prior calls, not only the Door's own id.
    async fn door_effect_children(
        &self,
        actor: &Actor,
        method: &str,
        params: &Value,
    ) -> Result<Vec<String>, RpcError> {
        if actor.kind != ActorKind::Agent {
            return Ok(Vec::new());
        }
        let mut children: Vec<String> = match method {
            "agent.digest" => {
                let NodeId { id } = rpc::params(params.clone())?;
                // The Digest module still owns its permission and node-kind checks.
                if actor.id != id {
                    return Err(RpcError::forbidden(format!(
                        "{} may not read the digest of agent {id}",
                        actor.id
                    )));
                }
                self.rail()
                    .await?
                    .into_iter()
                    .filter(|n| n.parent.as_deref() == Some(&id) && n.status.is_some())
                    .map(|n| n.id)
                    .collect()
            }
            "workstream.rewind" | "workstream.rewindPlan" => {
                self.rewind_children(actor, method, params).await?
            }
            "decision.list" => {
                // Use the Agent's Reach filtering; never wait on an unrelated Workstream.
                let listed = self.modules["decision"]
                    .call(&self.ctx(actor.clone()), method, Value::Null)
                    .await?;
                let decisions: Vec<contracts::decision::Decision> =
                    serde_json::from_value(listed).map_err(RpcError::internal)?;
                decisions.into_iter().map(|d| d.agent).collect()
            }
            _ => self
                .door_effect_child(actor, method, params)
                .await?
                .into_iter()
                .collect(),
        };
        children.sort();
        children.dedup();
        Ok(children)
    }

    /// The wire reader and router must agree which effects share a child's arrival order.
    async fn door_effect_child(
        &self,
        actor: &Actor,
        method: &str,
        params: &Value,
    ) -> Result<Option<String>, RpcError> {
        if actor.kind != ActorKind::Agent {
            return Ok(None);
        }
        match method {
            "terminal.snapshot" | "terminal.screen" | "terminal.read" | "terminal.search"
            | "terminal.write" | "terminal.kill" | "terminal.close" | "terminal.resize" => {
                self.guard_terminal_read(actor, method, params).await
            }
            "decision.answers" => {
                let answers: contracts::decision::AnswersParams = rpc::params(params.clone())?;
                Ok(Some(answers.agent))
            }
            "decision.answer" | "decision.capture" => {
                let id = if method == "decision.answer" {
                    rpc::params::<contracts::decision::AnswerParams>(params.clone())?.id
                } else {
                    rpc::params::<contracts::decision::CaptureParams>(params.clone())?.id
                };
                let listed = self.modules["decision"]
                    .call(&self.ctx(actor.clone()), "decision.list", Value::Null)
                    .await?;
                let decisions: Vec<contracts::decision::Decision> =
                    serde_json::from_value(listed).map_err(RpcError::internal)?;
                Ok(decisions.into_iter().find(|d| d.id == id).map(|d| d.agent))
            }
            "agent.prompt"
            | "agent.setOrder"
            | "agent.interrupt"
            | "agent.resume"
            | "agent.stop"
            | "agent.discard"
            | "agent.land"
            | "agent.worktreeState"
            | "agent.spawnPrompt"
            | "agent.run"
            | "rail.rename"
            | "rail.move"
            | "rail.remove" => Ok(Some(rpc::params::<NodeId>(params.clone())?.id)),
            _ => Ok(None),
        }
    }

    fn child_effects(&self, child: &str) -> Arc<tokio::sync::RwLock<()>> {
        let mut writes = self
            .child_effects
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // No historical child or completed effect needs a live lock. Weak entries keep
        // concurrent callers on one lock without retaining every Agent ever created.
        writes.retain(|_, lock| lock.strong_count() != 0);
        if let Some(lock) = writes.get(child).and_then(std::sync::Weak::upgrade) {
            return lock;
        }
        let lock = Arc::new(tokio::sync::RwLock::new(()));
        writes.insert(child.to_owned(), Arc::downgrade(&lock));
        lock
    }

    async fn before_prompt_effect(
        &self,
        actor: &Actor,
        method: &str,
        params: &Value,
    ) -> Result<(), RpcError> {
        if method == "agent.signal"
            && agents::claude_code::ends_decision(&params["payload"])
            && let Some(gate) = &self.prompt_effect_gate
            && std::fs::read_to_string(gate).ok().as_deref() == Some("effect")
        {
            // The Signal proceeds to its synchronous fold in this same router poll.
            std::fs::write(gate.with_extension("racing"), method).map_err(RpcError::internal)?;
        }
        let observed = match method {
            "agent.stop" | "agent.interrupt" | "agent.prompt" | "rail.rename" | "terminal.kill"
            | "terminal.close" | "terminal.resize" => actor.kind == ActorKind::Agent,
            "decision.answer" => params["identity"].is_object() || actor.kind == ActorKind::Agent,
            "terminal.write" => params["identity"].is_object(),
            _ => false,
        };
        if !observed {
            return Ok(());
        }
        let Some(gate) = &self.prompt_effect_gate else {
            return Ok(());
        };
        if std::fs::read_to_string(gate).ok().as_deref() != Some("before") {
            return Ok(());
        }
        std::fs::write(gate.with_extension("arrived"), method).map_err(RpcError::internal)?;
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while gate.is_file() {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .map_err(|_| RpcError::conflict("original prompt effect observer was not released"))?;
        Ok(())
    }

    /// H21: a Decision caller is the Agent owning its peer process, else the user.
    fn decision_actor(&self, conn: &Conn) -> Actor {
        match conn.peer.and_then(|pid| self.agents.agent_of_peer(pid)) {
            Some(id) => Actor {
                kind: ActorKind::Agent,
                id,
                parent: None,
            },
            None => Actor::user(),
        }
    }

    async fn decision_call(
        &self,
        conn: &Conn,
        method: &str,
        params: Value,
    ) -> Result<Value, RpcError> {
        let module = self
            .modules
            .get("decision")
            .ok_or_else(|| RpcError::method_not_found(method))?;
        // The router holds the child's effect guard; taking it again could deadlock
        // behind a waiting Takeover writer. Identity was fixed before that guard.
        let ctx = self.ctx(conn.actor.clone());
        self.before_prompt_effect(&ctx.actor, method, &params)
            .await?;
        module.call(&ctx, method, params).await
    }

    /// M13: `NOT_FOUND` for an unknown Terminal, then `FORBIDDEN` when an Agent may not read it.
    /// A malformed id is left to the Terminal's own `INVALID_PARAMS`. Returns the authorized
    /// Terminal's owner so its write and the owner's Takeover use the same exclusion lock.
    async fn guard_terminal_read(
        &self,
        actor: &Actor,
        method: &str,
        params: &Value,
    ) -> Result<Option<String>, RpcError> {
        self.guard_terminal(actor, method, params, false).await
    }

    /// `guard_terminal_read`, or with `show` B53's wider rule for a Show request.
    async fn guard_terminal(
        &self,
        actor: &Actor,
        method: &str,
        params: &Value,
        show: bool,
    ) -> Result<Option<String>, RpcError> {
        if actor.kind != ActorKind::Agent {
            return Ok(None);
        }
        let Some(id) = params["id"].as_str() else {
            return Ok(None);
        };
        let terminals = self
            .modules
            .get("terminal")
            .ok_or_else(|| RpcError::internal("terminal module is not registered"))?;
        let listed = terminals
            .call(&self.ctx(Actor::daemon()), "terminal.list", Value::Null)
            .await?;
        let infos: Vec<TerminalInfo> =
            serde_json::from_value(listed).map_err(RpcError::internal)?;
        match infos.iter().find(|info| info.id == id) {
            Some(info) if show => self
                .agents
                .guard_terminal_show(actor, id, info.owner.as_deref())
                .map(|()| info.owner.clone()),
            Some(info) => self
                .agents
                .guard_terminal_read(actor, id, info.owner.as_deref(), is_child_read(method))
                .map(|()| info.owner.clone()),
            None if id.parse::<u64>().is_ok_and(|n| n.to_string() == id) => {
                Err(RpcError::not_found(format!("terminal {id}")))
            }
            None => Ok(None),
        }
    }

    /// X14: select or release one Terminal's output for this connection.
    async fn terminal_output(
        &self,
        conn: &Conn,
        method: &str,
        params: Value,
    ) -> Result<Value, RpcError> {
        let contracts::terminal::TerminalId { id } = rpc::params(params.clone())?;
        let port = conn
            .output
            .as_ref()
            .ok_or_else(|| RpcError::internal("this call cannot receive Terminal output"))?;
        self.guard_terminal_read(&conn.actor, method, &params)
            .await?;
        if method == "terminal.outputStop" {
            port.release(&id);
            return rpc::reply(&());
        }
        let stream = self.terminals.subscribe_output(&id)?;
        let offset = stream.start();
        port.select(&id, stream);
        rpc::reply(&contracts::terminal::OutputStarted { offset })
    }

    /// M14: an Agent opens a Terminal only in its own Workstream, one above it or one in its Reach.
    fn guard_terminal_open(&self, actor: &Actor, params: &Value) -> Result<(), RpcError> {
        match params["workstream"].as_str() {
            Some(workstream) if actor.kind == ActorKind::Agent => {
                self.agents
                    .guard_terminal_read(actor, workstream, Some(workstream), false)
            }
            _ => Ok(()),
        }
    }

    /// M14: an Agent's Route is between Agents that are it or in its Reach, never to itself.
    fn guard_route_set(&self, actor: &Actor, params: &Value) -> Result<(), RpcError> {
        if actor.kind != ActorKind::Agent {
            return Ok(());
        }
        self.agents.guard_route(actor, params)
    }

    /// B32: the user's bytes are counted before they reach the Terminal, so a Message checked
    /// after they arrive finds them. A call that is not well formed is left to the Terminal's own error.
    async fn note_user_input(&self, params: &Value) {
        use base64::Engine as _;
        let (Some(messages), Some(id), Some(data)) = (
            self.sender.get(),
            params["id"].as_str(),
            params["data"].as_str(),
        ) else {
            return;
        };
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(data) {
            messages.user_wrote(id, &bytes).await;
        }
    }

    /// T14, P12: a Todo or Pad an Agent just created gets the nearest Workstream at or above that Agent as its Home.
    async fn home_in_workstream(
        &self,
        method: &str,
        actor: &Actor,
        created: Value,
    ) -> Result<Value, RpcError> {
        let (namespace, move_method, key) = match method {
            "pad.create" => ("pad", "pad.move", "name"),
            _ => ("todo", "todo.move", "id"),
        };
        let nodes = self.rail().await?;
        let mut at = nodes.iter().find(|node| node.id == actor.id);
        while let Some(node) = at {
            if node.kind == contracts::agent::NodeKind::Workstream {
                let module = self.modules.get(namespace).ok_or_else(|| {
                    RpcError::internal(format!("{namespace} module is not registered"))
                })?;
                let ctx = self.ctx(Actor::daemon());
                return module
                    .call(
                        &ctx,
                        move_method,
                        json!({ key: created[key].clone(), "home": node.id }),
                    )
                    .await;
            }
            at = node
                .parent
                .as_ref()
                .and_then(|parent| nodes.iter().find(|n| &n.id == parent));
        }
        Ok(created)
    }

    /// M10: only a Door writes a Checkpoint; a Door is the Agent whose id is a Workstream's.
    async fn require_door(&self, actor: &Actor) -> Result<(), RpcError> {
        let is_door = actor.kind == ActorKind::Agent
            && self.rail().await?.iter().any(|node| {
                node.id == actor.id && node.kind == contracts::agent::NodeKind::Workstream
            });
        if is_door {
            return Ok(());
        }
        Err(RpcError::forbidden(format!(
            "{} is not a Door, so it may not write a Checkpoint",
            actor.id
        )))
    }

    /// T13, P12: a `todo.move` or `pad.move` names a Workstream on the Rail, or the Project root.
    async fn require_workstream(&self, params: &Value) -> Result<(), RpcError> {
        let home: Option<String> = rpc::params::<HomeOnly>(params.clone())?.home;
        let Some(home) = home else { return Ok(()) };
        let nodes = self.rail().await?;
        match nodes.iter().find(|node| node.id == home) {
            None => Err(RpcError::not_found(format!("rail node {home}"))),
            Some(node) if node.kind != contracts::agent::NodeKind::Workstream => {
                Err(RpcError::new(
                    code::INVALID_PARAMS,
                    format!("rail node {home} is not a Workstream"),
                ))
            }
            Some(_) => Ok(()),
        }
    }

    /// D8: the Resource use of every running Agent and the Project's total. The Rail is read as
    /// the Daemon, so the call changes nothing and logs no Touch.
    async fn project_resources(&self) -> Result<Value, RpcError> {
        let worktrees = self
            .rail()
            .await?
            .into_iter()
            .filter_map(|node| Some((node.id, std::path::PathBuf::from(node.worktree?.path))))
            .collect();
        let running = self.agents.running_programs();
        rpc::reply(&self.resources.sample(&running, &worktrees).await)
    }

    /// T13: after `rail.remove`, each Todo whose Home left the Rail goes to the Project root, in id order.
    async fn reset_orphaned_homes(&self) -> Result<(), RpcError> {
        let rail: std::collections::HashSet<String> =
            self.rail().await?.into_iter().map(|node| node.id).collect();
        let todos = self
            .modules
            .get("todo")
            .ok_or_else(|| RpcError::internal("todo module is not registered"))?;
        let ctx = self.ctx(Actor::daemon());
        let listed: Vec<contracts::todo::Todo> =
            serde_json::from_value(todos.call(&ctx, "todo.list", Value::Null).await?)
                .map_err(RpcError::internal)?;
        for todo in listed {
            if todo.home.is_some_and(|home| !rail.contains(&home)) {
                todos
                    .call(&ctx, "todo.move", json!({ "id": todo.id, "home": null }))
                    .await?;
            }
        }
        Ok(())
    }

    /// P12: after `rail.remove`, each Pad whose Home left the Rail goes to the Project root, in name order.
    async fn reset_orphaned_pad_homes(&self) -> Result<(), RpcError> {
        let rail: std::collections::HashSet<String> =
            self.rail().await?.into_iter().map(|node| node.id).collect();
        let pads = self
            .modules
            .get("pad")
            .ok_or_else(|| RpcError::internal("pad module is not registered"))?;
        let ctx = self.ctx(Actor::daemon());
        let listed: Vec<contracts::pad::Pad> =
            serde_json::from_value(pads.call(&ctx, "pad.list", Value::Null).await?)
                .map_err(RpcError::internal)?;
        for pad in listed {
            if pad.home.is_some_and(|home| !rail.contains(&home)) {
                pads.call(&ctx, "pad.move", json!({ "name": pad.name, "home": null }))
                    .await?;
            }
        }
        Ok(())
    }

    async fn rail(&self) -> Result<Vec<contracts::agent::RailNode>, RpcError> {
        let rail = self
            .modules
            .get("rail")
            .ok_or_else(|| RpcError::internal("rail module is not registered"))?;
        let tree = rail
            .call(&self.ctx(Actor::daemon()), "rail.tree", Value::Null)
            .await?;
        serde_json::from_value(tree).map_err(RpcError::internal)
    }

    /// E2, E3: the Context of an Agent, or its Brief. The Rail and the Todos are read as the
    /// Daemon, so the call changes nothing and logs no Touch.
    async fn awareness(&self, conn: &Conn, method: &str, params: Value) -> Result<Value, RpcError> {
        let NodeId { id } = rpc::params(params)?;
        let read = |namespace: &'static str, method: &'static str| {
            let module = self
                .modules
                .get(namespace)
                .ok_or_else(|| RpcError::internal(format!("{namespace} module is not registered")));
            let ctx = self.ctx(Actor::daemon());
            async move { module?.call(&ctx, method, Value::Null).await }
        };
        let nodes: Vec<contracts::agent::RailNode> =
            serde_json::from_value(read("rail", "rail.tree").await?).map_err(RpcError::internal)?;
        let todos: Vec<contracts::todo::Todo> =
            serde_json::from_value(read("todo", "todo.list").await?).map_err(RpcError::internal)?;
        let context = context::compose(&nodes, &todos, &id)?;
        let allowed = match conn.actor.kind {
            ActorKind::User => true,
            ActorKind::Agent => conn.actor.id == id,
            ActorKind::Ext => false,
        };
        if !allowed {
            return Err(RpcError::forbidden(format!(
                "{} may not read the Context of agent {id}",
                conn.actor.id
            )));
        }
        match method {
            "agent.context" => rpc::reply(&context),
            _ => rpc::reply(&context::brief(&nodes, &context)),
        }
    }

    /// A31: the Skills of an Agent, or of a Workstream's Door, read from the vendor's folders at
    /// each call. The Rail is read as the Daemon, so the call changes nothing and logs no Touch.
    async fn skills(&self, params: Value) -> Result<Value, RpcError> {
        let NodeId { id } = rpc::params(params)?;
        let node = self
            .rail()
            .await?
            .into_iter()
            .find(|node| node.id == id)
            .ok_or_else(|| RpcError::not_found(format!("agent {id}")))?;
        let cwd = node
            .worktree
            .map_or_else(|| self.project.clone(), |tree| tree.path.into());
        let home = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_default();
        let skills = agents::claude_code::skills(&home, &cwd, &self.project);
        rpc::reply(&contracts::agent::Skills { skills })
    }

    /// B19, B20: the digest of a Door's children. Everything is read as the user or the
    /// Daemon, so the call changes nothing and logs no Touch.
    async fn digest(&self, conn: &Conn, params: Value) -> Result<Value, RpcError> {
        let NodeId { id } = rpc::params(params)?;
        let world = self.reader().world().await?;
        rpc::reply(&self.digests.ask(&conn.actor, &id, &world.sources())?)
    }

    fn ctx(&self, actor: Actor) -> Ctx {
        Ctx {
            actor,
            bus: self.bus.clone(),
            touches: Arc::clone(&self.touches),
        }
    }

    /// Observe future events directly on this Daemon's global Bus, with its retained-event
    /// capacity. The caller controls reads: falling behind returns Tokio's structured
    /// `RecvError::Lagged`, without pausing producers or other subscribers. No forwarding
    /// queue is created. Dropping the receiver detaches only this observer.
    pub fn subscribe(&self) -> (tokio::sync::broadcast::Receiver<contracts::Event>, usize) {
        (self.bus.subscribe(), self.bus.capacity())
    }

    /// Stop every running Terminal's program, Agents' included, and return once none is running.
    /// A program that ignores SIGHUP is killed by the Terminal module's own escalation. A
    /// Terminal spawned while this runs is caught by the next round, up to [`STOP_ROUNDS`]; past
    /// that the caller gets an error rather than a loop that never ends.
    pub async fn stop_terminals(&self) -> Result<(), RpcError> {
        for _ in 0..STOP_ROUNDS {
            if self.stop_running_terminals().await? == 0 {
                return Ok(());
            }
        }
        Err(RpcError::internal(format!(
            "terminals were still being spawned after {STOP_ROUNDS} rounds of stopping"
        )))
    }

    /// One round: kill every Terminal listed as running, concurrently. Returns how many kills stopped a program; one that had already ended does not count.
    async fn stop_running_terminals(&self) -> Result<usize, RpcError> {
        let terminals = self
            .modules
            .get("terminal")
            .ok_or_else(|| RpcError::internal("terminal module is not registered"))?;
        let listed = terminals
            .call(&self.ctx(Actor::daemon()), "terminal.list", Value::Null)
            .await?;
        let infos: Vec<TerminalInfo> =
            serde_json::from_value(listed).map_err(RpcError::internal)?;
        let mut set = tokio::task::JoinSet::new();
        for info in infos.into_iter().filter(|info| info.running) {
            let terminals = Arc::clone(terminals);
            let ctx = self.ctx(Actor::daemon());
            set.spawn(async move {
                terminals
                    .call(&ctx, "terminal.kill", json!({ "id": info.id }))
                    .await
            });
        }
        let mut stopped = 0;
        while let Some(joined) = set.join_next().await {
            // A program that ended on its own between the list and the kill is already stopped.
            match joined.map_err(RpcError::internal)? {
                Ok(_) => stopped += 1,
                Err(err) if err.code != code::NOT_FOUND => return Err(err),
                Err(_) => {}
            }
        }
        Ok(stopped)
    }
}

fn failure(id: &Value, err: &RpcError) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": err })
}

/// Accept connections until the listener fails.
pub async fn serve(listener: UnixListener, daemon: Arc<Daemon>) -> io::Result<()> {
    let mut exits = daemon.bus.subscribe();
    daemon.reconcile_actions().await.map_err(io::Error::other)?;
    let weak = Arc::downgrade(&daemon);
    tokio::spawn(async move {
        loop {
            match exits.recv().await {
                Ok(event) => {
                    // User release can still be typing a recorded Message; its router call
                    // owns that later cut. Only the Daemon's exit release runs here.
                    if let contracts::EventData::TakeoverChanged(change) = &event.data
                        && event.actor == Actor::daemon()
                        && !change.on
                    {
                        let Some(daemon) = weak.upgrade() else { return };
                        let _child = daemon.child_effects(&change.agent).write_owned().await;
                        if !daemon
                            .sender
                            .get()
                            .is_some_and(|m| m.takeover_active(&change.agent))
                            && let Err(e) = daemon.release_actions(&change.agent).await
                        {
                            eprintln!("rupd: held action release failed: {}", e.message);
                            return;
                        }
                    }
                    if let contracts::EventData::TerminalExited(exit) = event.data {
                        let Some(daemon) = weak.upgrade() else { return };
                        if let Err(e) = daemon.resize_exit(&exit.id).await {
                            eprintln!("rupd: resize exit accounting failed: {}", e.message);
                            return;
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    // A missed exit must not silently strand an accepted resize.
                    let Some(daemon) = weak.upgrade() else { return };
                    if let Err(e) = daemon.reconcile_actions().await {
                        eprintln!("rupd: resize exit reconciliation failed: {}", e.message);
                        return;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            }
        }
    });
    loop {
        let (stream, _) = listener.accept().await?;
        let daemon = Arc::clone(&daemon);
        tokio::spawn(async move {
            if let Err(err) = handle(stream, daemon).await {
                eprintln!("rupd: connection failed: {err}");
            }
        });
    }
}

/// The methods whose call is dropped when the caller hangs up: they wait on a process of the
/// caller's, not on the Daemon.
const WAITS_ON_ITS_CALLER: [&str; 2] = ["agent.permission", "agent.ask"];

async fn handle(stream: UnixStream, daemon: Arc<Daemon>) -> io::Result<()> {
    daemon.clears.me.get_or_init(|| Arc::downgrade(&daemon));
    let stream_pid = stream
        .peer_cred()
        .ok()
        .and_then(|cred| cred.pid())
        .and_then(|pid| u32::try_from(pid).ok());
    let mut stream = BufReader::new(stream);
    let first = tokio::time::timeout(std::time::Duration::from_secs(10), stream.fill_buf())
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "connection sent no request"))??;
    if first.first() == Some(&b'P') {
        return terminal_question::serve(stream, daemon, stream_pid).await;
    }
    let (read, mut write) = tokio::io::split(stream);
    let (frames, mut outbox) = mpsc::unbounded_channel::<Value>();
    let (port, mut selected_output) = output::Port::new();
    let (finished, mut finish) = tokio::sync::oneshot::channel();
    let writer = tokio::spawn(async move {
        loop {
            let frame = tokio::select! {
                biased;
                _ = &mut finish, if !outbox.is_closed() => {
                    outbox.close();
                    continue;
                }
                frame = outbox.recv() => match frame {
                    Some(frame) => frame,
                    None => break,
                },
                // Behind replies, so a flood of selected output cannot starve them.
                Some(frame) = selected_output.recv() => frame,
            };
            let mut bytes = serde_json::to_vec(&frame).map_err(io::Error::other)?;
            bytes.push(b'\n');
            write.write_all(&bytes).await?;
        }
        io::Result::Ok(())
    });
    let mut conn = Conn {
        actor: Actor::user(),
        peer: stream_pid,
        events: frames.clone(),
        output: Some(port),
    };
    let mut lines = BufReader::new(read).lines();
    let mut ordinary = tokio::task::JoinSet::new();
    let mut caller_waits = tokio::task::JoinSet::new();
    let mut preceding_wait = None;
    let read_result = loop {
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => break Ok(()),
            Err(err) => break Err(err),
        };
        let request = serde_json::from_str::<Value>(&line).ok();
        let method = request
            .as_ref()
            .and_then(|request| request["method"].as_str().map(str::to_owned));
        // Actor and subscription changes must commit before the next wire request is read.
        if matches!(
            method.as_deref(),
            Some(
                "daemon.identify" | "events.subscribe" | "terminal.output" | "terminal.outputStop"
            )
        ) {
            if let Some(reply) = daemon.dispatch(&mut conn, &line, None).await {
                let _ = frames.send(reply);
            }
            continue;
        }
        let actor = if method.as_deref() == Some("terminal.write") {
            let writer = daemon.terminal_writer(&conn);
            if writer.kind == ActorKind::Agent {
                writer
            } else {
                conn.actor.clone()
            }
        } else if method
            .as_deref()
            .is_some_and(|m| m.starts_with("decision."))
        {
            daemon.decision_actor(&conn)
        } else if matches!(
            method.as_deref(),
            Some(
                "rail.rename"
                    | "agent.stop"
                    | "agent.interrupt"
                    | "agent.prompt"
                    | "agent.setOrder"
                    | "agent.resume"
                    | "agent.actionHistory"
                    | "workstream.rewind"
                    | "workstream.rewindPlan"
            )
        ) {
            daemon.action_actor(&conn)
        } else {
            conn.actor.clone()
        };
        let params = request
            .as_ref()
            .and_then(|r| r.get("params"))
            .unwrap_or(&Value::Null);
        // Register before spawning so scheduler order cannot overtake wire arrival.
        let child_arrival = match daemon
            .door_effect_children(&actor, method.as_deref().unwrap_or_default(), params)
            .await
        {
            Ok(children) => children
                .into_iter()
                .map(|child| {
                    daemon.held_actions.arrive(
                        child,
                        matches!(
                            method.as_deref(),
                            Some(
                                "terminal.resize"
                                    | "terminal.close"
                                    | "terminal.kill"
                                    | "terminal.write"
                                    | "rail.rename"
                                    | "agent.stop"
                                    | "agent.interrupt"
                                    | "agent.prompt"
                                    | "workstream.rewind"
                            )
                        ),
                    )
                })
                .collect::<Vec<_>>(),
            Err(error) => {
                // Reject the original target; a later target must not revive an invalid call.
                if let Some(id) = request.as_ref().and_then(|r| r.get("id")) {
                    let _ = frames.send(failure(id, &error));
                }
                continue;
            }
        };
        let mut request_conn = conn.clone();
        let daemon = Arc::clone(&daemon);
        if method
            .as_deref()
            .is_some_and(|method| WAITS_ON_ITS_CALLER.contains(&method))
        {
            // Chain receivers are allocated in wire order, rather than task scheduling order.
            let before = preceding_wait.take();
            let (done, next) = tokio::sync::oneshot::channel::<()>();
            preceding_wait = Some(next);
            caller_waits.spawn(async move {
                if let Some(before) = before {
                    let _ = before.await;
                }
                if let Some(reply) = daemon.dispatch(&mut request_conn, &line, Some(actor)).await {
                    let _ = request_conn.events.send(reply);
                }
                let _ = done.send(());
            });
        } else {
            ordinary.spawn(async move {
                let mut completed = Vec::new();
                for (before, done) in child_arrival {
                    if let Some(mut before) = before {
                        let _ = before.wait_for(|done| *done).await;
                    }
                    if let Some(done) = done {
                        completed.push(done);
                    }
                }
                if let Some(reply) = daemon.dispatch(&mut request_conn, &line, Some(actor)).await {
                    let _ = request_conn.events.send(reply);
                }
                for done in completed {
                    daemon.held_actions.finish_arrival(done);
                }
            });
        }
        while let Some(joined) = ordinary.try_join_next() {
            joined.map_err(io::Error::other)?;
        }
        while let Some(joined) = caller_waits.try_join_next() {
            joined.map_err(io::Error::other)?;
        }
    };
    // EOF cancels both the entered caller wait and every successor before one can enter.
    caller_waits.abort_all();
    while caller_waits.join_next().await.is_some() {}
    // Accepted ordinary calls survive a write-side half-close and can still send their replies.
    while let Some(joined) = ordinary.join_next().await {
        joined.map_err(io::Error::other)?;
    }
    drop(conn);
    drop(frames);
    let _ = finished.send(());
    writer.await.map_err(io::Error::other)??;
    read_result
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn ask(daemon: &Daemon, line: &str) -> Value {
        let (events, _) = mpsc::unbounded_channel();
        let mut conn = Conn {
            actor: Actor::user(),
            peer: None,
            events,
            output: None,
        };
        daemon.dispatch(&mut conn, line, None).await.unwrap()
    }

    fn daemon() -> (tempfile::TempDir, Daemon) {
        let dir = tempfile::tempdir().unwrap();
        let daemon = Daemon::open(dir.path()).unwrap();
        (dir, daemon)
    }

    struct RailReply(Value);

    #[async_trait::async_trait]
    impl Module for RailReply {
        fn namespaces(&self) -> &'static [&'static str] {
            &["rail"]
        }

        async fn call(&self, _ctx: &Ctx, _method: &str, _params: Value) -> Result<Value, RpcError> {
            Ok(self.0.clone())
        }
    }

    #[tokio::test]
    async fn r16_rail_reply_preserves_every_field_and_row_order_on_the_socket() {
        let (_dir, mut daemon) = daemon();
        // This fixture is the module's already-presented result, not a replacement for
        // Agents' lifecycle tests: the Daemon must transport every field unchanged.
        let nodes = json!([
            {"id":"door", "kind":"workstream", "name":"日本語\n\"Door\" 🦀",
             "parent":null, "order":0, "status":null, "attempt":null,
             "status_revision":null, "terminal_id":null, "worktree":null,
             "can_resume":false, "channel":null, "work":null, "stray":[],
             "model":null, "effort":null, "iteration":2},
            {"id":"child", "kind":"agent", "name":"Child", "parent":"door",
             "order":0, "status":{"kind":"needs-you", "label":"Waiting for approval", "since":123456789},
             "attempt":"9223372036854775807", "status_revision":"3",
             "terminal_id":"terminal-1", "worktree":{"path":"/tmp/p", "branch":"b", "base":"main"},
             "can_resume":false, "channel":"missing", "work":{"kind":"work", "ask":"Continue", "limits":["Preserve files"]},
             "stray":[{"pid":123,"command":"claude \t--resume"}], "model":"model", "effort":"high"},
            {"id":"sibling", "kind":"agent", "name":"Exited", "parent":"door",
             "order":1, "status":null, "attempt":"2", "status_revision":null,
             "terminal_id":null, "worktree":null, "can_resume":true, "channel":null,
             "work":null, "stray":[], "model":null, "effort":null}
        ]);
        serde_json::from_value::<Vec<contracts::agent::RailNode>>(nodes.clone()).unwrap();
        daemon.register(Arc::new(RailReply(nodes.clone())));
        let (client, server) = UnixStream::pair().unwrap();
        let handler = tokio::spawn(handle(server, Arc::new(daemon)));
        let (read, mut write) = client.into_split();
        let mut lines = BufReader::new(read).lines();
        write
            .write_all(b"{\"id\":\"request\\n1\",\"method\":\"rail.tree\"}\n")
            .await
            .unwrap();
        let frame: Value =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(
            frame,
            json!({"jsonrpc":"2.0", "id":"request\n1", "result":nodes})
        );
        assert_eq!(
            frame["result"][0]
                .as_object()
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            nodes[0].as_object().unwrap().keys().collect::<Vec<_>>()
        );
        drop(write);
        drop(lines);
        handler.await.unwrap().unwrap();
    }

    async fn frames(daemon: Daemon, requests: &str, count: usize) -> Vec<Value> {
        let (client, server) = UnixStream::pair().unwrap();
        let handler = tokio::spawn(handle(server, Arc::new(daemon)));
        let (read, mut write) = client.into_split();
        let mut lines = BufReader::new(read).lines();
        write.write_all(requests.as_bytes()).await.unwrap();
        let mut frames = Vec::new();
        for _ in 0..count {
            frames.push(serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap());
        }
        drop(write);
        drop(lines);
        handler.await.unwrap().unwrap();
        frames
    }

    #[tokio::test]
    async fn r16_frames_preserve_complete_error_envelopes() {
        for (request, expected) in [
            (
                "invalid\n",
                json!({"jsonrpc":"2.0", "id":null,
                    "error":{"code":-32700, "message":"parse error"}}),
            ),
            (
                "{\"id\":null,\"method\":\"missing.method\"}\n",
                json!({"jsonrpc":"2.0", "id":null,
                    "error":{"code":-32601, "message":"method not found: missing.method"}}),
            ),
        ] {
            let (_dir, daemon) = daemon();
            assert_eq!(frames(daemon, request, 1).await, vec![expected]);
        }
    }

    #[tokio::test]
    async fn r16_notifications_do_not_add_frames_between_pipelined_replies() {
        let (_dir, daemon) = daemon();
        let mut frames = frames(
            daemon,
            concat!(
                "{\"id\":7,\"method\":\"rail.tree\"}\n",
                "{\"method\":\"daemon.ping\"}\n",
                "{\"id\":8,\"method\":\"rail.tree\"}\n"
            ),
            2,
        )
        .await;
        frames.sort_by_key(|frame| frame["id"].as_u64());
        assert_eq!(
            frames,
            vec![
                json!({"jsonrpc":"2.0", "id":7, "result":[]}),
                json!({"jsonrpc":"2.0", "id":8, "result":[]}),
            ]
        );
    }

    #[tokio::test]
    async fn ping_answers_pong() {
        let (_dir, daemon) = daemon();
        let reply = ask(
            &daemon,
            r#"{"jsonrpc":"2.0","id":7,"method":"daemon.ping"}"#,
        )
        .await;
        assert_eq!(reply["id"], 7);
        assert_eq!(reply["result"]["pong"], true);
    }

    #[tokio::test]
    async fn a38_ping_reports_the_generated_contract() {
        let (_dir, daemon) = daemon();
        let reply = ask(
            &daemon,
            r#"{"jsonrpc":"2.0","id":7,"method":"daemon.ping"}"#,
        )
        .await;
        let generated = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../contracts/generated/contract.ts"
        ))
        .unwrap();
        let contract = &reply["result"]["contract"];
        let listed: Vec<(&str, &str)> = generated
            .lines()
            .filter_map(|line| line.trim().strip_suffix("\","))
            .filter_map(|line| line.strip_prefix('"')?.split_once("\": \""))
            .collect();
        let names: Vec<&str> = contract["methods"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_str().unwrap())
            .collect();
        assert!(!names.is_empty());
        assert_eq!(
            listed.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            names
        );
        assert_eq!(
            contract["fingerprint"].as_str().unwrap(),
            contracts::methods::fingerprint_of_hashes(&listed)
        );
    }

    #[tokio::test]
    async fn unknown_method_is_an_error() {
        let (_dir, daemon) = daemon();
        let reply = ask(
            &daemon,
            r#"{"jsonrpc":"2.0","id":1,"method":"nope.nothing"}"#,
        )
        .await;
        assert_eq!(reply["error"]["code"], code::METHOD_NOT_FOUND);
    }

    struct Echo;

    #[async_trait::async_trait]
    impl Module for Echo {
        fn namespaces(&self) -> &'static [&'static str] {
            &["echo"]
        }

        async fn call(&self, ctx: &Ctx, method: &str, _params: Value) -> Result<Value, RpcError> {
            Ok(json!({ "method": method, "actor": ctx.actor.id }))
        }
    }

    #[tokio::test]
    async fn module_methods_reach_their_module_with_the_callers_actor() {
        let (_dir, mut daemon) = daemon();
        daemon.register(Arc::new(Echo));

        let reply = ask(&daemon, r#"{"jsonrpc":"2.0","id":1,"method":"echo.hello"}"#).await;

        assert_eq!(reply["result"]["method"], "echo.hello");
        assert_eq!(reply["result"]["actor"], "you");
    }

    #[tokio::test]
    async fn subscribers_receive_events_pushed_on_the_same_connection() {
        let dir = tempfile::tempdir().unwrap();
        let daemon = Arc::new(Daemon::open(dir.path()).unwrap());
        let socket = dir.path().join("rupd.sock");
        tokio::spawn(serve(
            UnixListener::bind(&socket).unwrap(),
            Arc::clone(&daemon),
        ));
        let mut client = rpc::Client::connect(&socket).await.unwrap();
        client
            .request("events.subscribe", Value::Null)
            .await
            .unwrap();

        daemon
            .bus
            .emit(Actor::daemon(), contracts::EventData::RailChanged);

        let event = client.next_event().await.unwrap();
        assert_eq!(event.actor, Actor::daemon());
        assert!(matches!(event.data, contracts::EventData::RailChanged));
    }

    /// Lists one running Terminal whose program has already ended, so every kill is `NOT_FOUND`.
    struct SelfEnded;

    #[async_trait::async_trait]
    impl Module for SelfEnded {
        fn namespaces(&self) -> &'static [&'static str] {
            &["terminal"]
        }

        async fn call(&self, _ctx: &Ctx, method: &str, _params: Value) -> Result<Value, RpcError> {
            match method {
                "terminal.list" => Ok(json!([
                    { "id": "1", "cwd": "/", "title": null, "program": "sh", "running": true, "exit_code": null }
                ])),
                _ => Err(RpcError::not_found("running terminal 1")),
            }
        }
    }

    #[tokio::test]
    async fn d1_stopping_programs_that_ended_by_themselves_is_not_a_failure() {
        let (_dir, mut daemon) = daemon();
        daemon.register(Arc::new(SelfEnded));

        daemon.stop_terminals().await.unwrap();
    }

    /// The Daemon wires up every module it owns, including `messages` (`scenarios/messages.md`
    /// B5): a real request through `Daemon::open`, not a fake module, must reach it.
    #[tokio::test]
    async fn the_messages_module_is_reachable_through_the_daemon() {
        let (_dir, daemon) = daemon();
        let reply = ask(&daemon, r#"{"jsonrpc":"2.0","id":1,"method":"route.list"}"#).await;
        assert_eq!(reply["result"], json!([]));
    }

    #[tokio::test]
    async fn garbage_is_a_parse_error() {
        let (_dir, daemon) = daemon();
        let reply = ask(&daemon, "{").await;
        assert_eq!(reply["error"]["code"], code::PARSE_ERROR);
    }
    /// Answers only once released, and says when its call enters and when it is dropped
    /// unanswered.
    struct Hangs {
        says: tokio::sync::mpsc::UnboundedSender<&'static str>,
        release: Arc<tokio::sync::Notify>,
    }

    /// Says `dropped` when its call ends without an answer.
    struct Dropped(Option<tokio::sync::mpsc::UnboundedSender<&'static str>>);

    impl Drop for Dropped {
        fn drop(&mut self) {
            if let Some(says) = self.0.take() {
                let _ = says.send("dropped");
            }
        }
    }

    #[async_trait::async_trait]
    impl Module for Hangs {
        fn namespaces(&self) -> &'static [&'static str] {
            &["agent", "decision"]
        }

        async fn call(&self, _ctx: &Ctx, _method: &str, _params: Value) -> Result<Value, RpcError> {
            let _ = self.says.send("entered");
            let mut dropped = Dropped(Some(self.says.clone()));
            self.release.notified().await;
            dropped.0 = None;
            Ok(json!("answered"))
        }
    }

    async fn serving_a_module_that_hangs() -> (
        tempfile::TempDir,
        tokio::sync::mpsc::UnboundedReceiver<&'static str>,
        Arc<tokio::sync::Notify>,
        std::path::PathBuf,
    ) {
        let (dir, mut daemon) = daemon();
        let (says, said) = mpsc::unbounded_channel();
        let release = Arc::new(tokio::sync::Notify::new());
        daemon.register(Arc::new(Hangs {
            says,
            release: Arc::clone(&release),
        }));
        let socket = dir.path().join("rupd.sock");
        tokio::spawn(serve(
            UnixListener::bind(&socket).unwrap(),
            Arc::new(daemon),
        ));
        (dir, said, release, socket)
    }

    /// What `Hangs` says first, once its call reaches it.
    async fn first_said(
        said: &mut tokio::sync::mpsc::UnboundedReceiver<&'static str>,
    ) -> &'static str {
        tokio::time::timeout(std::time::Duration::from_secs(5), said.recv())
            .await
            .expect("the call reached the module")
            .expect("the module still reports its calls")
    }

    /// H7(b): the hook's process dying closes its connection, which drops the waiting call.
    #[tokio::test]
    async fn h7_a_hung_up_permission_call_is_dropped() {
        let (_dir, mut said, _release, socket) = serving_a_module_that_hangs().await;
        let mut hook = UnixStream::connect(&socket).await.unwrap();
        hook.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"agent.permission\"}\n")
            .await
            .unwrap();
        assert_eq!(first_said(&mut said).await, "entered");
        assert!(
            said.try_recv().is_err(),
            "the call waits while the hook is connected"
        );

        drop(hook);

        let seen = tokio::time::timeout(std::time::Duration::from_secs(5), said.recv()).await;
        assert_eq!(seen.unwrap(), Some("dropped"));
    }

    /// Any other call is not dropped by a half-close: a client may write its request, shut down
    /// its write side and wait for the reply.
    #[tokio::test]
    async fn a_half_closed_call_that_is_not_a_permission_call_still_runs() {
        let (_dir, mut said, release, socket) = serving_a_module_that_hangs().await;
        let mut client = UnixStream::connect(&socket).await.unwrap();
        client
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"agent.stop\",\"params\":{\"id\":\"fixture\"}}\n")
            .await
            .unwrap();
        client.shutdown().await.unwrap();
        assert_eq!(first_said(&mut said).await, "entered");

        release.notify_one();
        let mut reply = String::new();
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            BufReader::new(&mut client).read_line(&mut reply),
        )
        .await
        .expect("the half-closed client reads the reply")
        .unwrap();
        let reply: Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(reply["id"], 1);
        assert_eq!(reply["result"], "answered");

        assert!(said.try_recv().is_err());
    }
}

#[cfg(test)]
mod workstream_door;

/// `RUPD_MESSAGES_FAULT=<operation>` makes the first such Messages operation fail with an error
/// (B28's observer through a real Daemon).
fn messages_env(touches: Arc<provenance::Touches>) -> messages::Env {
    let mut env = messages::Env::system(touches);
    // B21/B22's real-Daemon observer supplies time and records send attempts without a new RPC.
    if let Some(path) = std::env::var_os("RUPD_MESSAGES_CLOCK") {
        env.clock = Arc::new(move || {
            std::fs::read_to_string(&path)
                .expect("read Messages fake clock")
                .trim()
                .parse()
                .expect("Messages fake clock milliseconds")
        });
    }
    let op = std::env::var("RUPD_MESSAGES_FAULT").ok();
    let observer = std::env::var_os("RUPD_MESSAGES_OBSERVER");
    let fault_file = std::env::var_os("RUPD_MESSAGES_FAULT_FILE");
    if op.is_some() || observer.is_some() || fault_file.is_some() {
        let armed = std::sync::atomic::AtomicBool::new(true);
        env.fault = Arc::new(move |name| {
            if let Some(path) = &observer {
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .expect("open Messages observer");
                writeln!(file, "{name}").expect("write Messages operation");
            }
            let selected = fault_file
                .as_ref()
                .and_then(|path| std::fs::read_to_string(path).ok());
            if selected.as_deref() == Some(name)
                || (op.as_deref() == Some(name)
                    && armed.swap(false, std::sync::atomic::Ordering::SeqCst))
            {
                Err(format!("injected failure in {name}"))
            } else {
                Ok(())
            }
        });
    }
    env
}

/// The `home` of a `todo.move` or `pad.move`.
#[derive(serde::Deserialize)]
struct HomeOnly {
    home: Option<String>,
}

#[cfg(test)]
mod concurrent_tests;

/// B6's physical-effect observer. The two phases distinguish invalidation before the
/// check from a user action arriving while the checked effect holds its locks.
fn prompt_effect_observer()
-> impl Fn(&contracts::decision::PromptIdentity) -> Result<(), RpcError> + Send + Sync {
    let gate = std::env::var_os("RUPD_PROMPT_EFFECT_GATE").map(std::path::PathBuf::from);
    move |identity| {
        let Some(gate) = &gate else {
            return Ok(());
        };
        if std::fs::read_to_string(gate).ok().as_deref() != Some("effect") {
            return Ok(());
        }
        std::fs::write(gate.with_extension("arrived"), &identity.decision)
            .map_err(RpcError::internal)?;
        let started = std::time::Instant::now();
        while gate.is_file() {
            if started.elapsed() >= std::time::Duration::from_secs(10) {
                return Err(RpcError::conflict(
                    "original prompt effect observer was not released",
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        Ok(())
    }
}

/// B6's real-Daemon observer pauses on the Terminal input thread before any Steer bytes.
fn steer_effect_observer() -> impl Fn(&str) -> Result<(), RpcError> + Send + Sync {
    let gate = std::env::var_os("RUPD_ORDER_EFFECT_GATE").map(std::path::PathBuf::from);
    move |agent| {
        let Some(gate) = &gate else {
            return Ok(());
        };
        if !gate.is_file() {
            return Ok(());
        }
        std::fs::write(gate.with_extension("arrived"), agent).map_err(RpcError::internal)?;
        let started = std::time::Instant::now();
        while gate.is_file() {
            if std::fs::read_to_string(gate).ok().as_deref() == Some("refuse") {
                return Err(RpcError::conflict("Steer observer refused before bytes"));
            }
            if started.elapsed() >= std::time::Duration::from_secs(10) {
                return Err(RpcError::conflict("Steer observer was not released"));
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        Ok(())
    }
}

/// B6's isolated observer controls Terminal recording and cleanup without a desktop.
fn resume_effect_observer() -> impl Fn(&str, &str) -> Result<(), RpcError> + Send + Sync {
    let gate = std::env::var_os("RUPD_RESUME_EFFECT_GATE").map(std::path::PathBuf::from);
    move |phase, terminal| {
        let Some(gate) = &gate else { return Ok(()) };
        let gate = gate.with_extension(phase);
        let recording = gate.with_extension(format!("{phase}-recording"));
        std::fs::write(&recording, terminal).map_err(RpcError::internal)?;
        std::fs::rename(recording, gate.with_extension(format!("{phase}-arrived")))
            .map_err(RpcError::internal)?;
        let started = std::time::Instant::now();
        while gate.is_file() {
            if std::fs::read_to_string(&gate).ok().as_deref() == Some("refuse") {
                return Err(RpcError::conflict("Terminal cleanup refused before kill"));
            }
            if started.elapsed() >= std::time::Duration::from_secs(30) {
                return Err(RpcError::unknown_outcome(
                    "Resume observer was not released",
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        Ok(())
    }
}

/// B6's isolated exit observer keeps a Terminal's exit unconfirmed while its gate file exists.
fn terminal_exit_observer() -> impl Fn() + Send + Sync {
    let gate = std::env::var_os("RUPD_TERMINAL_EXIT_GATE").map(std::path::PathBuf::from);
    move || {
        let Some(gate) = &gate else { return };
        let started = std::time::Instant::now();
        while gate.is_file() && started.elapsed() < std::time::Duration::from_secs(30) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

/// B6's isolated physical-IO observer uses the real PTY underneath every controlled write.
fn terminal_input_observer()
-> impl Fn(Box<dyn std::io::Write + Send>) -> Box<dyn std::io::Write + Send> {
    let gate = std::env::var_os("RUPD_TERMINAL_INPUT_GATE").map(std::path::PathBuf::from);
    move |writer| match &gate {
        Some(gate) => Box::new(ObservedInput {
            writer,
            gate: gate.clone(),
            partial: false,
        }),
        None => writer,
    }
}

struct ObservedInput {
    writer: Box<dyn std::io::Write + Send>,
    gate: std::path::PathBuf,
    partial: bool,
}

impl std::io::Write for ObservedInput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        match std::fs::read_to_string(&self.gate).ok().as_deref() {
            Some("pause") => {
                std::fs::write(self.gate.with_extension("arrived"), "input")?;
                let started = std::time::Instant::now();
                while self.gate.is_file() {
                    if started.elapsed() >= std::time::Duration::from_secs(10) {
                        return Err(std::io::Error::other("input observer was not released"));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
            Some("partial") if !self.partial => {
                self.partial = true;
                return self.writer.write(&bytes[..bytes.len().min(1)]);
            }
            Some("partial") => return Err(std::io::Error::other("input observer partial write")),
            Some("end") => panic!("input observer ended without a result"),
            _ => {}
        }
        self.writer.write(bytes)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if std::fs::read_to_string(&self.gate).ok().as_deref() == Some("flush") {
            return Err(std::io::Error::other("input observer flush failure"));
        }
        self.writer.flush()
    }
}

// N7: keep in-process test launches at the audited composition root. Callers own
// the temporary data directory and must never fall back to the user's data.
#[cfg(test)]
fn test_daemon(dir: &Path) -> Result<Daemon, OpenError> {
    assert!(dir.is_absolute(), "test data directory must be absolute");
    Daemon::open(dir)
}

#[cfg(test)]
fn test_daemon_with_agents(
    dir: &Path,
    configure: impl FnOnce(agents::Agents) -> agents::Agents,
) -> Result<Daemon, OpenError> {
    assert!(dir.is_absolute(), "test data directory must be absolute");
    Daemon::open_stores(dir, dir, None, configure)
}

/// A Door's read of a child's Terminal, which waits out the child's Takeover.
fn is_child_read(method: &str) -> bool {
    matches!(
        method,
        "terminal.snapshot" | "terminal.screen" | "terminal.read" | "terminal.search"
    )
}
