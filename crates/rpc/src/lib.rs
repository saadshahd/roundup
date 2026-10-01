//! Shared by the Daemon and its clients: where the socket lives and how to call it.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

/// `RUPD_SOCKET` if set, else `~/.roundup/rupd.sock`.
pub fn socket_path() -> io::Result<PathBuf> {
    if let Some(path) = std::env::var_os("RUPD_SOCKET") {
        return Ok(path.into());
    }
    let home = std::env::var_os("HOME")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
    Ok(PathBuf::from(home).join(".roundup/rupd.sock"))
}

/// One JSON-RPC 2.0 request over a newline-delimited Unix socket; returns the whole response.
pub async fn call(socket: &Path, method: &str) -> io::Result<Value> {
    let mut stream = UnixStream::connect(socket).await?;
    let request = json!({ "jsonrpc": "2.0", "id": 1, "method": method });
    stream.write_all(format!("{request}\n").as_bytes()).await?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).await?;
    serde_json::from_str(&line).map_err(io::Error::other)
}
