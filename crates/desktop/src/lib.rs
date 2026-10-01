//! The App: starts a Daemon for one Project and passes calls and events to the webview (`scenarios/app.md`).

mod config;
mod daemon;
mod state;

use std::path::{Path, PathBuf};

pub use config::{CALL_BOUND, Config, READY_BOUND};
use contracts::Event;
use rpc::RpcError;
use serde_json::Value;
pub use state::{AppState, Project};
use tauri::ipc::Channel;
use tauri::{AppHandle, Builder, Manager, RunEvent, Runtime, State};

pub fn build<R: Runtime>(
    builder: Builder<R>,
    config: Config,
    project_dir: Option<PathBuf>,
) -> Builder<R> {
    builder
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new(config))
        .invoke_handler(tauri::generate_handler![
            project,
            open_project,
            rpc,
            subscribe
        ])
        .setup(move |app| {
            if let Some(project_dir) = project_dir {
                let state = app.state::<AppState>();
                tauri::async_runtime::block_on(state.open_project(app.handle(), &project_dir))?;
            }
            Ok(())
        })
}

pub fn handle_run_event<R: Runtime>(app: &AppHandle<R>, event: &RunEvent) {
    if matches!(event, RunEvent::Exit) {
        app.state::<AppState>().close();
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let project_dir = std::env::args_os()
        .nth(1)
        .map(std::path::absolute)
        .transpose()?;
    build(tauri::Builder::default(), config, project_dir)
        .build(tauri::generate_context!())?
        .run(|app, event| handle_run_event(app, &event));
    Ok(())
}

#[tauri::command]
fn project(state: State<'_, AppState>) -> Option<Project> {
    state.project()
}

#[tauri::command]
async fn open_project<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    path: String,
) -> Result<Project, RpcError> {
    state.open_project(&app, Path::new(&path)).await
}

#[tauri::command]
async fn rpc(state: State<'_, AppState>, method: String, params: Value) -> Result<Value, RpcError> {
    state.rpc(&method, params).await
}

#[tauri::command]
async fn subscribe(state: State<'_, AppState>, channel: Channel<Event>) -> Result<(), RpcError> {
    state.subscribe(channel).await
}
