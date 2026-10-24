//! Terminals: plain shell processes behind a PTY. Owner: terminal Builder.
//!
//! Rust callers (the agents module) use [`Terminals`] directly, with no socket: [`Terminals::spawn`]
//! returns the new Terminal's id together with a [`broadcast::Receiver`] of its events. The receiver
//! exists before the reader thread reads any output, so none is missed.

mod integration;
mod output;
mod scheme;

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
    ExitedEvent, ReadParams, ResizeParams, SchemeParams, SearchParams, Snapshot, SpawnParams,
    TerminalId, TerminalInfo, TitleEvent, WriteParams,
};
use contracts::{Actor, EventData};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use rpc::{Bus, Ctx, Module, OpenError, RpcError, code, params, reply};
use serde_json::Value;
use tokio::sync::broadcast;

/// Events a slow subscriber may fall behind by before it is told it lagged (it never blocks the reader).
const EVENT_BACKLOG: usize = 1024;
/// X14: output chunks a Terminal's slow output subscriber may fall behind by before it sees a gap.
const OUTPUT_BACKLOG: usize = 256;
const SNAPSHOT_SCROLLBACK: usize = 1000;
const SNAPSHOT_MAX_BYTES: usize = 1_048_576;
const SNAPSHOT_MAX_CELLS: u32 = 65_536;
const SNAPSHOT_MAX_HISTORY_CELLS: usize = 262_144;
const TITLE_ROWS: u16 = 24;
const TITLE_COLS: u16 = 80;
/// How often the reader thread checks whether a program that closed its PTY has ended.
const REAP_POLL: Duration = Duration::from_millis(5);
const READ_CHUNK: usize = 8192;
/// Writes a Terminal accepts ahead of its program reading them. Past this, `write` is `CONFLICT`:
/// a program that never reads would otherwise hold a thread per write until the blocking pool is
/// exhausted and `kill` cannot run.
const WRITE_BACKLOG: usize = 16;
/// X11: Terminals one owner holds, running or exited.
const OWNED_CAP: usize = 8;

/// A freshly spawned Terminal: its id, its title and exit events, and its output from the first byte.
pub struct Spawned {
    pub id: String,
    pub events: broadcast::Receiver<Arc<EventData>>,
    pub output: OutputStream,
}

/// X14: one chunk of a Terminal's output and the offset of its first byte.
#[derive(Clone)]
struct Chunk {
    offset: u64,
    bytes: Arc<[u8]>,
}

/// What a subscriber to one Terminal's output receives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Output {
    /// The next bytes, starting at `offset`.
    Bytes { offset: u64, bytes: Arc<[u8]> },
    /// The subscriber fell behind: bytes `from..to` were not delivered and delivery resumes at `to`.
    Gap { from: u64, to: u64 },
}

/// X14: the output of one Terminal from the offset it was taken at, in order. It never slows the
/// PTY reader or another Terminal's subscribers; a subscriber that falls behind sees a [`Output::Gap`].
pub struct OutputStream {
    receiver: broadcast::Receiver<Chunk>,
    /// The offset after the last byte delivered, which the next chunk must start at.
    next: u64,
    after_gap: Option<Chunk>,
}

impl OutputStream {
    /// The offset of the first byte this stream delivers.
    pub fn start(&self) -> u64 {
        self.next
    }

