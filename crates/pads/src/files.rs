//! File-backed storage: each Pad is `<dir>/pads/<name>.md`. Names are validated before they get here.

use std::io;
use std::path::{Path, PathBuf};

pub fn write(dir: &Path, name: &str, text: &str) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(path(dir, name), text)
}

/// A missing file is already removed.
pub fn remove(dir: &Path, name: &str) -> io::Result<()> {
    match std::fs::remove_file(path(dir, name)) {
        Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
        _ => Ok(()),
    }
}

/// `None` when the file does not exist.
pub fn read(dir: &Path, name: &str) -> io::Result<Option<String>> {
    match std::fs::read_to_string(path(dir, name)) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

fn path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.md"))
}
