//! The Daemon: every other part of roundup is a client of it.
//! It owns the connection loop, the module router, the event bus and the Provenance log.

mod builtin;

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::Arc;

use contracts::Actor;
use contracts::terminal::TerminalInfo;
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
        std::fs::create_dir_all(dir)?;
        let bus = Bus::new();
        let terminals = Arc::new(terminal::Terminals::open_with(
            dir,
            bus.clone(),
            &agents::claude_code::MARKERS,
        )?);
        let agents = Arc::new(agents::Agents::open(
            dir,
            bus.clone(),
            Arc::clone(&terminals),
        )?);
        let agents_module: Arc<dyn Module> = agents.clone();
        let mut daemon = Self {
            modules: HashMap::new(),
            touches: Arc::new(Touches::open(&dir.join("provenance.db"))?),
            bus: bus.clone(),
        };
        daemon.register(Arc::new(todos::Todos::open(dir, bus.clone())?));
        daemon.register(Arc::new(pads::Pads::open(dir, bus.clone())?));
        // Slice 3 maps `Agents::prompt` onto this; it does not exist yet, so nothing is typed.
        let deliver: messages::Deliver =
            Arc::new(|_, _| Box::pin(async { Err(messages::Refusal::NotFound) }));
        daemon.register(Arc::new(messages::Messages::open(
            dir,
            bus,
            agents_module,
            deliver,
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
        if let Some(outcome) = builtin::call(self, conn, namespace, method, &params) {
            return outcome;
        }
        let module = self
            .modules
            .get(namespace)
            .ok_or_else(|| RpcError::method_not_found(method))?;
        module
            .call(&self.ctx(conn.actor.clone()), method, params)
            .await
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
const WAITS_ON_ITS_CALLER: [&str; 1] = ["agent.permission"];

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
mod room_door;
