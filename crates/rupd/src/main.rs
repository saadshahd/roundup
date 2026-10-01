use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::net::{UnixListener, UnixStream};

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut args: Vec<_> = std::env::args_os().skip(1).collect();
    let attached = args.iter().any(|arg| arg == "--attached");
    args.retain(|arg| arg != "--attached");
    let project = args
        .first()
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
    let daemon = Arc::new(daemon);
    if !attached {
        return rupd::serve(listener, daemon).await;
    }
    // The App holds our stdin open; its exit, even a crash, closes it. Nothing is read from it.
    let stdin_closed =
        tokio::task::spawn_blocking(|| io::copy(&mut io::stdin().lock(), &mut io::sink()));
    tokio::select! {
        served = rupd::serve(listener, Arc::clone(&daemon)) => served,
        closed = stdin_closed => {
            closed.map_err(io::Error::other)??;
            eprintln!("rupd: stdin closed, stopping");
            daemon.stop_terminals().await.map_err(io::Error::other)
        }
    }
}
