//! The Daemon: every other part of roundup is a client of it.
//! It owns the connection loop, the module router, the event bus and the Provenance log.

mod builtin;
mod context;
mod handshake;

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
    touches: Arc<Touches>,
}

/// Per-connection state.
pub struct Conn {
    actor: Actor,
    events: mpsc::UnboundedSender<Value>,
}

impl Daemon {
    /// `dir` is the Project's `.roundup/` directory; it is created if missing.
    pub fn open(dir: &Path) -> Result<Self, OpenError> {
        Self::open_with_proof(dir, None)
    }

    /// Like `open`, with the `proof` an attached App handed over (H4, H18); without one every
    /// `decision.answer` is `FORBIDDEN`.
    pub fn open_with_proof(dir: &Path, proof: Option<String>) -> Result<Self, OpenError> {
        std::fs::create_dir_all(dir)?;
        let bus = Bus::new();
        let terminals = Arc::new(terminal::Terminals::open_with(
            dir,
            bus.clone(),
            &agents::claude_code::MARKERS,
        )?);
        let agents = Arc::new(
            agents::Agents::open(dir, bus.clone(), Arc::clone(&terminals))?.with_proof(proof),
        );
        let agents_module: Arc<dyn Module> = agents.clone();
        let mut daemon = Self {
            modules: HashMap::new(),
            touches: Arc::new(Touches::open(&dir.join("provenance.db"))?),
            bus: bus.clone(),
        };
        daemon.register(Arc::new(todos::Todos::open(dir, bus.clone())?));
        daemon.register(Arc::new(pads::Pads::open(dir, bus.clone())?));
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
        daemon.register(Arc::new(messages::Messages::open_with(
            dir,
            bus,
            agents_module,
            deliver,
            messages::Env::system(Arc::clone(&daemon.touches)),
        )?));
        daemon.register(agents);
        daemon.register(terminals);
        Ok(daemon)
    }

    fn register(&mut self, module: Arc<dyn Module>) {
        for namespace in module.namespaces() {
            self.modules.insert(namespace, Arc::clone(&module));
        }
    }

    /// Answer one request line. Notifications (no `id`) get no answer.
    async fn dispatch(&self, conn: &mut Conn, line: &str) -> Option<Value> {
        let Ok(request) = serde_json::from_str::<Value>(line) else {
            return Some(failure(
                &Value::Null,
                &RpcError::new(code::PARSE_ERROR, "parse error"),
            ));
        };
        let id = request.get("id").cloned();
        let method = request["method"].as_str().unwrap_or_default();
        let params = request.get("params").cloned().unwrap_or(Value::Null);
        let outcome = self.route(conn, method, params).await;
        let id = id?;
        Some(match outcome {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(err) => failure(&id, &err),
        })
    }

    async fn route(&self, conn: &mut Conn, method: &str, params: Value) -> Result<Value, RpcError> {
        let namespace = method.split('.').next().unwrap_or_default();
        if matches!(method, "agent.context" | "agent.brief") {
            return self.awareness(conn, method, params).await;
        }
        if let Some(outcome) = builtin::call(self, conn, namespace, method, &params) {
            return outcome;
        }
        let module = self
            .modules
            .get(namespace)
            .ok_or_else(|| RpcError::method_not_found(method))?;
        if method == "todo.move" {
            self.require_workstream(&params).await?;
        }
        let outcome = module
            .call(&self.ctx(conn.actor.clone()), method, params)
            .await;
        if method == "todo.create" && conn.actor.kind == ActorKind::Agent {
            return self.home_in_workstream(&conn.actor, outcome?).await;
        }
        if method == "rail.remove" && outcome.is_ok() {
            self.reset_orphaned_homes().await?;
        }
        outcome
    }

    /// T14: a Todo an Agent just created gets the nearest Workstream at or above that Agent as its Home.
    async fn home_in_workstream(&self, actor: &Actor, created: Value) -> Result<Value, RpcError> {
        let nodes = self.rail().await?;
        let mut at = nodes.iter().find(|node| node.id == actor.id);
        while let Some(node) = at {
            if node.kind == contracts::agent::NodeKind::Workstream {
                let module = self
                    .modules
                    .get("todo")
                    .ok_or_else(|| RpcError::internal("todo module is not registered"))?;
                let id = created["id"].clone();
                let ctx = self.ctx(Actor::daemon());
                return module
                    .call(&ctx, "todo.move", json!({ "id": id, "home": node.id }))
                    .await;
            }
            at = node
                .parent
                .as_ref()
                .and_then(|parent| nodes.iter().find(|n| &n.id == parent));
        }
        Ok(created)
    }

