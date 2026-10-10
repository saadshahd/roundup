//! `rup mcp <agent-id>`: Todos, Pads and Messages as MCP tools for one Agent, over stdio.
//! Every call reaches the Daemon on its own connection identified as that Agent, so it is a Touch by that Agent.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use contracts::{Actor, ActorKind, IdentifyParams, agent, decision, message, pad, todo};
use rmcp::model::{
    CacheScope, CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock,
    Implementation, JsonObject, ListToolsResult, PaginatedRequestParams, ServerCapabilities,
    ServerConfig, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData, ServerHandler, ServiceExt};
use rpc::RpcError;
use schemars::JsonSchema;
use serde_json::Value;
use tokio::sync::mpsc;

/// A Todo or Pad call is a few SQLite statements; past this the Daemon is wedged, and a hung tool call would hang the Agent's turn.
const CALL_TIMEOUT: Duration = Duration::from_secs(10);

/// `ttlMs` and `cacheScope` are required by the client's `tools/list` schema (M4). The tool set is fixed for
/// the life of this process, so the reply is `public` and fresh for an hour.
const TOOLS_TTL_MS: u64 = 3_600_000;

struct Offered {
    method: &'static str,
    tool: Tool,
    takes_params: bool,
    /// E4: the params are `{id}` with the shim's own Agent id, and the tool takes no input.
    own_id: bool,
    /// H14: the call waits for the user, so no `CALL_TIMEOUT` applies and its result is the answer's text.
    waits_for_user: bool,
}

/// The Daemon cannot be spoken to; the shim exits instead of answering the Agent.
#[derive(Debug)]
enum Gone {
    Unreachable(String),
    /// The method was sent and got no answer, so it may or may not have been applied.
    Silent,
}

impl std::fmt::Display for Gone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Gone::Unreachable(why) => f.write_str(why),
            Gone::Silent => write!(
                f,
                "no answer within {}s; the call may have been applied",
                CALL_TIMEOUT.as_secs()
            ),
        }
    }
}

fn offered<P: JsonSchema>(method: &'static str, description: &'static str) -> Offered {
    let Ok(Value::Object(schema)) = serde_json::to_value(schemars::schema_for!(P)) else {
        unreachable!("a params struct's schema is an object");
    };
    offer(method, description, schema, true)
}

/// H14: the one tool that is not a method's name with the dot replaced; the Agent asks the user and the call returns the answer.
fn ask_user() -> Offered {
    let Ok(Value::Object(schema)) =
        serde_json::to_value(schemars::schema_for!(decision::AskParams))
    else {
        unreachable!("a params struct's schema is an object");
    };
    Offered {
        method: "agent.ask",
        tool: Tool::new(
            "ask_user",
            "Ask the user a question with one to four short answers to pick from. Waits for the user and returns the answer they picked.",
            schema,
        ),
        takes_params: true,
        own_id: false,
        waits_for_user: true,
    }
}

fn offered_without_params(method: &'static str, description: &'static str) -> Offered {
    let schema = JsonObject::from_iter([("type".into(), "object".into())]);
    offer(method, description, schema, false)
}

fn offer(
    method: &'static str,
    description: &'static str,
    schema: JsonObject,
    takes_params: bool,
) -> Offered {
    Offered {
        method,
        tool: Tool::new(method.replace('.', "_"), description, schema),
        takes_params,
        own_id: false,
        waits_for_user: false,
    }
}

/// E4: `agent.context` takes `{id}`; the shim fills it, so the Agent's tool takes no input.
fn agent_context() -> Offered {
    Offered {
        own_id: true,
        ..offered_without_params(
            "agent.context",
            "Ask where you stand now: whom you report to, whom to ask, your peers and your open Todos.",
        )
    }
}

