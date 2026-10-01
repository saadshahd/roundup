//! The App: starts a Daemon for one Project and passes calls and events to the webview (`scenarios/app.md`).

mod config;
mod shell;

use std::path::PathBuf;

pub use config::{CALL_BOUND, Config, READY_BOUND};
pub use shell::{Project, Shell};
use tauri::{AppHandle, Builder, RunEvent, Runtime};

pub fn build<R: Runtime>(
    _builder: Builder<R>,
    _config: Config,
    _folder: Option<PathBuf>,
) -> Builder<R> {
    todo!()
}

pub fn handle_run_event<R: Runtime>(_app: &AppHandle<R>, _event: &RunEvent) {
    todo!()
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    todo!()
}