    /// T13: a `todo.move` names a Workstream on the Rail, or the Project root.
    async fn require_workstream(&self, params: &Value) -> Result<(), RpcError> {
        let contracts::todo::MoveParams { home, .. } = rpc::params(params.clone())?;
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
            _ => rpc::reply(&context::brief(&context)),
        }
    }

    fn ctx(&self, actor: Actor) -> Ctx {
        Ctx {
            actor,
            bus: self.bus.clone(),
            touches: Arc::clone(&self.touches),
        }
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

fn waits_on_its_caller(line: &str) -> bool {
    serde_json::from_str::<Value>(line)
        .ok()
        .and_then(|request| request["method"].as_str().map(str::to_owned))
        .is_some_and(|method| WAITS_ON_ITS_CALLER.contains(&method.as_str()))
}

async fn handle(stream: UnixStream, daemon: Arc<Daemon>) -> io::Result<()> {
    let (read, mut write) = stream.into_split();
    let (frames, mut outbox) = mpsc::unbounded_channel::<Value>();
    let writer = tokio::spawn(async move {
        while let Some(frame) = outbox.recv().await {
            write.write_all(format!("{frame}\n").as_bytes()).await?;
        }
        io::Result::Ok(())
    });
    let mut conn = Conn {
        actor: Actor::user(),
        events: frames.clone(),
    };
    let mut lines = BufReader::new(read).lines();
    // A request read while a call was being watched for a hang-up; handled next.
    let mut pipelined = None;
    loop {
        let line = match pipelined.take() {
            Some(line) => line,
            None => match lines.next_line().await? {
                Some(line) => line,
                None => break,
            },
        };
        let reply = if waits_on_its_caller(&line) {
            // H7: the call waits on the hook's process, and the peer closing is what a No or an
            // Esc looks like from here, so the call is dropped the moment the peer hangs up.
            let call = daemon.dispatch(&mut conn, &line);
            tokio::pin!(call);
            tokio::select! {
                reply = &mut call => reply,
                next = lines.next_line() => match next? {
                    Some(next) => {
                        pipelined = Some(next);
                        call.await
                    }
                    None => break,
                },
            }
        } else {
            daemon.dispatch(&mut conn, &line).await
        };
        if let Some(reply) = reply {
            let _ = frames.send(reply);
        }
    }
    drop(conn);
    drop(frames);
    writer.await.map_err(io::Error::other)?
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn ask(daemon: &Daemon, line: &str) -> Value {
        let (events, _) = mpsc::unbounded_channel();
        let mut conn = Conn {
            actor: Actor::user(),
            events,
        };
        daemon.dispatch(&mut conn, line).await.unwrap()
    }

    fn daemon() -> (tempfile::TempDir, Daemon) {
        let dir = tempfile::tempdir().unwrap();
        let daemon = Daemon::open(dir.path()).unwrap();
        (dir, daemon)
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
                    { "id": "1", "cwd": "/", "title": null, "running": true, "exit_code": null }
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
    /// Never answers, and says when its call is dropped.
    struct Hangs(tokio::sync::mpsc::UnboundedSender<&'static str>);

    struct Dropped(tokio::sync::mpsc::UnboundedSender<&'static str>);

    impl Drop for Dropped {
        fn drop(&mut self) {
            let _ = self.0.send("dropped");
        }
    }

    #[async_trait::async_trait]
    impl Module for Hangs {
        fn namespaces(&self) -> &'static [&'static str] {
            &["agent", "decision"]
        }

        async fn call(&self, _ctx: &Ctx, _method: &str, _params: Value) -> Result<Value, RpcError> {
            let _dropped = Dropped(self.0.clone());
            std::future::pending().await
        }
    }

    async fn serving_a_module_that_hangs() -> (
        tempfile::TempDir,
        tokio::sync::mpsc::UnboundedReceiver<&'static str>,
        std::path::PathBuf,
    ) {
        let (dir, mut daemon) = daemon();
        let (dropped, seen) = mpsc::unbounded_channel();
        daemon.register(Arc::new(Hangs(dropped)));
        let socket = dir.path().join("rupd.sock");
        tokio::spawn(serve(
            UnixListener::bind(&socket).unwrap(),
            Arc::new(daemon),
        ));
        (dir, seen, socket)
    }

    /// H7(b): the hook's process dying closes its connection, which drops the waiting call.
    #[tokio::test]
    async fn h7_a_hung_up_permission_call_is_dropped() {
        let (_dir, mut dropped, socket) = serving_a_module_that_hangs().await;
        let mut hook = UnixStream::connect(&socket).await.unwrap();
        hook.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"agent.permission\"}\n")
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(
            dropped.try_recv().is_err(),
            "the call waits while the hook is connected"
        );

        drop(hook);

        let seen = tokio::time::timeout(std::time::Duration::from_secs(5), dropped.recv()).await;
        assert_eq!(seen.unwrap(), Some("dropped"));
    }

    /// Any other call is not dropped by a half-close: a client may write its request, shut down
    /// its write side and wait for the reply.
    #[tokio::test]
    async fn a_half_closed_call_that_is_not_a_permission_call_still_runs() {
        let (_dir, mut dropped, socket) = serving_a_module_that_hangs().await;
        let mut client = UnixStream::connect(&socket).await.unwrap();
        client
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"agent.stop\"}\n")
            .await
            .unwrap();
        client.shutdown().await.unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        assert!(dropped.try_recv().is_err());
    }
}

#[cfg(test)]
mod workstream_door;
