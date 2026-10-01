use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use contracts::Event;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::net::unix::OwnedWriteHalf;
use tokio::sync::{mpsc, oneshot};

use crate::error::{RpcError, code};

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, RpcError>>>>>;

/// A persistent connection: concurrent requests, plus pushed events after `events.subscribe`.
pub struct Client {
    writer: tokio::sync::Mutex<OwnedWriteHalf>,
    pending: Pending,
    next_id: AtomicU64,
    events: mpsc::UnboundedReceiver<Event>,
    /// Set once the read side has seen the connection end; a request made after is refused
    /// without writing it, instead of racing a write that may or may not land.
    closed: Arc<AtomicBool>,
}

impl Client {
    pub async fn connect(socket: &Path) -> io::Result<Self> {
        let (read, writer) = UnixStream::connect(socket).await?.into_split();
        let pending = Pending::default();
        let closed = Arc::new(AtomicBool::new(false));
        let (event_tx, events) = mpsc::unbounded_channel();
        tokio::spawn(read_loop(
            BufReader::new(read),
            Arc::clone(&pending),
            event_tx,
            Arc::clone(&closed),
        ));
        Ok(Self {
            writer: tokio::sync::Mutex::new(writer),
            pending,
            next_id: AtomicU64::new(1),
            events,
            closed,
        })
    }

    pub async fn request(&self, method: &str, params: impl Serialize) -> Result<Value, RpcError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(RpcError::internal(
                "the connection is already closed; the request was not sent",
            ));
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .map_err(RpcError::internal)?
            .insert(id, tx);
        let line = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        if let Err(err) = self
            .writer
            .lock()
            .await
            .write_all(format!("{line}\n").as_bytes())
            .await
        {
            self.pending.lock().map_err(RpcError::internal)?.remove(&id);
            return Err(RpcError::internal(format!(
                "the request was not sent: {err}"
            )));
        }
        match rx.await {
            Ok(outcome) => outcome,
            Err(_) => Err(RpcError::unknown_outcome(
                "the connection closed before the reply; the Daemon may have run the call",
            )),
        }
    }

    /// The next pushed event; `None` once the connection closes.
    pub async fn next_event(&mut self) -> Option<Event> {
        self.events.recv().await
    }
}

async fn read_loop(
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    pending: Pending,
    events: mpsc::UnboundedSender<Event>,
    closed: Arc<AtomicBool>,
) {
    let mut lines = reader.lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let Ok(frame) = serde_json::from_str::<Value>(&line) else {
            eprintln!("rpc: dropped a frame that is not JSON");
            continue;
        };
        if frame["method"] == "event" {
            match serde_json::from_value::<Event>(frame["params"].clone()) {
                Ok(event) => {
                    let _ = events.send(event);
                }
                Err(err) => {
                    eprintln!("rpc: dropped an event that does not match the contract: {err}")
                }
            }
            continue;
        }
        let Some(id) = frame["id"].as_u64() else {
            continue;
        };
        let outcome = match frame.get("error") {
            Some(error) => Err(serde_json::from_value(error.clone())
                .unwrap_or_else(|_| RpcError::new(code::INTERNAL, "malformed error reply"))),
            None => Ok(frame["result"].clone()),
        };
        if let Some(waiter) = pending.lock().ok().and_then(|mut map| map.remove(&id)) {
            let _ = waiter.send(outcome);
        }
    }
    // Connection closed: mark it before waking waiters, so a request racing the drop
    // either lands in `pending` (and is woken below) or sees `closed` and refuses itself.
    closed.store(true, Ordering::Release);
    if let Ok(mut map) = pending.lock() {
        map.clear();
    }
}
