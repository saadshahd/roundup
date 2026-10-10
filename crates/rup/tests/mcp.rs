use std::path::Path;
use std::process::{Output, Stdio};
use std::sync::Arc;
use std::time::Duration;

use contracts::{Actor, ActorKind, IdentifyParams, Touch, Verb, message, pad, todo};
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult, ProtocolVersion};
use rmcp::service::{RoleClient, RunningService};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::process::Child;
use tokio::sync::mpsc;

const BOUND: Duration = Duration::from_secs(20);

struct Shim {
    client: RunningService<RoleClient, ()>,
    child: Child,
}

fn start_rup_mcp(socket: &Path, agent: &str) -> Child {
    tokio::process::Command::new(env!("CARGO_BIN_EXE_rup"))
        .args(["mcp", agent])
        .env("RUPD_SOCKET", socket)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap()
}

async fn spawn_shim(socket: &Path, agent: &str) -> Shim {
    let mut child = start_rup_mcp(socket, agent);
    let transport = (child.stdout.take().unwrap(), child.stdin.take().unwrap());
    let client = tokio::time::timeout(BOUND, ().serve(transport))
        .await
        .expect("the shim answered initialize")
        .unwrap();
    Shim { client, child }
}

impl Shim {
    async fn call(&self, tool: &'static str, arguments: Value) -> CallToolResult {
        let mut request = CallToolRequestParams::new(tool);
        if let Value::Object(map) = arguments {
            request = request.with_arguments(map);
        }
        tokio::time::timeout(BOUND, self.client.call_tool(request))
            .await
            .expect("the shim answered the call")
            .unwrap()
    }
}

fn text(result: &CallToolResult) -> Value {
    let raw = result.content[0]
        .as_text()
        .expect("a text result")
        .text
        .clone();
    serde_json::from_str(&raw).unwrap()
}

struct Project {
    _dir: tempfile::TempDir,
    socket: std::path::PathBuf,
    daemon: tokio::task::JoinHandle<std::io::Result<()>>,
}

fn start_daemon() -> Project {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("rupd.sock");
    let daemon = Arc::new(rupd::Daemon::open(&dir.path().join(".roundup")).unwrap());
    let daemon = tokio::spawn(rupd::serve(UnixListener::bind(&socket).unwrap(), daemon));
    Project {
        _dir: dir,
        socket,
        daemon,
    }
}

fn agent(id: &str) -> Actor {
    Actor {
        kind: ActorKind::Agent,
        id: id.into(),
        parent: None,
    }
}

async fn client_as(socket: &Path, actor: Actor) -> rpc::Client {
    let client = rpc::Client::connect(socket).await.unwrap();
    client
        .request("daemon.identify", IdentifyParams { actor })
        .await
        .unwrap();
    client
}

async fn write_line(to: &mut (impl AsyncWrite + Unpin), message: &Value) {
    to.write_all(format!("{message}\n").as_bytes())
        .await
        .unwrap();
}

/// A stand-in Daemon that answers `null` to the `answered` methods and never to any other.
/// It holds every connection open: a close would fail the shim's request at once instead of leaving it unanswered.
fn fake_daemon(socket: &Path, answered: &'static [&'static str]) -> mpsc::UnboundedReceiver<Value> {
    let listener = UnixListener::bind(socket).unwrap();
    let (seen, requests) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let seen = seen.clone();
            tokio::spawn(async move {
                let (read, mut write) = stream.into_split();
                let mut lines = BufReader::new(read).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let request: Value = serde_json::from_str(&line).unwrap();
                    let reply = answered
                        .iter()
                        .any(|method| request["method"] == *method)
                        .then(|| json!({ "jsonrpc": "2.0", "id": request["id"], "result": null }));
                    let _ = seen.send(request);
                    if let Some(reply) = reply {
                        write_line(&mut write, &reply).await;
                    }
                }
            });
        }
    });
    requests
}

