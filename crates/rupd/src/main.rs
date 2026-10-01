use std::io;

use tokio::net::UnixListener;

#[tokio::main]
async fn main() -> io::Result<()> {
    let path = rpc::socket_path()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // A stale socket from a crashed run would make bind fail; a live one is caught by connect.
    if path.exists() && tokio::net::UnixStream::connect(&path).await.is_err() {
        std::fs::remove_file(&path)?;
    }
    let listener = UnixListener::bind(&path)?;
    eprintln!("rupd: listening on {}", path.display());
    rupd::serve(listener).await
}
