//! The App: starts a Daemon for one Project and passes calls and events to the webview (`scenarios/app.md`).

mod config;
mod daemon;
mod shell;

use std::path::{Path, PathBuf};

pub use config::{CALL_BOUND, Config, READY_BOUND};
use contracts::Event;
use rpc::RpcError;
use serde_json::Value;
pub use shell::{Project, Shell};
use tauri::ipc::Channel;
use tauri::{AppHandle, Builder, Manager, RunEvent, Runtime, State};

pub fn build<R: Runtime>(
    builder: Builder<R>,
    config: Config,
    folder: Option<PathBuf>,
) -> Builder<R> {
    builder
        .plugin(tauri_plugin_dialog::init())
        .manage(Shell::new(config))
        .invoke_handler(tauri::generate_handler![
            project,
            open_project,
            rpc,
            subscribe
        ])
        .setup(move |app| {
            if let Some(folder) = folder {
                let shell = app.state::<Shell>();
                tauri::async_runtime::block_on(shell.open_project(app.handle(), &folder))?;
            }
            Ok(())
        })
}

pub fn handle_run_event<R: Runtime>(app: &AppHandle<R>, event: &RunEvent) {
    if matches!(event, RunEvent::Exit) {
        app.state::<Shell>().close();
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let folder = std::env::args_os()
        .nth(1)
        .map(std::path::absolute)
        .transpose()?;
    build(tauri::Builder::default(), config, folder)
        .build(tauri::generate_context!())?
        .run(|app, event| handle_run_event(app, &event));
    Ok(())
}

#[tauri::command]
fn project(shell: State<'_, Shell>) -> Option<Project> {
    shell.project()
}

#[tauri::command]
async fn open_project<R: Runtime>(
    app: AppHandle<R>,
    shell: State<'_, Shell>,
    path: String,
) -> Result<Project, RpcError> {
    shell.open_project(&app, Path::new(&path)).await
}

#[tauri::command]
async fn rpc(shell: State<'_, Shell>, method: String, params: Value) -> Result<Value, RpcError> {
    shell.rpc(&method, params).await
}

#[tauri::command]
async fn subscribe(shell: State<'_, Shell>, channel: Channel<Event>) -> Result<(), RpcError> {
    shell.subscribe(channel).await
}
