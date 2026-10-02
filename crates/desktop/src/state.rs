use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use contracts::Event;
use rpc::{Client, RpcError, code};
use serde::Serialize;
use serde_json::Value;
use tauri::async_runtime::JoinHandle;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::process::ChildStdin;
use tokio::time::timeout;

use crate::config::Config;
use crate::daemon::{self, OwnedSocket};

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
    /// This run's socket path (S5): `subscribe` reconnects here, not through `Config`, since a
    /// reopen's Daemon runs on its own path.
    socket: OwnedSocket,
    /// One at a time: a reloaded webview subscribes again and must not receive every Event twice.
    subscription: Option<JoinHandle<()>>,
}

impl Drop for Open {
    fn drop(&mut self) {
        if let Some(subscription) = self.subscription.take() {
            subscription.abort();
        }
    }
}

enum Phase {
    Closed,
    /// The Project reopened, if any (S5): what to fall back to if the Daemon fails to start.
    Opening(Option<Project>),
    Open(Open),
    Exited(Project),
}

/// Which Project is open and the Daemon serving it; the only state the App holds.
pub struct AppState {
    config: Config,
    phase: Mutex<Phase>,
    /// Counts Daemon starts so each gets its own socket path (S5): a reopen must never land on
    /// the exited run's path.
    runs: Mutex<u64>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            phase: Mutex::new(Phase::Closed),
            runs: Mutex::new(0),
        }
    }

    pub fn project(&self) -> Option<Project> {
        match &*self.phase() {
            Phase::Open(open) => Some(open.project.clone()),
            Phase::Exited(project) => Some(project.clone()),
            Phase::Closed | Phase::Opening(_) => None,
        }
    }

    pub async fn open_project<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        path: &Path,
    ) -> Result<Project, RpcError> {
        let run = self.begin_opening(path)?;
        match self.start_daemon(app, path, run).await {
            Ok(project) => Ok(project),
            Err(err) => {
                self.fail_opening();
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
        let socket = self.socket()?;
        let mut events = Client::connect(&socket).await.map_err(RpcError::internal)?;
        self.bounded(
            "events.subscribe",
            events.request("events.subscribe", Value::Null),
        )
        .await?;
        let pump = tauri::async_runtime::spawn(async move {
            while let Some(event) = events.next_event().await {
                if channel.send(event).is_err() {
                    return;
                }
            }
        });
        match &mut *self.phase() {
            Phase::Open(open) => {
                if let Some(previous) = open.subscription.replace(pump) {
                    previous.abort();
                }
                Ok(())
            }
            Phase::Closed | Phase::Opening(_) | Phase::Exited(_) => {
                pump.abort();
                Err(RpcError::internal("the Daemon went away while subscribing"))
            }
        }
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

    /// A Project whose Daemon exited may reopen on the same path (S5); any other path, or any
    /// path while a Daemon runs or is starting, is `CONFLICT` because the App holds one Project.
    /// Returns this open's run number, so each Daemon gets its own socket path (S5).
    fn begin_opening(&self, path: &Path) -> Result<u64, RpcError> {
        let mut phase = self.phase();
        match &*phase {
            Phase::Closed => {
                *phase = Phase::Opening(None);
            }
            Phase::Exited(project) if project.path == path.display().to_string() => {
                let project = project.clone();
                *phase = Phase::Opening(Some(project));
            }
            Phase::Opening(_) | Phase::Open(_) | Phase::Exited(_) => {
                return Err(RpcError::conflict("a Project is already open"));
            }
        }
        drop(phase);
        let mut runs = self
            .runs
            .lock()
            .expect("the runs lock is never held across a panic");
        let run = *runs;
        *runs += 1;
        Ok(run)
    }

    /// Reverts a failed open: back to `Closed` for a first open, back to `Exited` with the same
    /// Project for a failed reopen, so the App still holds that one Project (S5) and is ready to
    /// try again.
    fn fail_opening(&self) {
        let mut phase = self.phase();
        let Phase::Opening(reopened) = &mut *phase else {
            return;
        };
        *phase = reopened.take().map_or(Phase::Closed, Phase::Exited);
    }

    async fn start_daemon<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        path: &Path,
        run: u64,
    ) -> Result<Project, RpcError> {
        if !path.is_absolute() || !path.is_dir() {
            return Err(RpcError::new(
                code::INVALID_PARAMS,
                format!("not an absolute path to a directory: {}", path.display()),
            ));
        }
        let project = Project {
            name: named_after(path),
            path: path.display().to_string(),
        };
        let socket = self.config.socket_for_run(run);
        let started = daemon::start(&self.config, path, &socket).await?;
        *self.phase() = Phase::Open(Open {
            project: project.clone(),
            client: Arc::new(started.client),
            _stdin: started.stdin,
            socket: started.socket,
            subscription: None,
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
        self.open_ref(|open| Arc::clone(&open.client))
    }

    fn socket(&self) -> Result<PathBuf, RpcError> {
        self.open_ref(|open| open.socket.path().to_path_buf())
    }

    /// The guard every read of the open Daemon's state shares: `INTERNAL` once it has exited,
    /// `CONFLICT` before any Project is open or while one is still opening.
    fn open_ref<T>(&self, read: impl FnOnce(&Open) -> T) -> Result<T, RpcError> {
        match &*self.phase() {
            Phase::Open(open) => Ok(read(open)),
            Phase::Exited(_) => Err(RpcError::internal("the Daemon has exited")),
            Phase::Closed | Phase::Opening(_) => Err(RpcError::conflict("no Project is open")),
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

/// Resolves `.` and `..` first: the last component of `/a/b/..` is not the Project's name.
fn named_after(path: &Path) -> String {
    let reached = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    reached.file_name().map_or_else(
        || reached.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}