async fn run_without_client(socket: &Path) -> Output {
    let mut child = start_rup_mcp(socket, "a1");
    drop(child.stdin.take());
    tokio::time::timeout(BOUND, child.wait_with_output())
        .await
        .expect("rup mcp exited")
        .unwrap()
}

fn assert_exit_1_with_one_line_naming(output: &Output, socket: &Path) {
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.contains(socket.to_str().unwrap()), "{stderr}");
}

const M1_METHODS: [&str; 18] = [
    "todo.create",
    "todo.get",
    "todo.list",
    "todo.update",
    "todo.complete",
    "todo.setBlockers",
    "pad.create",
    "pad.read",
    "pad.list",
    "pad.write",
    "pad.append",
    "pad.setOwner",
    "pad.delete",
    // B12
    "message.send",
    "message.get",
    "message.list",
    "message.pass",
    // E2
    "agent.context",
];

#[tokio::test]
async fn m1_the_server_is_named_roundup() {
    let project = start_daemon();
    let shim = spawn_shim(&project.socket, "a1").await;

    let info = shim.client.peer_info().expect("initialized");

    assert_eq!(
        info.server_info.as_ref().expect("server info").name,
        "roundup"
    );
}

#[tokio::test]
async fn m1_offers_one_tool_per_method_and_ask_user_and_no_others() {
    let project = start_daemon();
    let shim = spawn_shim(&project.socket, "a1").await;

    let tools = shim.client.list_all_tools().await.unwrap();

    let mut names: Vec<_> = tools.iter().map(|t| t.name.to_string()).collect();
    let mut expected: Vec<_> = M1_METHODS.iter().map(|m| m.replace('.', "_")).collect();
    // H14: `ask_user` is the one tool that is not a method's name.
    expected.push("ask_user".into());
    // O4: and `agent_set_order`, which keeps the snake case of the order it sets.
    expected.push("agent_set_order".into());
    names.sort();
    expected.sort();
    assert_eq!(names, expected);
}

#[tokio::test]
async fn m1_input_schemas_are_the_contract_schemas() {
    let project = start_daemon();
    let shim = spawn_shim(&project.socket, "a1").await;

    let tools = shim.client.list_all_tools().await.unwrap();
    let schema = |name: &str| {
        let tool = tools.iter().find(|t| t.name == name).unwrap();
        Value::Object((*tool.input_schema).clone())
    };

    let derived = |schema| serde_json::to_value(schema).unwrap();
    let expected = [
        (
            "todo_create",
            derived(schemars::schema_for!(todo::CreateParams)),
        ),
        ("todo_get", derived(schemars::schema_for!(todo::TodoId))),
        (
            "todo_update",
            derived(schemars::schema_for!(todo::UpdateParams)),
        ),
        (
            "todo_complete",
            derived(schemars::schema_for!(todo::TodoId)),
        ),
        (
            "todo_setBlockers",
            derived(schemars::schema_for!(todo::SetBlockersParams)),
        ),
        (
            "pad_create",
            derived(schemars::schema_for!(pad::CreateParams)),
        ),
        ("pad_read", derived(schemars::schema_for!(pad::PadName))),
        (
            "pad_write",
            derived(schemars::schema_for!(pad::WriteParams)),
        ),
        (
            "pad_append",
            derived(schemars::schema_for!(pad::AppendParams)),
        ),
        (
            "pad_setOwner",
            derived(schemars::schema_for!(pad::SetOwnerParams)),
        ),
        ("pad_delete", derived(schemars::schema_for!(pad::PadName))),
        (
            "message_send",
            derived(schemars::schema_for!(message::SendParams)),
        ),
        (
            "message_get",
            derived(schemars::schema_for!(message::MessageId)),
        ),
        (
            "message_list",
            derived(schemars::schema_for!(message::ListParams)),
        ),
        (
            "message_pass",
            derived(schemars::schema_for!(message::MessageId)),
        ),
    ];
    for (name, contract_schema) in expected {
        assert_eq!(schema(name), contract_schema, "{name}");
    }
    assert_eq!(schema("todo_list"), json!({ "type": "object" }));
    assert_eq!(schema("pad_list"), json!({ "type": "object" }));
    // E4: the shim fills the id, so the tool takes no input.
    assert_eq!(schema("agent_context"), json!({ "type": "object" }));
}

