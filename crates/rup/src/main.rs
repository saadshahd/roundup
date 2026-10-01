use std::process::ExitCode;

mod mcp;

#[tokio::main]
async fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        eprintln!("usage: rup ping | rup mcp <agent-id>");
        return ExitCode::from(2);
    };
    if command == "mcp" {
        return mcp::run(args.next()).await;
    }
    let method = match command.as_str() {
        "ping" => "daemon.ping",
        other => {
            eprintln!("rup: unknown command {other:?}");
            return ExitCode::from(2);
        }
    };
    let reply = match rpc::socket_path() {
        Ok(path) => match rpc::Client::connect(&path).await {
            Ok(client) => client
                .request(method, serde_json::Value::Null)
                .await
                .map_err(|err| err.to_string()),
            Err(err) => Err(err.to_string()),
        },
        Err(err) => Err(err.to_string()),
    };
    match reply {
        Ok(reply) => {
            println!("{}", serde_json::json!({ "result": reply }));
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("rup: {err}");
            ExitCode::FAILURE
        }
    }
}
