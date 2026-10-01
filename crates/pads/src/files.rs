//! File-backed storage: each Pad is `<dir>/pads/<name>.md`. Names are validated before they get here.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use rpc::RpcError;
use tempfile::NamedTempFile;

/// Atomic: a temp file in the same directory, then a rename.
pub fn write(dir: &Path, name: &str, text: &str) -> Result<(), RpcError> {
    let write = || -> io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let mut temp = NamedTempFile::new_in(dir)?;
        temp.write_all(text.as_bytes())?;
        temp.persist(path(dir, name)).map_err(|err| err.error)?;
        Ok(())
    };
    write().map_err(|err| refused(err, name))
}

/// A missing file is already removed.
pub fn remove(dir: &Path, name: &str) -> Result<(), RpcError> {
    match std::fs::remove_file(path(dir, name)) {
        Err(err) if err.kind() != io::ErrorKind::NotFound => Err(refused(err, name)),
        _ => Ok(()),
    }
}

/// `None` when the file does not exist.
pub fn read(dir: &Path, name: &str) -> Result<Option<String>, RpcError> {
    match std::fs::read_to_string(path(dir, name)) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(refused(err, name)),
    }
}

fn path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.md"))
}

/// A name the file system refuses is the caller's mistake; anything else is ours.
fn refused(err: io::Error, name: &str) -> RpcError {
    let code = match err.kind() {
        io::ErrorKind::InvalidFilename => rpc::code::INVALID_PARAMS,
        _ => rpc::code::INTERNAL,
    };
    RpcError::new(code, format!("pad file {name}.md: {err}"))
}
