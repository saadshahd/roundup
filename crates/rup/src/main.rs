use std::io::Read;
use std::process::ExitCode;

use contracts::agent::SignalParams;
use contracts::{Actor, ActorKind};
use serde_json::{Value, json};

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["ping"] => ping().await,
        ["hook", agent_id] => hook(agent_id).await,
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
    eprintln!("usage: rup ping | rup hook <agent-id>");
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
async fn hook(agent_id: &str) -> Result<(), String> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|err| format!("cannot read the payload: {err}"))?;
    let payload =
        serde_json::from_str(&input).map_err(|err| format!("the payload is not JSON: {err}"))?;

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
        payload,
    };
    client
        .request("agent.signal", signal)
        .await
        .map(drop)
        .map_err(|err| err.to_string())
}
