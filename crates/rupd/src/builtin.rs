//! Methods the Daemon answers itself: `daemon.*`, `events.*`, `provenance.*`.

use contracts::{HistoryParams, IdentifyParams, TouchedParams};
use rpc::{RpcError, params, reply};
use serde_json::{Value, json};

use crate::{Conn, Daemon};

/// `None` means the namespace is not the Daemon's; route it to a module.
pub(crate) fn call(
    daemon: &Daemon,
    conn: &mut Conn,
    namespace: &str,
    method: &str,
    args: &Value,
) -> Option<Result<Value, RpcError>> {
    Some(match (namespace, method) {
        (_, "daemon.ping") => Ok(json!({ "pong": true })),
        (_, "daemon.identify") => params::<IdentifyParams>(args.clone()).map(|p| {
            conn.actor = p.actor;
            Value::Null
        }),
        (_, "events.subscribe") => {
            subscribe(daemon, conn);
            Ok(Value::Null)
        }
        (_, "provenance.history") => params::<HistoryParams>(args.clone())
            .and_then(|p| daemon.touches.history(&p.item).map_err(RpcError::internal))
            .and_then(|touches| reply(&touches)),
        (_, "provenance.touched") => params::<TouchedParams>(args.clone())
            .and_then(|p| {
                daemon
                    .touches
                    .touched(&p.actor_id)
                    .map_err(RpcError::internal)
            })
            .and_then(|touches| reply(&touches)),
        ("daemon" | "events" | "provenance", _) => Err(RpcError::method_not_found(method)),
        _ => return None,
    })
}

fn subscribe(daemon: &Daemon, conn: &Conn) {
    let mut events = daemon.bus.subscribe();
    let frames = conn.events.clone();
    tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(event) => {
                    let frame = json!({ "jsonrpc": "2.0", "method": "event", "params": event });
                    if frames.send(frame).is_err() {
                        return;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
                    eprintln!("rupd: a subscriber fell behind and missed {missed} events");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            }
        }
    });
}
