//! The Daemon: every other part of roundup is a client of it.

use std::io;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

const PARSE_ERROR: i64 = -32700;
const METHOD_NOT_FOUND: i64 = -32601;

/// Accept connections until the listener fails.
pub async fn serve(listener: UnixListener) -> io::Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(err) = handle(stream).await {
                eprintln!("rupd: connection failed: {err}");
            }
        });
    }
}

async fn handle(stream: UnixStream) -> io::Result<()> {
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    while let Some(line) = lines.next_line().await? {
        write
            .write_all(format!("{}\n", respond(&line)).as_bytes())
            .await?;
    }
    Ok(())
}

/// One request line in, one response line out.
pub fn respond(line: &str) -> Value {
    let Ok(request) = serde_json::from_str::<Value>(line) else {
        return error(Value::Null, PARSE_ERROR, "parse error");
    };
    let id = request["id"].clone();
    match request["method"].as_str() {
        Some("daemon.ping") => json!({ "jsonrpc": "2.0", "id": id, "result": { "pong": true } }),
        _ => error(id, METHOD_NOT_FOUND, "method not found"),
    }
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_answers_pong() {
        let reply = respond(r#"{"jsonrpc":"2.0","id":7,"method":"daemon.ping"}"#);
        assert_eq!(reply["id"], 7);
        assert_eq!(reply["result"]["pong"], true);
    }

    #[test]
    fn unknown_method_is_an_error() {
        let reply = respond(r#"{"jsonrpc":"2.0","id":1,"method":"nope"}"#);
        assert_eq!(reply["error"]["code"], METHOD_NOT_FOUND);
    }

    #[test]
    fn garbage_is_a_parse_error() {
        assert_eq!(respond("{")["error"]["code"], PARSE_ERROR);
    }
}