    /// The next output, or `None` once the Terminal's program has exited and every byte was delivered.
    pub async fn next(&mut self) -> Option<Output> {
        let chunk = match self.after_gap.take() {
            Some(chunk) => chunk,
            None => loop {
                match self.receiver.recv().await {
                    Ok(chunk) => break chunk,
                    // The next receive yields the oldest retained chunk, whose offset shows the gap.
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            },
        };
        if chunk.offset > self.next {
            let gap = Output::Gap {
                from: self.next,
                to: chunk.offset,
            };
            self.next = chunk.offset;
            self.after_gap = Some(chunk);
            return Some(gap);
        }
        self.next = chunk.offset + chunk.bytes.len() as u64;
        Some(Output::Bytes {
            offset: chunk.offset,
            bytes: chunk.bytes,
        })
    }
}

/// The program, shared by whoever reaps it and whoever stops it.
/// Invariant: only code holding this lock reaps or signals, and signals only when `try_wait` under
/// that lock has just said the program is still running. A reaped pid is never signalled.
type Program = Arc<Mutex<Box<dyn Child + Send + Sync>>>;

/// What it takes to drive a running program. Dropped from the Terminal's entry once it is reaped.
struct Handle {
    master: Mutex<Box<dyn MasterPty + Send>>,
    input: SyncSender<Input>,
    program: Program,
}

type InputWriter = dyn Fn(Box<dyn Write + Send>) -> Box<dyn Write + Send> + Send + Sync;

type ExitHold = dyn Fn() + Send + Sync;

type Effect = Box<dyn FnOnce(&mut dyn Write) -> Result<(), RpcError> + Send>;

enum Input {
    Bytes(Vec<u8>),
    Effect(Effect, tokio::sync::oneshot::Sender<Result<(), RpcError>>),
}

struct Entry {
    info: TerminalInfo,
    /// The program's pid while it runs; gone at its exit, before `terminal.exited` is published,
    /// so a pid the OS reuses afterwards names no Terminal (H21).
    pid: Option<u32>,
    events: broadcast::Sender<Arc<EventData>>,
    /// X14: dropped when the program exits, which ends every output stream after its last byte.
    output: Option<broadcast::Sender<Chunk>>,
    handle: Option<Arc<Handle>>,
    screen: Arc<Mutex<ScreenState>>,
}

struct ScreenState {
    parser: ScreenParser,
    after: u64,
    /// X13: the last megabyte of output; its end is `after`.
    log: output::Log,
    /// X16: where each reader's next `terminal.read` begins; it ends with the Terminal.
    cursors: std::collections::HashMap<String, u64>,
    snapshot_available: bool,
    scrollback_rows: usize,
    cols: u16,
    rows: u16,
    /// X17: the program turned on mode 2031 and gets a notice when the scheme changes.
    scheme_notices: bool,
}

enum ScreenParser {
    Pending,
    Active(Box<vt100::Parser<Seen>>),
    Failed,
}

impl ScreenState {
    fn ensure_parser(&mut self) -> Option<&mut vt100::Parser<Seen>> {
        if matches!(&self.parser, ScreenParser::Pending) {
            let (rows, cols, scrollback) = if self.snapshot_available {
                (self.rows, self.cols, self.scrollback_rows)
            } else {
                (TITLE_ROWS, TITLE_COLS, 0)
            };
            self.parser = ScreenParser::Active(Box::new(vt100::Parser::new_with_callbacks(
                rows,
                cols,
                scrollback,
                Seen::default(),
            )));
        }
        match &mut self.parser {
            ScreenParser::Active(parser) => Some(parser),
            ScreenParser::Pending | ScreenParser::Failed => None,
        }
    }
}

/// State shared with each Terminal's reader thread.
struct Shared {
    bus: Bus,
    table: Mutex<BTreeMap<u64, Entry>>,
    /// Env var names stripped from every spawned program's environment, however they reached the
    /// Daemon: its own environment or a caller's `params.env` (`open_with`'s caller owns the list).
    clean_env: Vec<String>,
    /// X17: the App's scheme; `None` until it is told, and until then no program is answered.
    scheme: Mutex<Option<scheme::Palette>>,
}

impl Shared {
    fn table(&self) -> MutexGuard<'_, BTreeMap<u64, Entry>> {
        self.table.lock().expect("terminal table lock")
    }

    /// Hand a title or exit event to the Bus, then to this Terminal's subscribers, so a subscriber
    /// that saw the event can rely on the Bus having it. Neither can block the caller.
    fn publish(&self, number: u64, data: EventData) {
        self.bus.emit(Actor::daemon(), data.clone());
        if let Some(entry) = self.table().get(&number) {
            let _ = entry.events.send(Arc::new(data));
        }
    }

    fn retitle(&self, number: u64, title: String) {
        let id = number.to_string();
        if let Some(entry) = self.table().get_mut(&number) {
            entry.info.title = Some(title.clone());
        }
        self.publish(number, EventData::TerminalTitle(TitleEvent { id, title }));
    }

    /// X17: queue `bytes` for the program of Terminal `number`; a program that no longer reads is told on stderr.
    fn type_into(&self, number: u64, bytes: Vec<u8>) {
        let handle = self
            .table()
            .get(&number)
            .and_then(|entry| entry.handle.clone());
        if let Some(handle) = handle
            && let Err(err) = handle.input.try_send(Input::Bytes(bytes))
        {
            eprintln!("terminal {number}: scheme reply dropped: {err}");
        }
    }

    /// X17: one reply per query, in the order asked, from the scheme the App last set.
    fn answer(&self, number: u64, requests: &[scheme::Request]) {
        let Some(palette) = *self.scheme.lock().expect("terminal scheme lock") else {
            return;
        };
        let replies: Vec<u8> = requests
            .iter()
            .filter_map(|request| match request {
                scheme::Request::Ask(query) => Some(palette.answer(*query)),
                scheme::Request::Notices(_) => None,
            })
            .flatten()
            .collect();
        if !replies.is_empty() {
            self.type_into(number, replies);
        }
    }

