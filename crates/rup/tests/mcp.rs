use std::path::Path;
use std::process::{Output, Stdio};
use std::sync::Arc;
use std::time::Duration;

use contracts::{Actor, ActorKind, IdentifyParams, Touch, Verb, pad, todo};
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

const M1_METHODS: [&str; 13] = [
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
async fn m1_offers_one_tool_per_method_and_no_others() {
    let project = start_daemon();
    let shim = spawn_shim(&project.socket, "a1").await;

    let tools = shim.client.list_all_tools().await.unwrap();

    let mut names: Vec<_> = tools.iter().map(|t| t.name.to_string()).collect();
    let mut expected: Vec<_> = M1_METHODS.iter().map(|m| m.replace('.', "_")).collect();
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
    ];
    for (name, contract_schema) in expected {
        assert_eq!(schema(name), contract_schema, "{name}");
    }
    assert_eq!(schema("todo_list"), json!({ "type": "object" }));
    assert_eq!(schema("pad_list"), json!({ "type": "object" }));
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
