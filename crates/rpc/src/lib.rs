//! Shared by the Daemon, its modules and its clients: errors, the module contract, and the client.

mod client;
mod error;
mod module;

use std::io;
use std::path::PathBuf;

pub use client::Client;
pub use error::{RpcError, code};
pub use module::{Bus, Ctx, Module, params, reply};

/// Why a module could not start (a bad database, an unwritable directory).
pub type OpenError = Box<dyn std::error::Error + Send + Sync>;

/// `RUPD_SOCKET` if set, else `~/.roundup/rupd.sock`.
pub fn socket_path() -> io::Result<PathBuf> {
    if let Some(path) = std::env::var_os("RUPD_SOCKET") {
        return Ok(path.into());
    }
    let home = std::env::var_os("HOME")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
    Ok(PathBuf::from(home).join(".roundup/rupd.sock"))
}
