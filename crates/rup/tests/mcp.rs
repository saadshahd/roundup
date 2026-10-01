use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use contracts::{Actor, ActorKind, IdentifyParams, Touch, Verb, pad, todo};
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::{RoleClient, RunningService};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::process::Child;

const BOUND: Duration = Duration::from_secs(20);

struct Shim {
    client: RunningService<RoleClient, ()>,
    child: Child,
}

async fn spawn_shim(socket: &Path, agent: &str) -> Shim {
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_rup"))
        .args(["mcp", agent])
        .env("RUPD_SOCKET", socket)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
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
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("fake.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let seen = tokio::spawn(async move {
        let mut seen = Vec::new();
        for _ in 0..2 {
            let (stream, _) = listener.accept().await.unwrap();
            let (read, mut write) = stream.into_split();
            let mut lines = BufReader::new(read).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let request: Value = serde_json::from_str(&line).unwrap();
                let result = if request["method"] == "todo.list" {
                    json!([])
                } else {
                    Value::Null
                };
                let reply = json!({ "jsonrpc": "2.0", "id": request["id"], "result": result });
                write
                    .write_all(format!("{reply}\n").as_bytes())
                    .await
                    .unwrap();
                seen.push(request);
                if seen.last().unwrap()["method"] == "todo.list" {
                    return seen;
                }
            }
        }
        seen
    });
    let shim = spawn_shim(&socket, "a1").await;

    shim.call("todo_list", json!({})).await;

    let seen = seen.await.unwrap();
    let list = seen.last().unwrap();
    assert_eq!(list["method"], "todo.list");
    assert_eq!(list["params"], Value::Null);
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

    let output = tokio::time::timeout(
        BOUND,
        tokio::process::Command::new(env!("CARGO_BIN_EXE_rup"))
            .args(["mcp", "a1"])
            .env("RUPD_SOCKET", &socket)
            .stdin(Stdio::null())
            .output(),
    )
    .await
    .expect("rup mcp exited")
    .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.lines().count(), 1);
    assert!(stderr.contains(socket.to_str().unwrap()));
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
