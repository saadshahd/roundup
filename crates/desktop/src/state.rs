use std::future::Future;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use contracts::Event;
use rpc::{Client, RpcError, code};
use serde::Serialize;
use serde_json::Value;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::process::ChildStdin;
use tokio::time::timeout;

use crate::config::Config;
use crate::daemon;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Project {
    pub name: String,
    pub path: String,
}

#[derive(Clone, Serialize)]
struct DaemonExited {
    code: Option<i32>,
}

struct Open {
    project: Project,
    client: Arc<Client>,
    /// Closing it is how the Daemon learns the App is gone (D1).
    _stdin: ChildStdin,
}

enum Phase {
    Closed,
    Opening,
    Open(Open),
    Exited(Project),
}

/// Which Project is open and the Daemon serving it; the only state the App holds.
pub struct AppState {
    config: Config,
    phase: Mutex<Phase>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            phase: Mutex::new(Phase::Closed),
        }
    }

    pub fn project(&self) -> Option<Project> {
        match &*self.phase() {
            Phase::Open(open) => Some(open.project.clone()),
            Phase::Exited(project) => Some(project.clone()),
            Phase::Closed | Phase::Opening => None,
        }
    }

    pub async fn open_project<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        path: &Path,
    ) -> Result<Project, RpcError> {
        self.begin_opening()?;
        match self.start_daemon(app, path).await {
            Ok(project) => Ok(project),
            Err(err) => {
                *self.phase() = Phase::Closed;
                Err(err)
            }
        }
    }

    pub async fn rpc(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let client = self.client()?;
        self.bounded(method, client.request(method, params)).await
    }

    /// Sends every Daemon Event to `channel`, in the Daemon's order, until the Daemon or the webview goes away.
    pub async fn subscribe(&self, channel: Channel<Event>) -> Result<(), RpcError> {
        self.client()?;
        let mut events = Client::connect(&self.config.socket)
            .await
            .map_err(RpcError::internal)?;
        self.bounded(
            "events.subscribe",
            events.request("events.subscribe", Value::Null),
        )
        .await?;
        tauri::async_runtime::spawn(async move {
            while let Some(event) = events.next_event().await {
                if channel.send(event).is_err() {
                    return;
                }
            }
        });
        Ok(())
    }

    /// Closes the Daemon's stdin; a Daemon started with `--attached` then stops its Agents and Terminals and exits.
    pub fn close(&self) {
        *self.phase() = Phase::Closed;
    }

    fn phase(&self) -> MutexGuard<'_, Phase> {
        self.phase
            .lock()
            .expect("the phase lock is never held across a panic")
    }

    fn begin_opening(&self) -> Result<(), RpcError> {
        let mut phase = self.phase();
        match *phase {
            Phase::Closed => {
                *phase = Phase::Opening;
                Ok(())
            }
            Phase::Opening | Phase::Open(_) | Phase::Exited(_) => {
                Err(RpcError::conflict("a Project is already open"))
            }
        }
    }

    async fn start_daemon<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        path: &Path,
    ) -> Result<Project, RpcError> {
        if !path.is_absolute() || !path.is_dir() {
            return Err(RpcError::new(
                code::INVALID_PARAMS,
                format!("not an absolute path to a directory: {}", path.display()),
            ));
        }
        let project = Project {
            name: path.file_name().map_or_else(
                || path.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            ),
            path: path.display().to_string(),
        };
        let started = daemon::start(&self.config, path).await?;
        *self.phase() = Phase::Open(Open {
            project: project.clone(),
            client: Arc::new(started.client),
            _stdin: started.stdin,
        });
        let app = app.clone();
        daemon::watch_exit(started.child, move |code| {
            if app.state::<AppState>().mark_exited()
                && let Err(err) = app.emit("daemon-exited", DaemonExited { code })
            {
                eprintln!("desktop: could not tell the webview the Daemon exited: {err}");
            }
        });
        Ok(project)
    }

    /// False when the App closed the Daemon itself, so the webview is not told about an exit it asked for.
    fn mark_exited(&self) -> bool {
        let mut phase = self.phase();
        let Phase::Open(open) = &*phase else {
            return false;
        };
        *phase = Phase::Exited(open.project.clone());
        true
    }

    fn client(&self) -> Result<Arc<Client>, RpcError> {
        match &*self.phase() {
            Phase::Open(open) => Ok(Arc::clone(&open.client)),
            Phase::Exited(_) => Err(RpcError::internal("the Daemon has exited")),
            Phase::Closed | Phase::Opening => Err(RpcError::conflict("no Project is open")),
        }
    }

    async fn bounded<T>(
        &self,
        method: &str,
        call: impl Future<Output = Result<T, RpcError>>,
    ) -> Result<T, RpcError> {
        timeout(self.config.call_bound, call).await.map_err(|_| {
            RpcError::internal(format!(
                "no reply to {method} within {:?}; whether the Daemon applied it is unknown",
                self.config.call_bound
            ))
        })?
    }
}