    fn finish(&self, number: u64, code: Option<i32>) {
        let id = number.to_string();
        if let Some(entry) = self.table().get_mut(&number) {
            entry.info.running = false;
            entry.handle = None;
            entry.pid = None;
            entry.info.exit_code = code;
            entry.output = None;
        }
        self.publish(number, EventData::TerminalExited(ExitedEvent { id, code }));
    }
}

pub struct Terminals {
    shared: Arc<Shared>,
    /// Where X15's shell integration files are written.
    integration_dir: std::path::PathBuf,
    next: AtomicU64,
    input_writer: Option<Arc<InputWriter>>,
    exit_hold: Option<Arc<ExitHold>>,
}

impl Terminals {
    /// `dir` is the Project's data folder (N1). `bus` is for events no call caused. No env
    /// var is stripped from a spawned program; see [`Terminals::open_with`] for that.
    pub fn open(dir: &Path, bus: Bus) -> Result<Self, OpenError> {
        Self::open_with(dir, bus, &[])
    }

    /// As [`Terminals::open`], but `clean_env` names env vars that must never reach a spawned
    /// program's environment, however they arrived (the Daemon's own environment or a caller's
    /// `params.env`): the caller owns the list and what it means.
    pub fn open_with(dir: &Path, bus: Bus, clean_env: &[&str]) -> Result<Self, OpenError> {
        Ok(Self {
            shared: Arc::new(Shared {
                bus,
                table: Mutex::new(BTreeMap::new()),
                clean_env: clean_env
                    .iter()
                    .map(|marker| (*marker).to_owned())
                    .collect(),
                scheme: Mutex::new(None),
            }),
            integration_dir: dir.join("shell-integration"),
            next: AtomicU64::new(1),
            input_writer: None,
            exit_hold: None,
        })
    }

    /// Adapt each PTY input writer before its input thread starts, for isolated physical-IO observers.
    pub fn with_input_writer(
        mut self,
        adapt: impl Fn(Box<dyn Write + Send>) -> Box<dyn Write + Send> + Send + Sync + 'static,
    ) -> Self {
        self.input_writer = Some(Arc::new(adapt));
        self
    }

    /// Run `hold` after a program is reaped and before its Terminal is reported exited, for isolated
    /// observers that keep an exit unconfirmed on any host, whether or not its PTY reports EOF.
    pub fn with_exit_hold(mut self, hold: impl Fn() + Send + Sync + 'static) -> Self {
        self.exit_hold = Some(Arc::new(hold));
        self
    }

    /// Start `params.command` (or the login shell) in a new PTY.
    /// A missing cwd, an empty command, or a program that cannot be found or run is the caller's error;
    /// any other failure to start is the Daemon's.
    pub async fn spawn(&self, params: SpawnParams) -> Result<Spawned, RpcError> {
        self.start(params, None).await
    }

    /// As [`Terminals::spawn`], for a Terminal that `owner` shows as a bubble (X11, X12). A ninth
    /// forgets the owner's oldest exited Terminal, and is `CONFLICT` when all eight run. Emits
    /// `terminal.changed`.
    pub async fn spawn_owned(&self, params: SpawnParams, owner: &str) -> Result<Spawned, RpcError> {
        let forgot = self.make_room(owner)?;
        let spawned = self.start(params, Some(owner.to_owned())).await;
        if forgot || spawned.is_ok() {
            self.shared
                .bus
                .emit(Actor::daemon(), EventData::TerminalChanged);
        }
        spawned
    }

    /// X12, X15: the login shell at `shell` in `cwd`, owned by `owner`, with `env` over the Daemon's. A zsh, bash or fish prints
    /// shell marks; any other shell starts as `shell -l` with no change. Emits `terminal.changed`.
    pub async fn spawn_shell_owned(
        &self,
        cwd: &str,
        shell: &str,
        owner: &str,
        env: BTreeMap<String, String>,
    ) -> Result<Spawned, RpcError> {
        let start =
            integration::start(&self.integration_dir, shell, &env).map_err(RpcError::internal)?;
        let mut command = vec![shell.to_owned()];
        command.extend(start.args);
        self.spawn_owned(
            SpawnParams {
                cwd: cwd.to_owned(),
                command: Some(command),
                env: start.env,
                cols: 80,
                rows: 24,
            },
            owner,
        )
        .await
    }

