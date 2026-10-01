//! Terminals: plain shell processes behind a PTY. Owner: terminal Builder.
//!
//! Rust callers (the agents module) use [`Terminals`] directly, with no socket: [`Terminals::spawn`]
//! returns the new Terminal's id together with a [`broadcast::Receiver`] of its events. The receiver
//! exists before the reader thread reads any output, so none is missed.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};

use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use contracts::terminal::{ExitedEvent, OutputEvent, SpawnParams, TerminalId, TerminalInfo};
use contracts::{Actor, EventData};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use rpc::{Bus, Ctx, Module, OpenError, RpcError, code, params, reply};
use serde_json::Value;
use tokio::sync::broadcast;

/// Events a slow subscriber may fall behind by before it is told it lagged (it never blocks the reader).
const EVENT_BACKLOG: usize = 1024;
const READ_CHUNK: usize = 8192;

/// A freshly spawned Terminal: its id and every event it emits, from the first byte of output.
pub struct Spawned {
    pub id: String,
    pub events: broadcast::Receiver<EventData>,
}

struct Entry {
    info: TerminalInfo,
    events: broadcast::Sender<EventData>,
}

/// State shared with each Terminal's reader thread.
struct Shared {
    bus: Bus,
    table: Mutex<BTreeMap<u64, Entry>>,
}

impl Shared {
    fn table(&self) -> MutexGuard<'_, BTreeMap<u64, Entry>> {
        self.table.lock().expect("terminal table lock")
    }

    /// Hand `data` to this Terminal's subscribers and to the Bus. Neither can block the caller.
    fn publish(&self, number: u64, data: EventData) {
        if let Some(entry) = self.table().get(&number) {
            let _ = entry.events.send(data.clone());
        }
        self.bus.emit(Actor::daemon(), data);
    }

    fn finish(&self, number: u64, code: Option<i32>) {
        let id = number.to_string();
        if let Some(entry) = self.table().get_mut(&number) {
            entry.info.running = false;
            entry.info.exit_code = code;
        }
        self.publish(number, EventData::TerminalExited(ExitedEvent { id, code }));
    }
}

pub struct Terminals {
    shared: Arc<Shared>,
    next: AtomicU64,
}

impl Terminals {
    /// `dir` is the Project's `.roundup/` directory. `bus` is for events no call caused.
    pub fn open(_dir: &Path, bus: Bus) -> Result<Self, OpenError> {
        Ok(Self {
            shared: Arc::new(Shared {
                bus,
                table: Mutex::new(BTreeMap::new()),
            }),
            next: AtomicU64::new(1),
        })
    }

    /// Start `params.command` (or the login shell) in a new PTY.
    /// A missing cwd or an empty command is the caller's error; a failure to start is the Daemon's.
    pub async fn spawn(&self, params: SpawnParams) -> Result<Spawned, RpcError> {
        if params.command.as_ref().is_some_and(Vec::is_empty) {
            return Err(invalid("command must not be empty"));
        }
        // portable-pty silently falls back to $HOME when the cwd is unusable; refuse instead.
        if !Path::new(&params.cwd).is_dir() {
            return Err(invalid(format!("cwd is not a directory: {}", params.cwd)));
        }
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: params.rows,
                cols: params.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(RpcError::internal)?;
        let mut command = match &params.command {
            Some(argv) => CommandBuilder::from_argv(argv.iter().map(Into::into).collect()),
            None => CommandBuilder::new_default_prog(),
        };
        command.cwd(&params.cwd);
        for (name, value) in &params.env {
            command.env(name, value);
        }
        // The reader thread starts before the program and receives the child over a channel, so
        // no later failure can leave a program that nobody waits for.
        let reader = pair.master.try_clone_reader().map_err(RpcError::internal)?;
        let number = self.next.fetch_add(1, Ordering::Relaxed);
        let (arrival, child_arrives) = mpsc::channel();
        let shared = Arc::clone(&self.shared);
        std::thread::Builder::new()
            .name(format!("terminal-{number}"))
            .spawn(move || {
                // No child means the program never started: nothing to report.
                if let Ok(child) = child_arrives.recv() {
                    pump(&shared, number, reader, child, pair.master);
                }
            })
            .map_err(RpcError::internal)?;
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(RpcError::internal)?;
        // Only the program may hold the slave end, or reading never sees it close.
        drop(pair.slave);

        let (sender, events) = broadcast::channel(EVENT_BACKLOG);
        self.shared.table().insert(
            number,
            Entry {
                info: TerminalInfo {
                    id: number.to_string(),
                    cwd: params.cwd,
                    title: None,
                    running: true,
                    exit_code: None,
                },
                events: sender,
            },
        );
        arrival
            .send(child)
            .expect("the reader thread waits for its child");
        Ok(Spawned {
            id: number.to_string(),
            events,
        })
    }

    /// Every Terminal, running or exited, oldest first.
    pub fn list(&self) -> Vec<TerminalInfo> {
        self.shared
            .table()
            .values()
            .map(|entry| entry.info.clone())
            .collect()
    }
}

/// Read until the program closes the PTY, publishing each chunk, then report how it ended.
/// Owns `master` so the PTY outlives the program's last write.
fn pump(
    shared: &Shared,
    number: u64,
    mut reader: Box<dyn Read + Send>,
    mut child: Box<dyn portable_pty::Child + Send + Sync>,
    master: Box<dyn portable_pty::MasterPty + Send>,
) {
    let id = number.to_string();
    let mut chunk = [0u8; READ_CHUNK];
    // Linux reports the closed PTY as an error, macOS as end of file: both mean "no more output".
    while let Ok(read) = reader.read(&mut chunk) {
        if read == 0 {
            break;
        }
        let data = STANDARD.encode(&chunk[..read]);
        shared.publish(
            number,
            EventData::TerminalOutput(OutputEvent {
                id: id.clone(),
                data,
            }),
        );
    }
    let code = match child.wait() {
        Ok(status) if status.signal().is_some() => None,
        Ok(status) => Some(i32::try_from(status.exit_code()).unwrap_or(i32::MAX)),
        Err(err) => {
            // `None` means "killed by a signal"; an unknown ending must not look like one.
            eprintln!("terminal {number}: cannot wait for program: {err}");
            Some(-1)
        }
    };
    drop(master);
    shared.finish(number, code);
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(code::INVALID_PARAMS, message)
}

#[async_trait]
impl Module for Terminals {
    fn namespaces(&self) -> &'static [&'static str] {
        &["terminal"]
    }

    async fn call(&self, _ctx: &Ctx, method: &str, value: Value) -> Result<Value, RpcError> {
        match method {
            "terminal.spawn" => {
                let spawned = self.spawn(params(value)?).await?;
                reply(&TerminalId { id: spawned.id })
            }
            "terminal.list" => reply(&self.list()),
            _ => Err(RpcError::method_not_found(method)),
        }
    }
}
