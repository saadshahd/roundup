use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::net::{UnixListener, UnixStream};

#[tokio::main]
async fn main() -> io::Result<()> {
    let project = std::env::args_os()
        .nth(1)
        .map_or_else(std::env::current_dir, |dir| Ok(PathBuf::from(dir)))?;
    let daemon = rupd::Daemon::open(&project.join(".roundup")).map_err(io::Error::other)?;

    let path = rpc::socket_path()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // A stale socket from a crashed run would make bind fail; a live one is caught by connect.
    if path.exists() && UnixStream::connect(&path).await.is_err() {
        std::fs::remove_file(&path)?;
    }
    let listener = UnixListener::bind(&path)?;
    eprintln!("rupd: serving {} on {}", project.display(), path.display());
    rupd::serve(listener, Arc::new(daemon)).await
}