/// O4: `agent.setOrder` takes `{id, order}`; the shim fills `id`, so the Agent's tool takes only
/// `order`.
fn agent_set_order() -> Offered {
    let Ok(Value::Object(mut schema)) =
        serde_json::to_value(schemars::schema_for!(agent::SetOrderParams))
    else {
        unreachable!("a params struct's schema is an object");
    };
    if let Some(Value::Object(properties)) = schema.get_mut("properties") {
        properties.remove("id");
    }
    if let Some(Value::Array(required)) = schema.get_mut("required") {
        required.retain(|name| name != "id");
    }
    Offered {
        own_id: true,
        tool: Tool::new(
            "agent_set_order",
            "Replace your own order: a Work order once the ask is clear, or a Clarification order while a question is open.",
            schema,
        ),
        ..offer("agent.setOrder", "", JsonObject::new(), true)
    }
}

/// `todo.delete` (a Todo has no owner to guard it), `todo.reorder` (the order is the user's), `pad.export` (writes outside the Project) and `pad.setStorage` (the user's setting) are left out on purpose.
fn offered_tools() -> Vec<Offered> {
    vec![
        offered::<todo::CreateParams>("todo.create", "Create a Todo."),
        offered::<todo::TodoId>("todo.get", "Read one Todo."),
        offered_without_params("todo.list", "List every Todo."),
        offered::<todo::UpdateParams>("todo.update", "Change a Todo's title or body."),
        offered::<todo::TodoId>("todo.complete", "Mark a Todo done."),
        offered::<todo::SetBlockersParams>(
            "todo.setBlockers",
            "Replace a Todo's whole blocker list.",
        ),
        offered::<pad::CreateParams>("pad.create", "Create a Pad you own."),
        offered::<pad::PadName>("pad.read", "Read one Pad."),
        offered_without_params("pad.list", "List every Pad."),
        offered::<pad::WriteParams>("pad.write", "Rewrite a Pad's whole text. Owner only."),
        offered::<pad::AppendParams>("pad.append", "Add text to the end of any Pad."),
        offered::<pad::SetOwnerParams>("pad.setOwner", "Give a Pad to another Actor."),
        offered::<pad::PadName>("pad.delete", "Delete a Pad."),
        offered::<message::SendParams>(
            "message.send",
            "Send a note or a question to an Agent or the user. You are the sender.",
        ),
        offered::<message::MessageId>("message.get", "Read one Message you sent or received."),
        offered::<message::ListParams>(
            "message.list",
            "List the Messages you sent or received, newest last.",
        ),
        offered::<message::MessageId>(
            "message.pass",
            "Pass a question sent to you on to the next Door above, at once.",
        ),
        agent_context(),
        agent_set_order(),
        ask_user(),
    ]
}

struct Shim {
    socket: PathBuf,
    actor: Actor,
    tools: Vec<Offered>,
    gone: mpsc::Sender<Gone>,
}

impl Shim {
    async fn connect(&self) -> Result<rpc::Client, Gone> {
        let socket = self.socket.display();
        let handshake = async {
            let client = rpc::Client::connect(&self.socket)
                .await
                .map_err(|err| Gone::Unreachable(format!("no Daemon at {socket}: {err}")))?;
            client
                .request(
                    "daemon.identify",
                    IdentifyParams {
                        actor: self.actor.clone(),
                    },
                )
                .await
                .map_err(|err| {
                    Gone::Unreachable(format!("the Daemon at {socket} refused identify: {err}"))
                })?;
            Ok(client)
        };
        tokio::time::timeout(CALL_TIMEOUT, handshake)
            .await
            .map_err(|_| {
                Gone::Unreachable(format!(
                    "no answer from the Daemon at {socket} within {}s",
                    CALL_TIMEOUT.as_secs()
                ))
            })?
    }