#[tokio::test]
async fn m1_list_tools_send_null_params_to_the_daemon() {
    for (tool, method) in [("todo_list", "todo.list"), ("pad_list", "pad.list")] {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("fake.sock");
        let mut requests = fake_daemon(&socket, &["daemon.identify", "todo.list", "pad.list"]);
        let shim = spawn_shim(&socket, "a1").await;

        shim.call(tool, json!({})).await;

        let sent = loop {
            let request = requests.recv().await.unwrap();
            if request["method"] == method {
                break request;
            }
        };
        assert_eq!(sent["params"], Value::Null, "{tool}");
    }
}

#[tokio::test]
async fn m2_a_created_todo_is_a_touch_by_the_calling_agent() {
    let project = start_daemon();
    let shim = spawn_shim(&project.socket, "a1").await;

    let result = shim.call("todo_create", json!({ "title": "x" })).await;

    assert_eq!(result.is_error, Some(false));
    let created = text(&result);
    assert_eq!(created["title"], "x");
    let user = client_as(&project.socket, Actor::user()).await;
    let history = user
        .request("provenance.history", json!({ "item": "todo:1" }))
        .await
        .unwrap();
    let touches: Vec<Touch> = serde_json::from_value(history).unwrap();
    assert!(
        touches
            .iter()
            .any(|t| t.verb == Verb::Wrote && t.actor == agent("a1"))
    );
}

#[tokio::test]
async fn b12_a_call_is_made_as_the_agent_and_returns_the_daemons_result() {
    let project = start_daemon();
    let shim = spawn_shim(&project.socket, "a1").await;

    // `from` is never a parameter: a forged one is ignored.
    let sent = shim
        .call(
            "message_send",
            json!({"to": "you", "kind": "note", "body": "hi", "from": {"kind": "user", "id": "you"}}),
        )
        .await;

    assert_eq!(sent.is_error, Some(false));
    let message = text(&sent);
    assert_eq!(message["id"], 1);
    assert_eq!(message["from"]["id"], "a1");
    assert_eq!(message["from"]["kind"], "agent");
    assert_eq!(message["status"], "delivered");
    assert_eq!(
        text(&shim.call("message_get", json!({"id": 1})).await),
        message
    );
    assert_eq!(
        text(&shim.call("message_list", json!({})).await),
        json!([message])
    );
    let other = spawn_shim(&project.socket, "a2").await;
    let refused = other.call("message_get", json!({"id": 1})).await;
    assert_eq!(refused.is_error, Some(true));
    assert_eq!(text(&refused)["code"], rpc::code::FORBIDDEN);
    let pass = shim.call("message_pass", json!({"id": 1})).await;
    assert_eq!(text(&pass)["code"], rpc::code::FORBIDDEN);
}

#[tokio::test]
async fn m2_a_daemon_error_is_a_tool_error_carrying_code_and_message() {
    let project = start_daemon();
    let owner = client_as(&project.socket, agent("owner")).await;
    owner
        .request("pad.create", json!({ "name": "notes" }))
        .await
        .unwrap();
    let shim = spawn_shim(&project.socket, "a1").await;

    let result = shim
        .call("pad_write", json!({ "name": "notes", "text": "mine" }))
        .await;

    assert_eq!(result.is_error, Some(true));
    let error = text(&result);
    assert_eq!(error["code"], rpc::code::FORBIDDEN);
    assert!(error["message"].is_string());
}

