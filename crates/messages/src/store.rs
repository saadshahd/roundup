//! SQLite persistence for Messages and Routes. `<dir>/messages.db` (ADR 0004, WAL).

use std::path::Path;

use contracts::Actor;
use contracts::message::{Delivery, Message, MessageKind, MessageStatus, Reason, Route};
use rpc::RpcError;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

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
        reason: Option<Reason>,
        at: i64,
    ) -> Result<Message, RpcError> {
        self.db
            .execute(
                "INSERT INTO messages (from_actor, to_id, kind, body, reply_to, status, reason, at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    actor_json(from),
                    to,
                    text_of(kind),
                    body,
                    reply_to,
                    text_of(status),
                    reason.map(text_of),
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
        reason: Option<Reason>,
    ) -> Result<bool, RpcError> {
        let changed = self
            .db
            .execute(
                "UPDATE messages SET status = ?2, reason = ?3 WHERE id = ?1 AND status = 'held'",
                params![id, text_of(status), reason.map(text_of)],
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
                params![from, to, text_of(delivery)],
            )
            .map_err(RpcError::internal)?;
        Ok(())
    }

    pub(crate) fn get_route(&self, from: &str, to: &str) -> Result<Option<Delivery>, RpcError> {
        self.db
            .query_row(
                "SELECT delivery FROM routes WHERE from_id = ?1 AND to_id = ?2",
                params![from, to],
                |row| from_text(0, &row.get::<_, String>(0)?),
            )
            .optional()
            .map_err(RpcError::internal)
    }

    pub(crate) fn list_routes(&self) -> Result<Vec<Route>, RpcError> {
        self.db
            .prepare("SELECT from_id, to_id, delivery FROM routes ORDER BY from_id, to_id")
            .and_then(|mut stmt| {
                stmt.query_map([], |row| {
                    Ok(Route {
                        from: row.get(0)?,
                        to: row.get(1)?,
                        delivery: from_text(2, &row.get::<_, String>(2)?)?,
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
        kind: from_text(3, &row.get::<_, String>(3)?)?,
        body: row.get(4)?,
        reply_to: row.get(5)?,
        status: from_text(6, &row.get::<_, String>(6)?)?,
        reason: row
            .get::<_, Option<String>>(7)?
            .map(|text| from_text(7, &text))
            .transpose()?,
        at: row.get(8)?,
    })
}

/// The word a contract enum has on the wire: the one spelling stored, so a new variant needs no
/// second table.
fn text_of(value: impl Serialize) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(text)) => text,
        other => unreachable!("a contract enum is a string on the wire: {other:?}"),
    }
}

/// The inverse of [`text_of`]; a stored word no variant has is an error, never a default.
fn from_text<T: DeserializeOwned>(column: usize, text: &str) -> rusqlite::Result<T> {
    serde_json::from_value(Value::String(text.to_owned())).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            Box::new(err),
        )
    })
}

fn actor_json(actor: &Actor) -> String {
    serde_json::to_string(actor).expect("an Actor is always serializable")
}

#[cfg(test)]
mod tests {
    use contracts::{Actor, ActorKind};

    use super::*;

    fn open(dir: &Path) -> Store {
        Store::open(&dir.join("messages.db")).unwrap()
    }

    fn put(store: &Store, status: MessageStatus, reason: Option<Reason>) -> u32 {
        let from = Actor {
            kind: ActorKind::Agent,
            id: "a".into(),
            parent: None,
        };
        store
            .insert(
                &from,
                "b",
                MessageKind::Question,
                "hi",
                None,
                status,
                reason,
                1,
            )
            .unwrap()
            .id
    }

    #[test]
    fn b8_every_reason_status_and_kind_survives_reopening() {
        let dir = tempfile::tempdir().unwrap();
        let reasons = [
            (MessageStatus::Held, Reason::AskFirst),
            (MessageStatus::Held, Reason::Takeover),
            (MessageStatus::Held, Reason::Escalated),
            (MessageStatus::Dropped, Reason::ReceiverGone),
            (MessageStatus::Dropped, Reason::NotAccepted),
        ];
        let ids: Vec<u32> = {
            let store = open(dir.path());
            reasons
                .iter()
                .map(|(status, reason)| put(&store, *status, Some(*reason)))
                .collect()
        };

        let store = open(dir.path());

        for (id, (status, reason)) in ids.into_iter().zip(reasons) {
            let message = store.get(id).unwrap().unwrap();
            assert_eq!(
                (message.status, message.reason, message.kind),
                (status, Some(reason), MessageKind::Question)
            );
        }
    }

    #[test]
    fn b8_a_stored_word_no_variant_has_is_an_error_not_a_default() {
        let dir = tempfile::tempdir().unwrap();
        let store = open(dir.path());
        let id = put(&store, MessageStatus::Held, Some(Reason::AskFirst));
        for column in ["reason", "status", "kind"] {
            store
                .db
                .execute(
                    &format!("UPDATE messages SET {column} = 'bogus' WHERE id = ?1"),
                    params![id],
                )
                .unwrap();

            assert!(store.get(id).is_err(), "{column}");
            store.db.execute("UPDATE messages SET reason = 'ask-first', status = 'held', kind = 'note' WHERE id = ?1", params![id]).unwrap();
        }
    }

    #[test]
    fn b5_a_stored_route_word_no_variant_has_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = open(dir.path());
        store
            .db
            .execute(
                "INSERT INTO routes (from_id, to_id, delivery) VALUES ('a', 'b', 'bogus')",
                [],
            )
            .unwrap();

        assert!(store.get_route("a", "b").is_err());
        assert!(store.list_routes().is_err());
    }
}
