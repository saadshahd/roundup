use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let Some(command) = std::env::args().nth(1) else {
        eprintln!("usage: rup ping");
        return ExitCode::from(2);
    };
    let method = match command.as_str() {
        "ping" => "daemon.ping",
        other => {
            eprintln!("rup: unknown command {other:?}");
            return ExitCode::from(2);
        }
    };
    let reply = match rpc::socket_path() {
        Ok(path) => rpc::call(&path, method).await,
        Err(err) => Err(err),
    };
    match reply {
        Ok(reply) => {
            println!("{reply}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("rup: {err}");
            ExitCode::FAILURE
        }
    }
}
