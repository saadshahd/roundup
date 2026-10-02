//! SQLite persistence for Messages and Routes. `<dir>/messages.db` (ADR 0004, WAL).

use std::path::Path;

use contracts::Actor;
use contracts::message::{Delivery, Held, Message, MessageKind, MessageStatus, Route};
use rpc::RpcError;
use rusqlite::{Connection, OptionalExtension, params};

pub(crate) struct Store {
    db: Connection,
}

impl Store {
    pub(crate) fn open(path: &Path) -> rusqlite::Result<Self> {
        let db = Connection::open(path)?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                from_actor TEXT NOT NULL,
                to_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                body TEXT NOT NULL,
                reply_to INTEGER,
                status TEXT NOT NULL,
                reason TEXT,
                at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS routes (
                from_id TEXT NOT NULL,
                to_id TEXT NOT NULL,
                delivery TEXT NOT NULL,
                PRIMARY KEY (from_id, to_id)
            );",
        )?;
        Ok(Self { db })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn insert(
        &self,
        from: &Actor,
        to: &str,
        kind: MessageKind,
        body: &str,
        reply_to: Option<u32>,
        status: MessageStatus,
        reason: Option<Held>,
        at: i64,
    ) -> Result<Message, RpcError> {
        self.db
            .execute(
                "INSERT INTO messages (from_actor, to_id, kind, body, reply_to, status, reason, at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    actor_json(from),
                    to,
                    kind_str(kind),
                    body,
                    reply_to,
                    status_str(status),
                    reason.map(held_str),
                    at
                ],
            )
            .map_err(RpcError::internal)?;
        let id = u32::try_from(self.db.last_insert_rowid()).map_err(RpcError::internal)?;
        self.get(id)?
            .ok_or_else(|| RpcError::internal("message vanished right after it was inserted"))
    }

    pub(crate) fn get(&self, id: u32) -> Result<Option<Message>, RpcError> {
        self.db
            .query_row(
                "SELECT id, from_actor, to_id, kind, body, reply_to, status, reason, at
                 FROM messages WHERE id = ?1",
                [id],
                row_to_message,
            )
            .optional()
            .map_err(RpcError::internal)
    }

    pub(crate) fn list(&self) -> Result<Vec<Message>, RpcError> {
        self.db
            .prepare(
                "SELECT id, from_actor, to_id, kind, body, reply_to, status, reason, at
                 FROM messages ORDER BY id",
            )
            .and_then(|mut stmt| stmt.query_map([], row_to_message)?.collect())
            .map_err(RpcError::internal)
    }

    /// Moves a `held` Message to `status`, in the same statement that checks it is still `held`,
    /// so two concurrent callers can never both apply their move (B3). Returns whether the
    /// Message was `held`: `false` means it had already left that status (or never existed).
    pub(crate) fn release_held(
        &self,
        id: u32,
        status: MessageStatus,
        reason: Option<Held>,
    ) -> Result<bool, RpcError> {
        let changed = self
            .db
            .execute(
                "UPDATE messages SET status = ?2, reason = ?3 WHERE id = ?1 AND status = 'held'",
                params![id, status_str(status), reason.map(held_str)],
            )
            .map_err(RpcError::internal)?;
        Ok(changed == 1)
    }

    pub(crate) fn count_open(&self, to: &str) -> Result<u32, RpcError> {
        self.db
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE to_id = ?1 AND status IN ('pending', 'held')",
                [to],
                |row| row.get(0),
            )
            .map_err(RpcError::internal)
    }

    pub(crate) fn set_route(
        &self,
        from: &str,
        to: &str,
        delivery: Delivery,
    ) -> Result<(), RpcError> {
        self.db
            .execute(
                "INSERT INTO routes (from_id, to_id, delivery) VALUES (?1, ?2, ?3)
                 ON CONFLICT(from_id, to_id) DO UPDATE SET delivery = excluded.delivery",
                params![from, to, delivery_str(delivery)],
            )
            .map_err(RpcError::internal)?;
        Ok(())
    }

    pub(crate) fn get_route(&self, from: &str, to: &str) -> Result<Option<Delivery>, RpcError> {
        self.db
            .query_row(
                "SELECT delivery FROM routes WHERE from_id = ?1 AND to_id = ?2",
                params![from, to],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(RpcError::internal)
            .map(|value| value.map(|value| parse_delivery(&value)))
    }

    pub(crate) fn list_routes(&self) -> Result<Vec<Route>, RpcError> {
        self.db
            .prepare("SELECT from_id, to_id, delivery FROM routes ORDER BY from_id, to_id")
            .and_then(|mut stmt| {
                stmt.query_map([], |row| {
                    Ok(Route {
                        from: row.get(0)?,
                        to: row.get(1)?,
                        delivery: parse_delivery(&row.get::<_, String>(2)?),
                    })
                })?
                .collect()
            })
            .map_err(RpcError::internal)
    }
}

fn row_to_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    let from: String = row.get(1)?;
    Ok(Message {
        id: row.get(0)?,
        from: serde_json::from_str(&from).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(err))
        })?,
        to: row.get(2)?,
        kind: parse_kind(&row.get::<_, String>(3)?),
        body: row.get(4)?,
        reply_to: row.get(5)?,
        status: parse_status(&row.get::<_, String>(6)?),
        reason: row
            .get::<_, Option<String>>(7)?
            .map(|text| parse_held(&text)),
        at: row.get(8)?,
    })
}

fn actor_json(actor: &Actor) -> String {
    serde_json::to_string(actor).expect("an Actor is always serializable")
}

fn kind_str(kind: MessageKind) -> &'static str {
    match kind {
        MessageKind::Note => "note",
        MessageKind::Question => "question",
    }
}

fn parse_kind(text: &str) -> MessageKind {
    match text {
        "question" => MessageKind::Question,
        _ => MessageKind::Note,
    }
}

fn status_str(status: MessageStatus) -> &'static str {
    match status {
        MessageStatus::Pending => "pending",
        MessageStatus::Held => "held",
        MessageStatus::Delivered => "delivered",
        MessageStatus::Dropped => "dropped",
    }
}

fn parse_status(text: &str) -> MessageStatus {
    match text {
        "held" => MessageStatus::Held,
        "delivered" => MessageStatus::Delivered,
        "dropped" => MessageStatus::Dropped,
        _ => MessageStatus::Pending,
    }
}

fn held_str(held: Held) -> &'static str {
    match held {
        Held::AskFirst => "ask-first",
        Held::Takeover => "takeover",
        Held::Escalated => "escalated",
    }
}

fn parse_held(text: &str) -> Held {
    match text {
        "takeover" => Held::Takeover,
        "escalated" => Held::Escalated,
        _ => Held::AskFirst,
    }
}

fn delivery_str(delivery: Delivery) -> &'static str {
    match delivery {
        Delivery::Auto => "auto",
        Delivery::AskFirst => "ask-first",
        Delivery::Drop => "drop",
    }
}

fn parse_delivery(text: &str) -> Delivery {
    match text {
        "ask-first" => Delivery::AskFirst,
        "drop" => Delivery::Drop,
        _ => Delivery::Auto,
    }
}
