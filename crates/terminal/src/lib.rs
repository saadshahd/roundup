//! Terminals: plain shell processes behind a PTY. Owner: terminal Builder.
//!
//! Rust callers (the agents module) use [`Terminals`] directly, with no socket: [`Terminals::spawn`]
//! returns the new Terminal's id together with a [`broadcast::Receiver`] of its events. The receiver
//! exists before the reader thread reads any output, so none is missed.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use contracts::terminal::{
    ExitedEvent, OutputEvent, ResizeParams, SpawnParams, TerminalId, TerminalInfo, TitleEvent,
    WriteParams,
};
use contracts::{Actor, EventData};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use rpc::{Bus, Ctx, Module, OpenError, RpcError, code, params, reply};
use serde_json::Value;
use tokio::sync::broadcast;

/// Events a slow subscriber may fall behind by before it is told it lagged (it never blocks the reader).
const EVENT_BACKLOG: usize = 1024;
/// The parser only listens for titles, so its screen is small and fixed however big the window is.
const TITLE_ROWS: u16 = 24;
const TITLE_COLS: u16 = 80;
/// How often the reader thread checks whether a program that closed its PTY has ended.
const REAP_POLL: Duration = Duration::from_millis(5);
const READ_CHUNK: usize = 8192;
/// Writes a Terminal accepts ahead of its program reading them. Past this, `write` is `CONFLICT`:
/// a program that never reads would otherwise hold a thread per write until the blocking pool is
/// exhausted and `kill` cannot run.
const WRITE_BACKLOG: usize = 16;

/// A freshly spawned Terminal: its id and every event it emits, from the first byte of output.
pub struct Spawned {
    pub id: String,
    pub events: broadcast::Receiver<EventData>,
}

/// The program, shared by whoever reaps it and whoever stops it.
/// Invariant: only code holding this lock reaps or signals, and signals only when `try_wait` under
/// that lock has just said the program is still running. A reaped pid is never signalled.
type Program = Arc<Mutex<Box<dyn Child + Send + Sync>>>;

/// What it takes to drive a running program. Dropped from the Terminal's entry once it is reaped.
struct Handle {
    master: Mutex<Box<dyn MasterPty + Send>>,
    input: SyncSender<Vec<u8>>,
    program: Program,
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
    /// A missing cwd, an empty command, or a program that cannot be found or run is the caller's error;
    /// any other failure to start is the Daemon's.
    pub async fn spawn(&self, params: SpawnParams) -> Result<Spawned, RpcError> {
        check_program(&params)?;
        // portable-pty silently falls back to $HOME when the cwd is unusable; refuse instead.
        if !Path::new(&params.cwd).is_dir() {
            return Err(invalid(format!("cwd is not a directory: {}", params.cwd)));
        }
        let pair = native_pty_system()
            .openpty(window(params.cols, params.rows)?)
            .map_err(RpcError::internal)?;
        let mut command = match &params.command {
            Some(argv) => CommandBuilder::from_argv(argv.iter().map(Into::into).collect()),
            None => CommandBuilder::new_default_prog(),
        };
        command.cwd(&params.cwd);
        for (name, value) in &params.env {
            command.env(name, value);
        }
        // The reader and input threads start before the program (the reader receives the child over a
        // channel), so no later failure can leave a program that nobody waits for.
        let reader = pair.master.try_clone_reader().map_err(RpcError::internal)?;
        let writer = pair.master.take_writer().map_err(RpcError::internal)?;
        let number = self.next.fetch_add(1, Ordering::Relaxed);
        let (arrival, child_arrives) = mpsc::channel();
        let shared = Arc::clone(&self.shared);
        std::thread::Builder::new()
            .name(format!("terminal-{number}"))
            .spawn(move || {
                // No child means the program never started: nothing to report.
                if let Ok(program) = child_arrives.recv() {
                    pump(&shared, number, reader, &program);
                }
            })
            .map_err(RpcError::internal)?;
        let (input, queued) = mpsc::sync_channel(WRITE_BACKLOG);
        std::thread::Builder::new()
            .name(format!("terminal-{number}-input"))
            .spawn(move || type_queued(number, queued, writer))
            .map_err(RpcError::internal)?;
        let argv0 = params
            .command
            .as_deref()
            .and_then(<[String]>::first)
            .map(String::as_str);
        let program: Program = Arc::new(Mutex::new(
            pair.slave
                .spawn_command(command)
                .map_err(|err| spawn_error(argv0, &err))?,
        ));
        // Only the program may hold the slave end, or reading never sees it close.
        drop(pair.slave);
        let handle = Handle {
            input,
            program: Arc::clone(&program),
            master: Mutex::new(pair.master),
        };

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
        arrival
            .send(program)
            .expect("the reader thread waits for its child");
        Ok(Spawned {
            id: number.to_string(),
            events,
        })
    }