#[tokio::test]
async fn m2_the_server_keeps_serving_after_a_daemon_error() {
    let project = start_daemon();
    let shim = spawn_shim(&project.socket, "a1").await;
    let refused = shim.call("todo_get", json!({ "id": 99 })).await;
    assert_eq!(refused.is_error, Some(true));

    let result = shim.call("todo_create", json!({ "title": "after" })).await;

    assert_eq!(result.is_error, Some(false));
}

#[tokio::test]
async fn m3_no_daemon_exits_nonzero_with_one_line_naming_the_socket() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("absent.sock");

    let output = run_without_client(&socket).await;

    assert_exit_1_with_one_line_naming(&output, &socket);
}

#[tokio::test]
async fn m3_a_daemon_that_never_answers_exits_nonzero_with_one_line_naming_the_socket() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("mute.sock");
    let _requests = fake_daemon(&socket, &[]);

    let output = run_without_client(&socket).await;

    assert_exit_1_with_one_line_naming(&output, &socket);
}

#[tokio::test]
async fn m3_a_call_the_daemon_never_answers_exits_1_saying_it_may_have_been_applied() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("fake.sock");
    let _requests = fake_daemon(&socket, &["daemon.identify"]);
    let mut shim = spawn_shim(&socket, "a1").await;
    let peer = shim.client.peer().clone();

    tokio::spawn(async move {
        peer.call_tool_once(CallToolRequestParams::new("todo_list"))
            .await
    });

    let status = tokio::time::timeout(BOUND, shim.child.wait())
        .await
        .expect("the shim exited")
        .unwrap();
    assert_eq!(status.code(), Some(1));
    let mut stderr = String::new();
    shim.child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .await
        .unwrap();
    assert!(stderr.contains("may have been applied"), "{stderr}");
}

#[tokio::test]
async fn m3_stdout_carries_only_protocol_messages_up_to_the_exit() {
    let project = start_daemon();
    let mut child = start_rup_mcp(&project.socket, "a1");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines();
    for message in [
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": ProtocolVersion::LATEST_WITH_INITIALIZE.as_str(),
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "0" },
        }}),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "todo_create", "arguments": { "title": "x" } } }),
    ] {
        write_line(&mut stdin, &message).await;
    }
    let mut lines = Vec::new();
    loop {
        let line = tokio::time::timeout(BOUND, stdout.next_line())
            .await
            .expect("the shim answered todo_create")
            .unwrap()
            .expect("stdout is open until the shim answers todo_create");
        let answers_create = serde_json::from_str::<Value>(&line).is_ok_and(|m| m["id"] == 2);
        lines.push(line);
        if answers_create {
            break;
        }
    }
    project.daemon.abort();
    let _ = project.daemon.await;

    write_line(
        &mut stdin,
        &json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "todo_list" } }),
    )
    .await;

    while let Some(line) = tokio::time::timeout(BOUND, stdout.next_line())
        .await
        .expect("the shim closed stdout")
        .unwrap()
    {
        lines.push(line);
    }
    let status = tokio::time::timeout(BOUND, child.wait())
        .await
        .expect("the shim exited")
        .unwrap();
    assert_eq!(status.code(), Some(1));
    for line in &lines {
        let message: Value = serde_json::from_str(line).unwrap_or_else(|_| panic!("{line}"));
        assert_eq!(message["jsonrpc"], "2.0", "{line}");
    }
}

/// Raw `tools/list` over stdio, against a Daemon that only answers `daemon.identify`: listing
/// tools never touches the Daemon, so this stand-in is enough. Returns the reply's `result`.
async fn raw_tools_list(socket: &Path, cursor: Option<&str>) -> Value {
    let mut child = start_rup_mcp(socket, "a1");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut params = json!({});
    if let Some(cursor) = cursor {
        params = json!({ "cursor": cursor });
    }
    for message in [
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": ProtocolVersion::LATEST_WITH_INITIALIZE.as_str(),
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "0" },
        }}),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": params }),
    ] {
        write_line(&mut stdin, &message).await;
    }
    loop {
        let line = tokio::time::timeout(BOUND, stdout.next_line())
            .await
            .expect("the shim answered tools/list")
            .unwrap()
            .expect("stdout is open until the shim answers tools/list");
        let message: Value = serde_json::from_str(&line).unwrap();
        if message["id"] == 2 {
            return message["result"].clone();
        }
    }
}