    /// E6: tell the Daemon this Agent's `rup mcp` is up, once, on the start connection (after its
    /// `daemon.identify`) and before any tool call. A refusal or silence is one line on stderr:
    /// the tools still work, and the Daemon's Channel says `missing` on its own.
    async fn report_channel(&self, start: &rpc::Client) {
        let up = start.request(
            "agent.channelUp",
            contracts::agent::NodeId {
                id: self.actor.id.clone(),
            },
        );
        match tokio::time::timeout(CALL_TIMEOUT, up).await {
            Ok(Ok(_)) => {}
            Ok(Err(err)) => eprintln!("rup: the Daemon did not take the Channel report: {err}"),
            Err(_) => eprintln!(
                "rup: no answer to the Channel report within {}s",
                CALL_TIMEOUT.as_secs()
            ),
        }
    }

    async fn call(
        &self,
        offer: &Offered,
        arguments: Option<JsonObject>,
    ) -> Result<Result<Value, RpcError>, Gone> {
        let params = match (offer.takes_params, offer.own_id, arguments) {
            (false, true, _) => serde_json::json!({ "id": self.actor.id }),
            (true, true, arguments) => {
                let mut arguments = arguments.unwrap_or_default();
                arguments.insert("id".into(), self.actor.id.clone().into());
                Value::Object(arguments)
            }
            (true, false, arguments) => Value::Object(arguments.unwrap_or_default()),
            (false, false, _) => Value::Null,
        };
        let client = self.connect().await?;
        if offer.waits_for_user {
            return Ok(client.request(offer.method, params).await);
        }
        tokio::time::timeout(CALL_TIMEOUT, client.request(offer.method, params))
            .await
            .map_err(|_| Gone::Silent)
    }
}

impl ServerHandler for Shim {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("roundup", env!("CARGO_PKG_VERSION")))
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        // Every tool is always in the one page handed out with no cursor; a cursor asks for
        // whatever comes after that page, which is nothing.
        let tools = match request.and_then(|r| r.cursor) {
            Some(_) => Vec::new(),
            None => self.tools.iter().map(|offer| offer.tool.clone()).collect(),
        };
        Ok(ListToolsResult::with_all_items(tools)
            .with_ttl_ms(TOOLS_TTL_MS)
            .with_cache_scope(CacheScope::Public))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let offer = self
            .tools
            .iter()
            .find(|offer| offer.tool.name == request.name)
            .ok_or_else(|| {
                ErrorData::invalid_params(format!("no tool named {}", request.name), None)
            })?;
        match self.call(offer, request.arguments).await {
            Ok(Ok(result)) => {
                let text = match result["answer"].as_str() {
                    Some(answer) if offer.waits_for_user => answer.to_owned(),
                    _ => result.to_string(),
                };
                Ok(CallToolResult::success(vec![ContentBlock::text(text)]).into())
            }
            Ok(Err(err)) => {
                let body = serde_json::to_string(&err)
                    .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::error(vec![ContentBlock::text(body)]).into())
            }
            Err(gone) => {
                let message = gone.to_string();
                // A full channel means the shim is already exiting.
                let _ = self.gone.try_send(gone);
                Err(ErrorData::internal_error(message, None))
            }
        }
    }
}

pub async fn run(agent_id: Option<String>) -> ExitCode {
    let Some(id) = agent_id else {
        eprintln!("usage: rup mcp <agent-id>");
        return ExitCode::from(2);
    };
    match serve(id).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("rup: {why}");
            // Returning would block in runtime shutdown on the pending stdin read until the client closes the pipe.
            std::process::exit(1)
        }
    }
}

async fn serve(id: String) -> Result<(), String> {
    let socket = rpc::socket_path().map_err(|err| err.to_string())?;
    let (gone, mut gone_rx) = mpsc::channel(1);
    let shim = Shim {
        socket,
        actor: Actor {
            kind: ActorKind::Agent,
            id,
            parent: None,
        },
        tools: offered_tools(),
        gone,
    };
    let start = shim.connect().await.map_err(|gone| gone.to_string())?;
    shim.report_channel(&start).await;
    drop(start);
    let running = shim
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|err| err.to_string())?;
    tokio::select! {
        ended = running.waiting() => ended.map(|_| ()).map_err(|err| err.to_string()),
        Some(gone) = gone_rx.recv() => Err(gone.to_string()),
    }
}