    /// Queue `bytes` to be typed into a running Terminal and return without waiting for the program to
    /// read them. `NOT_FOUND` if it is unknown or has exited; `CONFLICT` if the program has left
    /// [`WRITE_BACKLOG`] writes unread.
    pub async fn write(&self, id: &str, bytes: &[u8]) -> Result<(), RpcError> {
        match self.running(id)?.input.try_send(bytes.to_vec()) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(RpcError::conflict(format!(
                "terminal {id} is not reading its input"
            ))),
            Err(TrySendError::Disconnected(_)) => Err(RpcError::internal(format!(
                "terminal {id} can no longer take input"
            ))),
        }
    }

    /// Tell a running Terminal its window is now `cols` x `rows`. `NOT_FOUND` as for [`Terminals::write`].
    pub async fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), RpcError> {
        let size = window(cols, rows)?;
        let handle = self.running(id)?;
        let master = handle.master.lock().expect("terminal master lock");
        master.resize(size).map_err(RpcError::internal)
    }

    /// Stop a running Terminal's program (SIGHUP, then SIGKILL if it lingers) and return once it is
    /// stopped. It stays listed as exited once `terminal.exited` is emitted. `NOT_FOUND` as for
    /// [`Terminals::write`], including a program that ended just before this call.
    pub async fn kill(&self, id: &str) -> Result<(), RpcError> {
        let handle = self.running(id)?;
        // portable-pty's `Child::kill` waits out the SIGHUP grace period, so keep it off the runtime.
        tokio::task::spawn_blocking(move || {
            let mut program = handle.program.lock().expect("terminal program lock");
            match program.try_wait() {
                Ok(None) => program.kill().map_err(RpcError::internal)?,
                Ok(Some(_)) => return Err(RpcError::not_found("running terminal")),
                Err(err) => return Err(RpcError::internal(err)),
            }
            drop(program);
            // portable-pty returns right after SIGKILL; a second kill must already see the program gone.
            reap(&handle.program).map(drop).map_err(RpcError::internal)
        })
        .await
        .map_err(RpcError::internal)?
    }

    fn running(&self, id: &str) -> Result<Arc<Handle>, RpcError> {
        let number = parse_terminal_number(id)?;
        self.shared
            .table()
            .get(&number)
            .and_then(|entry| entry.handle.clone())
            .ok_or_else(|| RpcError::not_found(format!("running terminal {id}")))
    }

    /// Every event this Terminal emits from now on, in order. `NOT_FOUND` if it is unknown or has
    /// exited, as for [`Terminals::write`]: an exited Terminal has no more events, so a receiver
    /// would never yield or close.
    /// A subscriber that falls too far behind gets `RecvError::Lagged` and then the newest events;
    /// it never slows the PTY reader. For output from the first byte and a guaranteed exit event,
    /// use [`Spawned::events`] instead.
    pub fn subscribe(&self, id: &str) -> Result<broadcast::Receiver<EventData>, RpcError> {
        let number = parse_terminal_number(id)?;
        let table = self.shared.table();
        table
            .get(&number)
            .filter(|entry| entry.handle.is_some())
            .map(|entry| entry.events.subscribe())
            .ok_or_else(|| RpcError::not_found(format!("terminal {id}")))
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

    /// vt100 splits an OSC on every `;` and only reports a title that is a single piece, so a
    /// title containing `;` arrives here and would otherwise be dropped.
    fn unhandled_osc(&mut self, _: &mut vt100::Screen, params: &[&[u8]]) {
        if let [b"0" | b"2", pieces @ ..] = params {
            self.0
                .push(String::from_utf8_lossy(&pieces.join(&b';')).into_owned());
        }
    }
}