/// The `tools/list` result schema the interactive Claude Code 2.1.288 validates a reply against,
/// read from its own (minified) source: `ttlMs` a non-negative integer, `cacheScope` the enum
/// `public`/`private`, `tools` an array, `nextCursor` an optional string. Extracted with:
///
///   strings -n 4 ~/.local/share/claude/versions/2.1.288 \
///     | grep -o '"tools/list":Re({[^}]*}[^)]*)'
///
/// which prints (`claude --version` there is `2.1.288 (Claude Code)`):
///
///   "tools/list":Re({ttlMs:k().int().min(0),cacheScope:j(["public","private"]),tools:C(Ge),nextCursor:s.optional()})
///
/// (`k()` is a bound `z.number()`, `j([...])` is `z.enum([...])`, `C()` is `z.array()`, `s` is a
/// bound `z.string()`.) The same command against `~/.local/share/claude/versions/2.1.283`
/// (`claude --version` there is `2.1.283 (Claude Code)`, the headless build M4 says accepts the
/// older reply) extracts the same schema under other minified names (`cacheScope:G([...])`,
/// `tools:A(Ge)`), so the two builds differ in what they tolerate, not in what they validate
/// against. A later `claude` that adds a new required field, narrows `cacheScope`'s enum so it
/// excludes `public`, or tightens `ttlMs` (a minimum above 0 or a maximum) makes this reply
/// invalid: `/mcp` reconnecting against the real client (M4's laptop check) would fail while every
/// test here stayed green, so re-extract the schema and update the copy when `claude` is upgraded.
fn assert_matches_tools_list_schema(reply: &Value) {
    let ttl_ms = reply
        .get("ttlMs")
        .unwrap_or_else(|| panic!("no ttlMs: {reply}"));
    assert!(
        ttl_ms.is_u64(),
        "ttlMs must be an integer >= 0 (zod `k().int().min(0)`): {reply}"
    );
    let cache_scope = reply
        .get("cacheScope")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("no string cacheScope: {reply}"));
    assert!(
        matches!(cache_scope, "public" | "private"),
        "cacheScope must be \"public\" or \"private\" (zod `j([\"public\",\"private\"])`): {reply}"
    );
    assert!(
        reply.get("tools").is_some_and(Value::is_array),
        "no tools array: {reply}"
    );
    if let Some(next_cursor) = reply.get("nextCursor") {
        assert!(
            next_cursor.is_string(),
            "nextCursor must be a string when present: {reply}"
        );
    }
}

#[tokio::test]
async fn m4_the_full_tools_list_carries_a_numeric_ttl_ms_and_cache_scope() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("fake.sock");
    let _requests = fake_daemon(&socket, &["daemon.identify"]);

    let result = raw_tools_list(&socket, None).await;

    assert_matches_tools_list_schema(&result);
    // The methods' tools, `ask_user` (H14) and `agent_set_order` (O4).
    assert_eq!(
        result["tools"].as_array().unwrap().len(),
        M1_METHODS.len() + 2
    );
    assert_eq!(result["cacheScope"], "public", "{result}");
}

#[tokio::test]
async fn m4_the_empty_list_a_cursor_gets_carries_a_numeric_ttl_ms_and_cache_scope() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("fake.sock");
    let _requests = fake_daemon(&socket, &["daemon.identify"]);

    let result = raw_tools_list(&socket, Some("past-the-end")).await;

    assert_matches_tools_list_schema(&result);
    assert_eq!(result["tools"].as_array().unwrap().len(), 0);
    assert_eq!(result["cacheScope"], "public", "{result}");
}

