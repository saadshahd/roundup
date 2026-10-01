//! Terminals: plain shell processes behind a PTY. Owner: terminal Builder.
//!
//! Rust callers (the agents module) use [`Terminals`] directly, with no socket: [`Terminals::spawn`]
//! returns the new Terminal's id together with a [`broadcast::Receiver`] of its events, subscribed
//! before the program starts so no output is missed.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use contracts::terminal::{
    ExitedEvent, OutputEvent, ResizeParams, SpawnParams, TerminalId, TerminalInfo, TitleEvent,
    WriteParams,
};
use contracts::{Actor, EventData};
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
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

/// What it takes to drive a running program. Dropped when the program exits.
struct Handle {
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
}

struct Entry {
    info: TerminalInfo,
    events: broadcast::Sender<EventData>,
    handle: Option<Arc<Handle>>,
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

    fn retitle(&self, number: u64, title: String) {
        let id = number.to_string();
        if let Some(entry) = self.table().get_mut(&number) {
            entry.info.title = Some(title.clone());
        }
        self.publish(number, EventData::TerminalTitle(TitleEvent { id, title }));
    }

    fn finish(&self, number: u64, code: Option<i32>) {
        let id = number.to_string();
        if let Some(entry) = self.table().get_mut(&number) {
            entry.info.running = false;
            entry.handle = None;
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
    /// A program that cannot start (missing cwd or command) is the caller's error.
    pub async fn spawn(&self, params: SpawnParams) -> Result<Spawned, RpcError> {
        if params.command.as_ref().is_some_and(Vec::is_empty) {
            return Err(invalid("command must not be empty"));
        }
        // The forked child cannot report a bad cwd, so it would exit silently; check here.
        if !Path::new(&params.cwd).is_dir() {
            return Err(invalid(format!("cwd is not a directory: {}", params.cwd)));
        }
        let size = (params.rows, params.cols);
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
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|err| invalid(format!("cannot start program: {err}")))?;
        // Only the program may hold the slave end, or reading never sees it close.
        drop(pair.slave);
        let reader = pair.master.try_clone_reader().map_err(RpcError::internal)?;
        let handle = Handle {
            writer: Mutex::new(pair.master.take_writer().map_err(RpcError::internal)?),
            killer: Mutex::new(child.clone_killer()),
            master: Mutex::new(pair.master),
        };

        let number = self.next.fetch_add(1, Ordering::Relaxed);
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
                handle: Some(Arc::new(handle)),
            },
        );
        let shared = Arc::clone(&self.shared);
        std::thread::Builder::new()
            .name(format!("terminal-{number}"))
            .spawn(move || pump(&shared, number, reader, child, size))
            .map_err(RpcError::internal)?;
        Ok(Spawned {
            id: number.to_string(),
            events,
        })
    }

    /// Type `bytes` into a running Terminal. `NOT_FOUND` if it is unknown or has exited.
    pub async fn write(&self, id: &str, bytes: &[u8]) -> Result<(), RpcError> {
        let handle = self.running(id)?;
        let bytes = bytes.to_vec();
        // A program that stops reading fills the PTY buffer and blocks the write.
        tokio::task::spawn_blocking(move || {
            let mut writer = handle.writer.lock().expect("terminal writer lock");
            writer.write_all(&bytes).and_then(|()| writer.flush())
        })
        .await
        .map_err(RpcError::internal)?
        .map_err(RpcError::internal)
    }

    /// Tell a running Terminal its window is now `cols` x `rows`. `NOT_FOUND` as for [`Terminals::write`].
    pub async fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), RpcError> {
        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };
        let handle = self.running(id)?;
        let master = handle.master.lock().expect("terminal master lock");
        master.resize(size).map_err(RpcError::internal)
    }

    /// Stop a running Terminal's program. It stays listed as exited once `terminal.exited` is emitted.
    /// `NOT_FOUND` as for [`Terminals::write`].
    pub async fn kill(&self, id: &str) -> Result<(), RpcError> {
        let handle = self.running(id)?;
        let mut killer = handle.killer.lock().expect("terminal killer lock");
        killer.kill().map_err(RpcError::internal)
    }

    fn running(&self, id: &str) -> Result<Arc<Handle>, RpcError> {
        id.parse::<u64>()
            .ok()
            .and_then(|number| self.shared.table().get(&number)?.handle.clone())
            .ok_or_else(|| RpcError::not_found(format!("running terminal {id}")))
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

/// Collects the window titles (OSC 0 and 2) the parser sees; vt100 stitches sequences split across reads.
#[derive(Default)]
struct Titles(Vec<String>);

impl vt100::Callbacks for Titles {
    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        self.0.push(String::from_utf8_lossy(title).into_owned());
    }
}

/// Read until the program closes the PTY, publishing each chunk, then report how it ended.
fn pump(
    shared: &Shared,
    number: u64,
    mut reader: Box<dyn Read + Send>,
    mut child: Box<dyn portable_pty::Child + Send + Sync>,
    (rows, cols): (u16, u16),
) {
    let id = number.to_string();
    let mut parser = vt100::Parser::new_with_callbacks(rows, cols, 0, Titles::default());
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
        parser.process(&chunk[..read]);
        for title in std::mem::take(&mut parser.callbacks_mut().0) {
            shared.retitle(number, title);
        }
    }
    let code = child
        .wait()
        .ok()
        .filter(|status| status.signal().is_none())
        .map(|status| i32::try_from(status.exit_code()).unwrap_or(i32::MAX));
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
            "terminal.write" => {
                let WriteParams { id, data } = params(value)?;
                let bytes = STANDARD
                    .decode(data)
                    .map_err(|err| invalid(format!("data is not base64: {err}")))?;
                self.write(&id, &bytes).await.and(reply(&()))
            }
            "terminal.resize" => {
                let ResizeParams { id, cols, rows } = params(value)?;
                self.resize(&id, cols, rows).await.and(reply(&()))
            }
            "terminal.kill" => {
                let TerminalId { id } = params(value)?;
                self.kill(&id).await.and(reply(&()))
            }
            "terminal.list" => reply(&self.list()),
            _ => Err(RpcError::method_not_found(method)),
        }
    }
}
