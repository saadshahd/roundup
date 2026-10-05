use std::io::Read;
use std::process::ExitCode;
use std::time::Duration;

use contracts::agent::SignalParams;
use contracts::{Actor, ActorKind};
use serde_json::{Value, json};

/// How long `rup signal` waits for the Daemon: well inside the 5 s Claude Code gives a hook.
const SIGNAL_DEADLINE: Duration = Duration::from_secs(1);

/// What `rup signal` says when it cannot know whether the Daemon took the Signal.
const MAYBE_ARRIVED: &str = "the Signal may or may not have arrived";

mod mcp;

/// H15: `rup` never runs more than one task at a time, so one OS thread is enough; the thread test in
/// `crates/rup/tests/signal.rs` fails if tokio starts a second.
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["ping"] => ping().await,
        ["signal", agent_id] => signal(agent_id).await,
        // Exit 2 would tell Claude Code to block its tool call or prompt; `signal` only ever exits 1.
        ["signal", ..] => Err("usage: rup signal <agent-id>".into()),
        ["mcp"] => return mcp::run(None).await,
        ["mcp", agent_id, ..] => return mcp::run(Some(agent_id.to_owned())).await,
        [] => return usage(),
        [other, ..] => {
            eprintln!("rup: unknown command {other:?}");
            return ExitCode::from(2);
        }
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("rup: {err}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> ExitCode {
    eprintln!("usage: rup ping | rup signal <agent-id> | rup mcp <agent-id>");
    ExitCode::from(2)
}

async fn connect() -> Result<rpc::Client, String> {
    let socket = rpc::socket_path().map_err(|err| err.to_string())?;
    rpc::Client::connect(&socket)
        .await
        .map_err(|err| err.to_string())
}

async fn ping() -> Result<(), String> {
    let reply = connect()
        .await?
        .request("daemon.ping", Value::Null)
        .await
        .map_err(|err| err.to_string())?;
    println!("{}", json!({ "result": reply }));
    Ok(())
}

/// Claude Code runs this as the Agent's command hook: one payload on stdin is one Signal.
async fn signal(agent_id: &str) -> Result<(), String> {
    let attempt =
        std::env::var("ROUNDUP_ATTEMPT").map_err(|_| "ROUNDUP_ATTEMPT is required".to_owned())?;
    if contracts::agent::parse_positive_ordinal(&attempt).is_none() {
        return Err("ROUNDUP_ATTEMPT must be a canonical positive signed 64-bit decimal".into());
    }
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|err| format!("cannot read the payload: {err}"))?;
    let payload =
        serde_json::from_str(&input).map_err(|err| format!("the payload is not JSON: {err}"))?;
    tokio::time::timeout(SIGNAL_DEADLINE, deliver(agent_id, attempt, payload))
        .await
        .map_err(|_| {
            format!(
                "rupd did not answer within {} s; {MAYBE_ARRIVED}",
                SIGNAL_DEADLINE.as_secs()
            )
        })?
}

/// Identify as Agent `agent_id` and hand the Daemon its Signal.
async fn deliver(agent_id: &str, attempt: String, payload: Value) -> Result<(), String> {
    let client = connect().await?;
    let actor = Actor {
        kind: ActorKind::Agent,
        id: agent_id.to_owned(),
        parent: None,
    };
    client
        .request("daemon.identify", json!({ "actor": actor }))
        .await
        .map_err(|err| err.to_string())?;
    let signal = SignalParams {
        id: agent_id.to_owned(),
        attempt,
        payload,
    };
    client
        .request("agent.signal", signal)
        .await
        .map(drop)
        .map_err(|err| match err.code {
            // The connection closed mid-call: the Daemon may have taken the Signal already.
            rpc::code::UNKNOWN_OUTCOME => format!("{err}; {MAYBE_ARRIVED}"),
            _ => err.to_string(),
        })
}