#[tokio::test]
async fn m3_the_call_after_the_daemon_goes_away_makes_the_shim_exit_nonzero() {
    let project = start_daemon();
    let mut shim = spawn_shim(&project.socket, "a1").await;
    project.daemon.abort();
    let _ = project.daemon.await;

    let _ = tokio::time::timeout(
        BOUND,
        shim.client
            .call_tool(CallToolRequestParams::new("todo_list")),
    )
    .await;

    let status = tokio::time::timeout(BOUND, shim.child.wait())
        .await
        .expect("the shim exited")
        .unwrap();
    assert!(!status.success());
}

/// A stand-in Daemon that answers `agent.ask` with `{"answer": "dogs"}` after `delay` and `null` to `daemon.identify`.
fn asking_daemon(socket: &Path, delay: Duration) -> mpsc::UnboundedReceiver<Value> {
    let listener = UnixListener::bind(socket).unwrap();
    let (seen, requests) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let seen = seen.clone();
            tokio::spawn(async move {
                let (read, mut write) = stream.into_split();
                let mut lines = BufReader::new(read).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let request: Value = serde_json::from_str(&line).unwrap();
                    let result = match request["method"].as_str() {
                        Some("agent.ask") => {
                            tokio::time::sleep(delay).await;
                            json!({ "answer": "dogs" })
                        }
                        _ => Value::Null,
                    };
                    let reply = json!({ "jsonrpc": "2.0", "id": request["id"], "result": result });
                    let _ = seen.send(request);
                    write_line(&mut write, &reply).await;
                }
            });
        }
    });
    requests
}

/// The call outlasts the ten seconds every other tool call is bound by (the user may take minutes) and returns the answer's text, not JSON.
#[tokio::test]
async fn h14_ask_user_waits_past_the_call_bound_and_returns_the_answer_text() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("fake.sock");
    let mut requests = asking_daemon(&socket, Duration::from_secs(11));
    let shim = spawn_shim(&socket, "a1").await;

    let result = shim
        .call(
            "ask_user",
            json!({"question": "Cats or dogs?", "answers": ["cats", "dogs"]}),
        )
        .await;

    assert_eq!(result.is_error, Some(false));
    assert_eq!(result.content[0].as_text().unwrap().text, "dogs");
    let sent = loop {
        let request = requests.recv().await.unwrap();
        if request["method"] == "agent.ask" {
            break request;
        }
    };
    assert_eq!(
        sent["params"],
        json!({"question": "Cats or dogs?", "answers": ["cats", "dogs"]})
    );
}

#[tokio::test]
async fn h14_ask_user_schema_is_the_contract_schema() {
    let project = start_daemon();
    let shim = spawn_shim(&project.socket, "a1").await;

    let tools = shim.client.list_all_tools().await.unwrap();

    let tool = tools.iter().find(|t| t.name == "ask_user").unwrap();
    assert_eq!(
        Value::Object((*tool.input_schema).clone()),
        serde_json::to_value(schemars::schema_for!(contracts::decision::AskParams)).unwrap()
    );
}

#[tokio::test]
async fn u148_a_block_written_through_the_tools_is_stored_byte_exact() {
    let project = start_daemon();
    let shim = spawn_shim(&project.socket, "a1").await;
    let block = "```mermaid\ngraph TD\n  A[\"<b>x</b>\"] --> B\n```\n";
    let drawing = "```excalidraw\n{\"type\":\"excalidraw\",\"version\":2,\"elements\":[]}\n```\n";
    let user = client_as(&project.socket, Actor::user()).await;
    let stored = |name: &'static str| {
        let user = &user;
        async move {
            user.request("pad.read", json!({ "name": name }))
                .await
                .unwrap()["text"]
                .as_str()
                .unwrap()
                .to_string()
        }
    };

    let created = shim
        .call("pad_create", json!({ "name": "notes", "text": block }))
        .await;
    assert_eq!(created.is_error, Some(false));
    assert_eq!(stored("notes").await, block);

    let written = shim
        .call("pad_write", json!({ "name": "notes", "text": drawing }))
        .await;
    assert_eq!(written.is_error, Some(false));
    assert_eq!(stored("notes").await, drawing);

    let appended = shim
        .call("pad_append", json!({ "name": "notes", "text": block }))
        .await;
    assert_eq!(appended.is_error, Some(false));
    assert_eq!(stored("notes").await, format!("{drawing}{block}"));

    let out = tempfile::tempdir().unwrap();
    let target = out.path().join("notes.md");
    user.request("pad.export", json!({ "name": "notes", "path": target }))
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        format!("{drawing}{block}")
    );
}