    /// X12: stop Terminal `id` if it runs and forget it. `INVALID_PARAMS` for one with no owner.
    pub async fn close(&self, id: &str) -> Result<(), RpcError> {
        let number = parse_terminal_number(id)?;
        let running = {
            let table = self.shared.table();
            let entry = table
                .get(&number)
                .ok_or_else(|| RpcError::not_found(format!("terminal {id}")))?;
            if entry.info.owner.is_none() {
                return Err(invalid(format!("terminal {id} has no owner to close it")));
            }
            entry.handle.is_some()
        };
        if running {
            match self.kill(id).await {
                Ok(()) => {}
                // It ended on its own just before; closing it is still right.
                Err(err) if err.code == code::NOT_FOUND => {}
                Err(err) => return Err(err),
            }
        }
        if self.shared.table().remove(&number).is_none() {
            return Err(RpcError::not_found(format!("terminal {id}")));
        }
        self.shared
            .bus
            .emit(Actor::daemon(), EventData::TerminalChanged);
        Ok(())
    }

    /// Close every Terminal `owner` holds; one `terminal.changed` for all, none for none.
    pub async fn close_owned_by(&self, owner: &str) -> Result<(), RpcError> {
        let ids: Vec<u64> = self
            .shared
            .table()
            .iter()
            .filter(|(_, entry)| entry.info.owner.as_deref() == Some(owner))
            .map(|(number, _)| *number)
            .collect();
        for number in &ids {
            let id = number.to_string();
            match self.kill(&id).await {
                Ok(()) => {}
                Err(err) if err.code == code::NOT_FOUND => {}
                Err(err) => return Err(err),
            }
            self.shared.table().remove(number);
        }
        if !ids.is_empty() {
            self.shared
                .bus
                .emit(Actor::daemon(), EventData::TerminalChanged);
        }
        Ok(())
    }

    /// Forget `owner`'s oldest exited Terminal when it holds [`OWNED_CAP`]; true when one went.
    fn make_room(&self, owner: &str) -> Result<bool, RpcError> {
        let mut table = self.shared.table();
        let parked: Vec<(u64, bool)> = table
            .iter()
            .filter(|(_, entry)| entry.info.owner.as_deref() == Some(owner))
            .map(|(number, entry)| (*number, entry.handle.is_none()))
            .collect();
        if parked.len() < OWNED_CAP {
            return Ok(false);
        }
        let oldest_exited = parked
            .iter()
            .find(|(_, exited)| *exited)
            .map(|(number, _)| *number);
        match oldest_exited {
            Some(number) => {
                table.remove(&number);
                Ok(true)
            }
            None => Err(RpcError::conflict(format!(
                "{owner} already runs {OWNED_CAP} terminals"
            ))),
        }
    }