/// Feed queued writes to the PTY. It blocks on a program that does not read, which is why it has a
/// thread of its own instead of one from the pool `kill` also needs.
fn type_queued(number: u64, queued: mpsc::Receiver<Vec<u8>>, mut writer: Box<dyn Write + Send>) {
    for bytes in queued {
        if let Err(err) = writer.write_all(&bytes).and_then(|()| writer.flush()) {
            eprintln!("terminal {number}: input stopped: {err}");
            return;
        }
    }
}

/// Wait for the program to end without holding the lock while it runs, so `kill` can take it.
fn reap(program: &Program) -> std::io::Result<portable_pty::ExitStatus> {
    loop {
        let status = program.lock().expect("terminal program lock").try_wait()?;
        match status {
            Some(status) => return Ok(status),
            None => std::thread::sleep(REAP_POLL),
        }
    }
}

/// Read until the program closes the PTY, publishing each chunk, then report how it ended.
fn pump(shared: &Shared, number: u64, mut reader: Box<dyn Read + Send>, program: &Program) {
    let id = number.to_string();
    let mut parser = Some(vt100::Parser::new_with_callbacks(
        TITLE_ROWS,
        TITLE_COLS,
        0,
        Titles::default(),
    ));
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
        // A parser bug must cost titles, never the reader: the Terminal has to report its exit.
        let parsed = parser.as_mut().map(|parser| {
            catch_unwind(AssertUnwindSafe(|| {
                parser.process(&chunk[..read]);
                std::mem::take(&mut parser.callbacks_mut().0)
            }))
        });
        match parsed {
            Some(Ok(titles)) => titles
                .into_iter()
                .for_each(|title| shared.retitle(number, title)),
            Some(Err(_)) => {
                eprintln!("terminal {number}: title parser panicked; titles are off");
                parser = None;
            }
            None => {}
        }
    }
    let code = match reap(program) {
        Ok(status) if status.signal().is_some() => None,
        Ok(status) => Some(i32::try_from(status.exit_code()).unwrap_or(i32::MAX)),
        Err(err) => {
            // `None` means "killed by a signal"; an unknown ending must not look like one.
            eprintln!("terminal {number}: cannot wait for program: {err}");
            Some(-1)
        }
    };
    shared.finish(number, code);
}

/// What the OS would refuse at exec time, caught here so it is the caller's error with a short message.
fn check_program(params: &SpawnParams) -> Result<(), RpcError> {
    let argv = params.command.as_deref().unwrap_or_default();
    if params.command.is_some() && argv.first().is_none_or(String::is_empty) {
        return Err(invalid("command must start with a program name"));
    }
    let env = params.env.iter().flat_map(|(name, value)| [name, value]);
    if argv.iter().chain(env).any(|text| text.contains('\0')) {
        return Err(invalid("command and env must not contain NUL"));
    }
    if params
        .env
        .keys()
        .any(|name| name.is_empty() || name.contains('='))
    {
        return Err(invalid("env names must be non-empty and contain no '='"));
    }
    Ok(())
}

/// The number a Terminal id names. Only the form `spawn` hands out is an id: `+5`, `05` and `nope`
/// are malformed, never a second spelling of an existing Terminal.
fn parse_terminal_number(id: &str) -> Result<u64, RpcError> {
    id.parse::<u64>()
        .ok()
        .filter(|number| number.to_string() == id)
        .ok_or_else(|| invalid(format!("not a terminal id: {id:?}")))
}

/// portable-pty reports an argv[0] it cannot find or run as text that can quote the Daemon's `PATH`;
/// that is the caller's error and must not leak `PATH`. Any other failure to start is the Daemon's.
fn spawn_error(program: Option<&str>, err: &impl std::fmt::Display) -> RpcError {
    match program {
        Some(program) if err.to_string().starts_with("Unable to spawn ") => {
            invalid(format!("cannot run {program}"))
        }
        _ => RpcError::internal(err),
    }
}

fn window(cols: u16, rows: u16) -> Result<PtySize, RpcError> {
    if cols == 0 || rows == 0 {
        return Err(invalid("cols and rows must be at least 1"));
    }
    Ok(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    })
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