#[tokio::test]
async fn u149_a_drawing_written_through_the_tools_is_stored_byte_exact() {
    let project = start_daemon();
    let shim = spawn_shim(&project.socket, "a1").await;
    let block = "```excalidraw\n{\"type\":\"excalidraw\",\"version\":2,\"elements\":[{\"type\":\"rectangle\",\"id\":\"a\",\"x\":0,\"y\":0,\"width\":160,\"height\":80,\"label\":{\"text\":\"plan\"}},{\"type\":\"text\",\"id\":\"t\",\"x\":220,\"y\":28,\"text\":\"ship\"},{\"type\":\"arrow\",\"x\":160,\"y\":40,\"width\":60,\"height\":0,\"start\":{\"id\":\"a\"},\"end\":{\"id\":\"t\"}}]}\n```\n";
    let drawing = "```excalidraw\n{\"type\":\"excalidraw\",\"version\":2,\"elements\":[]}\n```\n";
    let user = client_as(&project.socket, Actor::user()).await;
    let stored = |name: &'static str| {
        let user = &user;
        async move {
            user.request("pad.read", json!({ "name": name }))
                .await
                .unwrap()["text"]
                .as_str()
                .unwrap()
                .to_string()
        }
    };

    let created = shim
        .call("pad_create", json!({ "name": "notes", "text": block }))
        .await;
    assert_eq!(created.is_error, Some(false));
    assert_eq!(stored("notes").await, block);

    let written = shim
        .call("pad_write", json!({ "name": "notes", "text": drawing }))
        .await;
    assert_eq!(written.is_error, Some(false));
    assert_eq!(stored("notes").await, drawing);

    let appended = shim
        .call("pad_append", json!({ "name": "notes", "text": block }))
        .await;
    assert_eq!(appended.is_error, Some(false));
    assert_eq!(stored("notes").await, format!("{drawing}{block}"));

    let out = tempfile::tempdir().unwrap();
    let target = out.path().join("notes.md");
    user.request("pad.export", json!({ "name": "notes", "path": target }))
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        format!("{drawing}{block}")
    );

async fn o4_agent_set_order_fills_its_own_id_and_takes_only_the_order() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("fake.sock");
    let mut requests = fake_daemon(
        &socket,
        &["daemon.identify", "agent.channelUp", "agent.setOrder"],
    );
    let shim = spawn_shim(&socket, "a1").await;

    let tools = shim.client.list_all_tools().await.unwrap();
    let tool = tools.iter().find(|t| t.name == "agent_set_order").unwrap();
    let schema = Value::Object((*tool.input_schema).clone());
    assert!(schema["properties"].get("id").is_none(), "{schema}");
    assert!(schema["properties"].get("order").is_some(), "{schema}");

    let order = json!({"kind": "work", "ask": "fix it", "limits": ["push"], "id": "other"});
    shim.call("agent_set_order", json!({ "order": order }))
        .await;

    while let Some(request) = requests.recv().await {
        if request["method"] == "agent.setOrder" {
            assert_eq!(request["params"]["id"], "a1");
            assert_eq!(request["params"]["order"]["ask"], "fix it");
            return;
        }
    }
    panic!("agent.setOrder was never sent");
}