    async fn start(&self, params: SpawnParams, owner: Option<String>) -> Result<Spawned, RpcError> {
        check_program(&params)?;
        // portable-pty silently falls back to $HOME when the cwd is unusable; refuse instead.
        if !Path::new(&params.cwd).is_dir() {
            return Err(invalid(format!("cwd is not a directory: {}", params.cwd)));
        }
        // Generate independently of authentication and vendor content; failure starts no program.
        let mut nonce = [0u8; 32];
        getrandom::fill(&mut nonce).map_err(RpcError::internal)?;
        let incarnation = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
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
        // Last, so none of them survive even if a caller passed one in `params.env`.
        for marker in &self.shared.clean_env {
            command.env_remove(marker);
        }
        // The reader and input threads start before the program (the reader receives the child over a
        // channel), so no later failure can leave a program that nobody waits for.
        let reader = pair.master.try_clone_reader().map_err(RpcError::internal)?;
        let writer = pair.master.take_writer().map_err(RpcError::internal)?;
        let writer = match &self.input_writer {
            Some(adapt) => adapt(writer),
            None => writer,
        };
        let number = self.next.fetch_add(1, Ordering::Relaxed);
        let snapshot_available = snapshot_size_supported(params.cols, params.rows);
        let scrollback = if snapshot_available {
            SNAPSHOT_SCROLLBACK.min(SNAPSHOT_MAX_HISTORY_CELLS / usize::from(params.cols))
        } else {
            0
        };
        let screen = Arc::new(Mutex::new(ScreenState {
            parser: ScreenParser::Pending,
            after: 0,
            log: output::Log::default(),
            cursors: Default::default(),
            snapshot_available,
            scrollback_rows: scrollback,
            cols: params.cols,
            rows: params.rows,
            scheme_notices: false,
        }));
        let (arrival, child_arrives) = mpsc::channel();
        let (output, output_receiver) = broadcast::channel(OUTPUT_BACKLOG);
        let pump_output = output.clone();
        let shared = Arc::clone(&self.shared);
        let reader_screen = Arc::clone(&screen);
        let exit_hold = self.exit_hold.clone();
        std::thread::Builder::new()
            .name(format!("terminal-{number}"))
            .spawn(move || {
                // No child means the program never started: nothing to report.
                if let Ok(program) = child_arrives.recv() {
                    pump(
                        &shared,
                        number,
                        reader,
                        &program,
                        &reader_screen,
                        pump_output,
                        exit_hold.as_deref(),
                    );
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
        let pid = program.lock().expect("program lock").process_id();
        let handle = Handle {
            input,
            program: Arc::clone(&program),
            master: Mutex::new(pair.master),
        };

        let (sender, events) = broadcast::channel(EVENT_BACKLOG);
        self.shared.table().insert(
            number,
            Entry {
                pid,
                info: TerminalInfo {
                    id: number.to_string(),
                    incarnation,
                    cwd: params.cwd,
                    title: None,
                    program: program_name(params.command.as_deref()),
                    running: true,
                    exit_code: None,
                    owner,
                },
                events: sender,
                output: Some(output.clone()),
                handle: Some(Arc::new(handle)),
                screen,
            },
        );
        arrival
            .send(program)
            .expect("the reader thread waits for its child");
        Ok(Spawned {
            id: number.to_string(),
            events,
            output: OutputStream {
                receiver: output_receiver,
                next: 0,
                after_gap: None,
            },
        })
    }

    /// Queue `bytes` to be typed into a running Terminal and return without waiting for the program to
    /// read them. `NOT_FOUND` if it is unknown or has exited; `CONFLICT` if the program has left
    /// [`WRITE_BACKLOG`] writes unread.
    pub async fn write(&self, id: &str, bytes: &[u8]) -> Result<(), RpcError> {
        self.enqueue(id, Input::Bytes(bytes.to_vec()))
    }

    /// Await the physical write and flush in input order. Refusal to enqueue has no effect;
    /// IO failure or a lost input-thread result is UNKNOWN_OUTCOME and must not be retried.
    /// Success leaves no bytes from this call queued (it does not await the child's read).
    pub async fn write_completed(&self, id: &str, bytes: Vec<u8>) -> Result<(), RpcError> {
        self.write_with(id, move |writer| {
            writer
                .write_all(&bytes)
                .and_then(|()| writer.flush())
                .map_err(|err| {
                    RpcError::unknown_outcome(format!(
                        "Terminal write may be partial: {err}; do not retry"
                    ))
                })
        })
        .await
    }

    /// Run an effect on the input thread at its place in the Terminal's write order, and
    /// return its result after it finishes. The effect may refuse before writing; a queued
    /// effect whose caller disappeared is skipped. B6 uses this to hold the prompt check
    /// through the physical write, rather than checking before the input queue.
    pub async fn write_with(
        &self,
        id: &str,
        effect: impl FnOnce(&mut dyn Write) -> Result<(), RpcError> + Send + 'static,
    ) -> Result<(), RpcError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.enqueue(id, Input::Effect(Box::new(effect), tx))?;
        rx.await.map_err(|_| {
            RpcError::unknown_outcome(
                "Terminal input ended before reporting its physical effect; do not retry",
            )
        })?
    }

    fn enqueue(&self, id: &str, input: Input) -> Result<(), RpcError> {
        match self.running(id)?.input.try_send(input) {
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
        let screen = self.screen(id)?;
        let mut state = screen.lock().expect("terminal screen lock");
        let master = handle.master.lock().expect("terminal master lock");
        master.resize(size).map_err(RpcError::internal)?;
        state.cols = cols;
        state.rows = rows;
        if state.snapshot_available
            && snapshot_size_supported(cols, rows)
            && (matches!(&state.parser, ScreenParser::Pending)
                || state.scrollback_rows * usize::from(cols) <= SNAPSHOT_MAX_HISTORY_CELLS)
        {
            match &mut state.parser {
                ScreenParser::Pending => {
                    state.scrollback_rows =
                        SNAPSHOT_SCROLLBACK.min(SNAPSHOT_MAX_HISTORY_CELLS / usize::from(cols));
                }
                ScreenParser::Active(parser) => parser.screen_mut().set_size(rows, cols),
                ScreenParser::Failed => {}
            }
        } else {
            // A size beyond the capture budget loses screen history; a later shrink cannot restore it.
            state.snapshot_available = false;
        }
        Ok(())
    }

    /// Return the Terminal's last screen and the output offset represented by it, from one cut.
    pub fn snapshot(&self, id: &str) -> Result<Snapshot, RpcError> {
        let screen = self.screen(id)?;
        let mut state = screen.lock().expect("terminal screen lock");
        if !state.snapshot_available {
            return Err(RpcError::conflict(format!(
                "terminal {id} screen is too large to snapshot"
            )));
        }
        let after = state.after;
        let parser = state
            .ensure_parser()
            .ok_or_else(|| RpcError::internal("terminal screen parser stopped"))?;
        let (rows, cols) = parser.screen().size();
        let data = formatted_snapshot(parser.screen())?;
        Ok(Snapshot {
            cols,
            rows,
            after,
            data: STANDARD.encode(data),
        })
    }

    /// X13: the screen as it is now, as plain text. Changes nothing.
    pub fn read_screen(
        &self,
        id: &str,
        max: Option<u32>,
    ) -> Result<contracts::terminal::Screen, RpcError> {
        let cap = output::cap(max)?;
        let screen = self.screen(id)?;
        let mut state = screen.lock().expect("terminal screen lock");
        if !state.snapshot_available {
            return Err(RpcError::conflict(format!(
                "terminal {id} screen is too large to read"
            )));
        }
        let offset = state.log.end();
        let parser = state
            .ensure_parser()
            .ok_or_else(|| RpcError::internal("terminal screen parser stopped"))?;
        let (rows, cols) = parser.screen().size();
        let shown = parser.screen().rows(0, cols).collect();
        Ok(output::screen(cols, rows, offset, shown, cap))
    }

    /// X13, X16: the output from `params.after` on, or from the reader's cursor, as plain text.
    /// Writes nothing and emits nothing; only a read by `reader` moves that reader's cursor.
    pub fn read(&self, params: &ReadParams) -> Result<contracts::terminal::Read, RpcError> {
        let cap = output::cap(params.max)?;
        let (after, reader) = match (params.after, params.reader.as_deref()) {
            (Some(after), _) => (Some(after), None),
            (None, Some(reader)) => (None, Some(reader)),
            (None, None) => {
                return Err(invalid("terminal.read needs `after` or `reader`"));
            }
        };
        let screen = self.screen(&params.id)?;
        let mut state = screen.lock().expect("terminal screen lock");
        let start = after.unwrap_or_else(|| {
            reader
                .and_then(|r| state.cursors.get(r).copied())
                .unwrap_or(0)
        });
        let window = state.log.from(start)?;
        let read = output::read(&window, cap);
        if let Some(reader) = reader {
            state.cursors.insert(reader.to_owned(), read.next);
        }
        Ok(read)
    }

    /// X13: the lines of the output that match `params.pattern`. Changes nothing, and searches a
    /// copy of the log with no lock parked.
    pub fn search(&self, params: &SearchParams) -> Result<contracts::terminal::Search, RpcError> {
        let cap = output::cap(params.max)?;
        let window = self.window(&params.id, params.after.unwrap_or(0))?;
        output::search(&window, &params.pattern, cap)
    }

    fn window(&self, id: &str, after: u64) -> Result<output::Window, RpcError> {
        let screen = self.screen(id)?;
        let state = screen.lock().expect("terminal screen lock");
        state.log.from(after)
    }

    fn screen(&self, id: &str) -> Result<Arc<Mutex<ScreenState>>, RpcError> {
        let number = parse_terminal_number(id)?;
        self.shared
            .table()
            .get(&number)
            .map(|entry| Arc::clone(&entry.screen))
            .ok_or_else(|| RpcError::not_found(format!("terminal {id}")))
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

    /// Whether Terminal `id`'s program has ended, which can be before the Terminal is exited: a
    /// descendant holding the PTY open delays `terminal.exited` on Linux. An unknown Terminal has
    /// no program running either.
    pub fn program_ended(&self, id: &str) -> bool {
        match self.running(id) {
            Ok(handle) => handle
                .program
                .lock()
                .expect("terminal program lock")
                .try_wait()
                .map_or(true, |status| status.is_some()),
            Err(_) => true,
        }
    }

    /// The pid of Terminal `id`'s program, only while it runs (H21).
    pub fn program_pid(&self, id: &str) -> Option<u32> {
        let number = parse_terminal_number(id).ok()?;
        self.shared.table().get(&number).and_then(|entry| entry.pid)
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
    pub fn subscribe(&self, id: &str) -> Result<broadcast::Receiver<Arc<EventData>>, RpcError> {
        let number = parse_terminal_number(id)?;
        let table = self.shared.table();
        table
            .get(&number)
            .filter(|entry| entry.handle.is_some())
            .map(|entry| entry.events.subscribe())
            .ok_or_else(|| RpcError::not_found(format!("terminal {id}")))
    }

    /// X14: the output of a running Terminal from its current end, in order. `NOT_FOUND` as for
    /// [`Terminals::subscribe`]. Dropping the stream releases the subscription.
    pub fn subscribe_output(&self, id: &str) -> Result<OutputStream, RpcError> {
        let number = parse_terminal_number(id)?;
        let (start, receiver) = {
            let table = self.shared.table();
            let entry = table
                .get(&number)
                .filter(|entry| entry.handle.is_some())
                .ok_or_else(|| RpcError::not_found(format!("terminal {id}")))?;
            let sender = entry
                .output
                .as_ref()
                .ok_or_else(|| RpcError::not_found(format!("terminal {id}")))?;
            // The reader sends a chunk under this lock, so the end read here is where the receiver starts.
            let screen = Arc::clone(&entry.screen);
            let state = screen.lock().expect("terminal screen lock");
            let start = state.after;
            let receiver = sender.subscribe();
            drop(state);
            (start, receiver)
        };
        Ok(OutputStream {
            receiver,
            next: start,
            after_gap: None,
        })
    }

    /// X17: set the scheme every program is answered in. A change from the one before (or from none)
    /// sends one notice to each running program that turned on mode 2031.
    pub async fn set_scheme(&self, params: SchemeParams) -> Result<(), RpcError> {
        let palette = scheme::Palette::parse(&params)?;
        let before = self
            .shared
            .scheme
            .lock()
            .expect("terminal scheme lock")
            .replace(palette);
        if before.is_some_and(|before| before.scheme == palette.scheme) {
            return Ok(());
        }
        let running: Vec<(u64, Arc<Mutex<ScreenState>>)> = self
            .shared
            .table()
            .iter()
            .filter(|(_, entry)| entry.handle.is_some())
            .map(|(number, entry)| (*number, Arc::clone(&entry.screen)))
            .collect();
        for (number, screen) in running {
            if screen.lock().expect("terminal screen lock").scheme_notices {
                self.shared.type_into(number, palette.notice());
            }
        }
        Ok(())
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

/// Collects what the parser sees that the Daemon acts on: window titles (OSC 0 and 2) and the colour
/// requests of X17. vt100 stitches sequences split across reads.
#[derive(Default)]
struct Seen {
    titles: Vec<String>,
    requests: Vec<scheme::Request>,
}

impl vt100::Callbacks for Seen {
    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        self.titles
            .push(String::from_utf8_lossy(title).into_owned());
    }

    fn unhandled_csi(
        &mut self,
        _: &mut vt100::Screen,
        first: Option<u8>,
        second: Option<u8>,
        params: &[&[u16]],
        last: char,
    ) {
        self.requests
            .extend(scheme::from_csi(first, second, params, last));
    }

    /// vt100 splits an OSC on every `;` and only reports a title that is a single piece, so a
    /// title containing `;` arrives here and would otherwise be dropped.
    fn unhandled_osc(&mut self, _: &mut vt100::Screen, params: &[&[u8]]) {
        if let [b"0" | b"2", pieces @ ..] = params {
            self.titles
                .push(String::from_utf8_lossy(&pieces.join(&b';')).into_owned());
        }
        self.requests.extend(scheme::from_osc(params));
    }
}

/// Feed queued writes to the PTY. It blocks on a program that does not read, which is why it has a
/// thread of its own instead of one from the pool `kill` also needs.
fn type_queued(number: u64, queued: mpsc::Receiver<Input>, mut writer: Box<dyn Write + Send>) {
    for input in queued {
        match input {
            Input::Bytes(bytes) => {
                if let Err(err) = writer.write_all(&bytes).and_then(|()| writer.flush()) {
                    eprintln!("terminal {number}: input stopped: {err}");
                    return;
                }
            }
            Input::Effect(effect, completed) => {
                if !completed.is_closed() {
                    let _ = completed.send(effect(&mut *writer));
                }
            }
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
fn pump(
    shared: &Shared,
    number: u64,
    mut reader: Box<dyn Read + Send>,
    program: &Program,
    screen: &Mutex<ScreenState>,
    output: broadcast::Sender<Chunk>,
    exit_hold: Option<&ExitHold>,
) {
    let mut chunk = [0u8; READ_CHUNK];
    // Linux reports the closed PTY as an error, macOS as end of file: both mean "no more output".
    while let Ok(read) = reader.read(&mut chunk) {
        if read == 0 {
            break;
        }
        let seen = {
            let mut state = screen.lock().expect("terminal screen lock");
            let offset = state.after;
            let parsed = state.ensure_parser().map(|parser| {
                catch_unwind(AssertUnwindSafe(|| {
                    parser.process(&chunk[..read]);
                    std::mem::take(parser.callbacks_mut())
                }))
            });
            state.after += read as u64;
            state.log.push(&chunk[..read]);
            // Sent under the lock so a snapshot's `after` and a subscription's start never straddle a chunk.
            let _ = output.send(Chunk {
                offset,
                bytes: Arc::from(&chunk[..read]),
            });
            match parsed {
                Some(Ok(seen)) => {
                    for request in &seen.requests {
                        if let scheme::Request::Notices(on) = request {
                            state.scheme_notices = *on;
                        }
                    }
                    seen
                }
                Some(Err(_)) => {
                    eprintln!("terminal {number}: screen parser panicked; snapshots are off");
                    state.parser = ScreenParser::Failed;
                    Seen::default()
                }
                None => Seen::default(),
            }
        };
        shared.answer(number, &seen.requests);
        seen.titles
            .into_iter()
            .for_each(|title| shared.retitle(number, title));
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
    if let Some(hold) = exit_hold {
        hold();
    }
    drop(output);
    shared.finish(number, code);
}

fn formatted_snapshot(screen: &vt100::Screen) -> Result<Vec<u8>, RpcError> {
    let visible = screen.state_formatted();
    let alternate = screen.alternate_screen();
    let prefix = if alternate {
        b"\x1b[?1049h".as_slice()
    } else {
        &[]
    };
    if prefix.len() + visible.len() > SNAPSHOT_MAX_BYTES {
        return Err(RpcError::conflict(
            "terminal visible screen exceeds snapshot limit",
        ));
    }

    let mut history = Vec::new();
    if !alternate {
        let mut prior = screen.clone();
        prior.set_scrollback(SNAPSHOT_SCROLLBACK);
        let count = prior.scrollback();
        let (_, cols) = prior.size();
        for offset in (1..=count).rev() {
            prior.set_scrollback(offset);
            let row = prior.rows(0, cols).next().unwrap_or_default();
            let mut line = row.into_bytes();
            line.extend_from_slice(b"\r\n");
            history.push(line);
        }
    }
    let mut total = prefix.len() + visible.len() + history.iter().map(Vec::len).sum::<usize>();
    while total > SNAPSHOT_MAX_BYTES {
        total -= history.remove(0).len();
    }
    let mut data = Vec::with_capacity(total);
    data.extend_from_slice(prefix);
    for line in history {
        data.extend_from_slice(&line);
    }
    data.extend_from_slice(&visible);
    Ok(data)
}

fn snapshot_size_supported(cols: u16, rows: u16) -> bool {
    u32::from(cols) * u32::from(rows) <= SNAPSHOT_MAX_CELLS
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

    async fn call(&self, ctx: &Ctx, method: &str, value: Value) -> Result<Value, RpcError> {
        match method {
            "terminal.spawn" => {
                // M14: a Terminal an Agent starts is owned by it; the user's and an Extension's is ownerless (X1).
                let spawned = if ctx.actor.kind == contracts::ActorKind::Agent {
                    self.spawn_owned(params(value)?, &ctx.actor.id).await?
                } else {
                    self.spawn(params(value)?).await?
                };
                reply(&TerminalId { id: spawned.id })
            }
            "terminal.write" => {
                let WriteParams { id, data } = params(value)?;
                let bytes = STANDARD
                    .decode(data)
                    .map_err(|err| invalid(format!("data is not base64: {err}")))?;
                if ctx.actor.kind == contracts::ActorKind::Agent {
                    self.write_completed(&id, bytes).await.and(reply(&()))
                } else {
                    self.write(&id, &bytes).await.and(reply(&()))
                }
            }
            "terminal.resize" => {
                let ResizeParams { id, cols, rows } = params(value)?;
                self.resize(&id, cols, rows).await.and(reply(&()))
            }
            "terminal.close" => {
                let TerminalId { id } = params(value)?;
                self.close(&id).await.and(reply(&()))
            }
            "terminal.kill" => {
                let TerminalId { id } = params(value)?;
                self.kill(&id).await.and(reply(&()))
            }
            "terminal.scheme" => self.set_scheme(params(value)?).await.and(reply(&())),
            "terminal.show" => {
                let TerminalId { id } = params(value)?;
                self.screen(&id)?;
                self.shared
                    .bus
                    .emit(Actor::daemon(), EventData::TerminalShown(TerminalId { id }));
                reply(&())
            }
            "terminal.list" => reply(&self.list()),
            "terminal.snapshot" => {
                let TerminalId { id } = params(value)?;
                reply(&self.snapshot(&id)?)
            }
            "terminal.screen" => {
                let TerminalId { id } = params(value)?;
                reply(&self.read_screen(&id, None)?)
            }
            "terminal.read" => reply(&self.read(&params::<ReadParams>(value)?)?),
            "terminal.search" => reply(&self.search(&params::<SearchParams>(value)?)?),
            _ => Err(RpcError::method_not_found(method)),
        }
    }
}

/// The basename of the program's argv0, else of `$SHELL`, else `terminal`.
fn program_name(command: Option<&[String]>) -> String {
    let shell = std::env::var("SHELL").ok();
    let path = command.and_then(<[String]>::first).or(shell.as_ref());

    path.and_then(|path| Path::new(path).file_name())
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("terminal")
        .to_string()
}
