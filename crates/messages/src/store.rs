//! SQLite persistence for Messages and Routes. `<dir>/messages.db` (ADR 0004, WAL).

use std::collections::HashSet;
use std::path::Path;

use contracts::message::{Delivery, Message, MessageKind, MessageStatus, Reason, Route};
use contracts::{Actor, ActorKind};
use rpc::RpcError;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

pub(crate) struct Store {
    db: Connection,
    /// Agents under a Takeover. Held in memory only (B6): never persisted, so a restarted
    /// Daemon has none.
    active_takeovers: HashSet<String>,
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
                at INTEGER NOT NULL,
                rank INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS routes (
                from_id TEXT NOT NULL,
                to_id TEXT NOT NULL,
                delivery TEXT NOT NULL,
                PRIMARY KEY (from_id, to_id)
            );",
        )?;
        let has_rank = db
            .prepare("SELECT 1 FROM pragma_table_info('messages') WHERE name = 'rank'")?
            .exists([])?;
        if !has_rank {
            db.execute_batch("ALTER TABLE messages ADD COLUMN rank INTEGER NOT NULL DEFAULT 0;")?;
        }
        let mut store = Self {
            db,
            active_takeovers: HashSet::new(),
        };
        store.release_stale_takeovers()?;
        Ok(store)
    }

    /// B6: no Takeover survives a restart, so a Message left `held` for the reason `takeover` by
    /// a previous run becomes `pending`, in id order, before anything else uses this Store.
    fn release_stale_takeovers(&mut self) -> rusqlite::Result<()> {
        let ids: Vec<u32> = self
            .db
            .prepare(
                "SELECT id FROM messages WHERE status = 'held' AND reason = 'takeover' ORDER BY id",
            )?
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        for id in ids {
            self.db.execute(
                "UPDATE messages SET status = 'pending', reason = NULL,
                     rank = (SELECT COALESCE(MAX(rank), 0) + 1 FROM messages)
                 WHERE id = ?1",
                params![id],
            )?;
        }
        Ok(())
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
                "INSERT INTO messages (from_actor, to_id, kind, body, reply_to, status, reason, at, rank)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                     CASE WHEN ?6 = 'pending' THEN (SELECT COALESCE(MAX(rank), 0) + 1 FROM messages) ELSE 0 END)",
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
                "UPDATE messages SET status = ?2, reason = ?3,
                     rank = CASE WHEN ?2 = 'pending' THEN (SELECT COALESCE(MAX(rank), 0) + 1 FROM messages) ELSE rank END
                 WHERE id = ?1 AND status = 'held'",
                params![id, text_of(status), reason.map(text_of)],
            )
            .map_err(RpcError::internal)?;
        Ok(changed == 1)
    }

    /// The oldest `pending` Message to `to`, by the order it became deliverable (B7): its `rank`,
    /// ties by id.
    pub(crate) fn next_pending(&self, to: &str) -> Result<Option<Message>, RpcError> {
        self.db
            .query_row(
                "SELECT id, from_actor, to_id, kind, body, reply_to, status, reason, at
                 FROM messages WHERE to_id = ?1 AND status = 'pending'
                 ORDER BY rank ASC, id ASC LIMIT 1",
                [to],
                row_to_message,
            )
            .optional()
            .map_err(RpcError::internal)
    }

    /// Records `id` `delivered`, only if it is still `pending` (B2's one conditional write).
    /// `None` when it was not `pending` any more.
    pub(crate) fn mark_delivered(&self, id: u32) -> Result<Option<Message>, RpcError> {
        let changed = self
            .db
            .execute(
                "UPDATE messages SET status = 'delivered', reason = NULL
                 WHERE id = ?1 AND status = 'pending'",
                params![id],
            )
            .map_err(RpcError::internal)?;
        if changed == 0 {
            return Ok(None);
        }
        self.get(id)
    }

    /// Reverses `mark_delivered` after a refused `deliver` (B2), only if `id` is still
    /// `delivered`: it goes back to `pending`, or to `held` for `takeover` when a Takeover of its
    /// receiver began while it was being typed and its sender is not the user (B6), so nothing is
    /// typed during the Takeover. `None` when it was not `delivered` any more.
    pub(crate) fn unmark_delivered(&self, id: u32) -> Result<Option<Message>, RpcError> {
        let Some(message) = self.get(id)? else {
            return Ok(None);
        };
        let held = message.from.kind != ActorKind::User && self.is_takeover_active(&message.to);
        let (status, reason) = if held {
            ("held", Some("takeover"))
        } else {
            ("pending", None)
        };
        let changed = self
            .db
            .execute(
                "UPDATE messages SET status = ?2, reason = ?3 WHERE id = ?1 AND status = 'delivered'",
                params![id, status, reason],
            )
            .map_err(RpcError::internal)?;
        if changed == 0 {
            return Ok(None);
        }
        self.get(id)
    }

    fn list_pending_to(&self, to: &str) -> Result<Vec<Message>, RpcError> {
        self.db
            .prepare(
                "SELECT id, from_actor, to_id, kind, body, reply_to, status, reason, at
                 FROM messages WHERE to_id = ?1 AND status = 'pending' ORDER BY id",
            )
            .and_then(|mut stmt| stmt.query_map(params![to], row_to_message)?.collect())
            .map_err(RpcError::internal)
    }

    fn list_takeover_held_to(&self, to: &str) -> Result<Vec<Message>, RpcError> {
        self.db
            .prepare(
                "SELECT id, from_actor, to_id, kind, body, reply_to, status, reason, at
                 FROM messages WHERE to_id = ?1 AND status = 'held' AND reason = 'takeover'
                 ORDER BY id",
            )
            .and_then(|mut stmt| stmt.query_map(params![to], row_to_message)?.collect())
            .map_err(RpcError::internal)
    }

    pub(crate) fn is_takeover_active(&self, agent: &str) -> bool {
        self.active_takeovers.contains(agent)
    }

    /// Begins a Takeover of `agent`: every Message to it that is `pending` and not from the user
    /// becomes `held` with the reason `takeover`, in id order (B6). `None` when a Takeover was
    /// already active (a repeated `begin` changes nothing); `Some` with the newly held Messages
    /// otherwise, even when that list is empty.
    pub(crate) fn begin_takeover(&mut self, agent: &str) -> Result<Option<Vec<Message>>, RpcError> {
        if !self.active_takeovers.insert(agent.to_owned()) {
            return Ok(None);
        }
        let mut held = Vec::new();
        for message in self.list_pending_to(agent)? {
            if message.from.kind == ActorKind::User {
                continue;
            }
            let changed = self
                .db
                .execute(
                    "UPDATE messages SET status = 'held', reason = 'takeover'
                     WHERE id = ?1 AND status = 'pending'",
                    params![message.id],
                )
                .map_err(RpcError::internal)?;
            if changed == 1 {
                held.push(self.get(message.id)?.expect("just updated"));
            }
        }
        Ok(Some(held))
    }

    /// Ends a Takeover of `agent`: every Message held for the reason `takeover` becomes `pending`
    /// in id order (B6). `None` when no Takeover was active (a repeated `end` changes nothing);
    /// `Some` with the newly pending Messages otherwise, even when that list is empty.
    pub(crate) fn end_takeover(&mut self, agent: &str) -> Result<Option<Vec<Message>>, RpcError> {
        if !self.active_takeovers.remove(agent) {
            return Ok(None);
        }
        let mut pending = Vec::new();
        for message in self.list_takeover_held_to(agent)? {
            let changed = self
                .db
                .execute(
                    "UPDATE messages SET status = 'pending', reason = NULL,
                         rank = (SELECT COALESCE(MAX(rank), 0) + 1 FROM messages)
                     WHERE id = ?1 AND status = 'held'",
                    params![message.id],
                )
                .map_err(RpcError::internal)?;
            if changed == 1 {
                pending.push(self.get(message.id)?.expect("just updated"));
            }
        }
        Ok(Some(pending))
    }

    /// Drops every `pending` Message to `to` with `reason`, in id order (B9).
    pub(crate) fn drop_all_pending(
        &mut self,
        to: &str,
        reason: Reason,
    ) -> Result<Vec<Message>, RpcError> {
        let mut dropped = Vec::new();
        for message in self.list_pending_to(to)? {
            let changed = self
                .db
                .execute(
                    "UPDATE messages SET status = 'dropped', reason = ?2
                     WHERE id = ?1 AND status = 'pending'",
                    params![message.id, text_of(reason)],
                )
                .map_err(RpcError::internal)?;
            if changed == 1 {
                dropped.push(self.get(message.id)?.expect("just updated"));
            }
        }
        Ok(dropped)
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
        // `takeover` does not round-trip unchanged: B6 says no Takeover survives a restart, so a
        // Message held for it becomes `pending` on reopen (`b6_a_held_for_takeover_message_becomes_pending_on_reopen`).
        let dir = tempfile::tempdir().unwrap();
        let reasons = [
            (MessageStatus::Held, Reason::AskFirst),
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
    fn b6_a_held_for_takeover_message_becomes_pending_on_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let id = {
            let store = open(dir.path());
            put(&store, MessageStatus::Held, Some(Reason::Takeover))
        };

        let store = open(dir.path());

        let message = store.get(id).unwrap().unwrap();
        assert_eq!(
            (message.status, message.reason),
            (MessageStatus::Pending, None)
        );
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

    #[test]
    fn b8_a_store_written_before_rank_opens_and_takes_a_send() {
        let dir = tempfile::tempdir().unwrap();
        {
            let db = Connection::open(dir.path().join("messages.db")).unwrap();
            db.execute_batch(
                "CREATE TABLE messages (
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
                INSERT INTO messages (from_actor, to_id, kind, body, status, at)
                VALUES ('{\"kind\":\"agent\",\"id\":\"a\",\"parent\":null}', 'b', 'note', 'old', 'pending', 1);",
            )
            .unwrap();
        }

        let store = open(dir.path());
        let new = put(&store, MessageStatus::Pending, None);

        assert_eq!(store.next_pending("b").unwrap().unwrap().id, 1);
        assert_eq!(new, 2);
    }

    #[test]
    fn b6_begin_between_check_and_record_holds_the_message() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = open(dir.path());
        let id = put(&store, MessageStatus::Pending, None);
        assert_eq!(store.next_pending("b").unwrap().unwrap().id, id);

        store.begin_takeover("b").unwrap();

        assert!(store.mark_delivered(id).unwrap().is_none());
        let message = store.get(id).unwrap().unwrap();
        assert_eq!(
            (message.status, message.reason),
            (MessageStatus::Held, Some(Reason::Takeover))
        );
    }
}
