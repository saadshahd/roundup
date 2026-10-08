use std::io::{self, Write};
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
    // An attached Daemon's first stdin line is the App's proof (H18); the rest of stdin is its lifetime.
    let proof = if attached {
        Some(attached_proof().await)
    } else {
        None
    };
    let daemon = rupd::Daemon::open_with_proof(&project.join(".roundup"), proof)
        .map_err(io::Error::other)?;

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
    // The App holds our stdin open; its exit, even a crash, closes it. Its bytes are drained and
    // discarded; only end of file matters.
    let stdin_closed =
        tokio::task::spawn_blocking(|| io::copy(&mut io::stdin().lock(), &mut io::sink()));
    let ended = tokio::select! {
        served = rupd::serve(listener, Arc::clone(&daemon)) => served,
        closed = stdin_closed => closed.map_err(io::Error::other)?.map(drop),
    };
    // An App that crashed has closed stderr too; a failed log write must not skip the stop.
    let _ = writeln!(io::stderr(), "rupd: stopping");
    let stopped = daemon.stop_terminals().await.map_err(io::Error::other);
    ended.and(stopped)
}

/// Never returns a bad handshake: the diagnostic names the fault, never the bytes, and the process
/// exits at once because a blocked stdin read cannot be cancelled.
async fn attached_proof() -> String {
    let read = tokio::task::spawn_blocking(|| rupd::read_handshake(&mut io::stdin().lock()));
    let failure = match tokio::time::timeout(rupd::HANDSHAKE_BOUND, read).await {
        Ok(Ok(Ok(handshake))) => return handshake.into_proof(),
        Ok(Ok(Err(err))) => err.to_string(),
        Ok(Err(err)) => err.to_string(),
        Err(_) => format!("no handshake within {:?}", rupd::HANDSHAKE_BOUND),
    };
    let _ = writeln!(io::stderr(), "rupd: refusing to start: {failure}");
    std::process::exit(1)
}
