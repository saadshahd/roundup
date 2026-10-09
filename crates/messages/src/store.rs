//! SQLite persistence for Messages and Routes. `<dir>/messages.db` (ADR 0004, WAL).

use std::collections::HashMap;
use std::path::Path;

use contracts::message::{Delivery, Message, MessageKind, MessageStatus, Reason, Route};
use contracts::{Actor, ActorKind};
use rpc::RpcError;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

pub(crate) type Binding = (i64, i64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ReceiverState {
    pub attempt: i64,
    pub revision: i64,
    pub closed_revision: i64,
    pub closed: bool,
}

/// One bubbling question (B13): who asked, what, the Doors still to try, and the hop now waiting.
pub(crate) struct Chain {
    pub id: i64,
    pub origin: Actor,
    pub body: String,
    pub rest: Vec<String>,
    pub current: u32,
    /// When the current hop's bound started (B14); `None` while the hop is `held`.
    pub armed_at: Option<i64>,
}

pub(crate) struct Store {
    db: Connection,
    /// Agents under a Takeover. Held in memory only (B6): never persisted, so a restarted
    /// Daemon has none.
    active_takeovers: HashMap<String, Binding>,
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
        let has_attempt = db
            .prepare("SELECT 1 FROM pragma_table_info('messages') WHERE name = 'receiver_attempt'")?
            .exists([])?;
        if !has_attempt {
            db.execute_batch(
                "ALTER TABLE messages ADD COLUMN receiver_attempt INTEGER NOT NULL DEFAULT 0;",
            )?;
        }
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS chains (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                origin TEXT NOT NULL,
                body TEXT NOT NULL,
                rest TEXT NOT NULL,
                current INTEGER NOT NULL,
                armed_at INTEGER,
                done INTEGER NOT NULL DEFAULT 0
            );",
        )?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS receiver_generations (id TEXT PRIMARY KEY, attempt INTEGER NOT NULL, closed INTEGER NOT NULL);")?;
        for (table, column, declaration) in [
            (
                "messages",
                "receiver_revision",
                "INTEGER NOT NULL DEFAULT 0",
            ),
            ("messages", "passed_from", "INTEGER"),
            (
                "receiver_generations",
                "revision",
                "INTEGER NOT NULL DEFAULT 0",
            ),
            (
                "receiver_generations",
                "closed_revision",
                "INTEGER NOT NULL DEFAULT -1",
            ),
        ] {
            let exists = db
                .prepare(&format!(
                    "SELECT 1 FROM pragma_table_info('{table}') WHERE name = ?"
                ))?
                .exists([column])?;
            if !exists {
                db.execute_batch(&format!(
                    "ALTER TABLE {table} ADD COLUMN {column} {declaration}"
                ))?;
            }
        }
        let mut store = Self {
            db,
            active_takeovers: HashMap::new(),
        };
        store.release_stale_takeovers()?;
        Ok(store)
    }

    pub(crate) fn receivers(&self) -> Result<Vec<String>, RpcError> {
        self.db.prepare("SELECT id FROM receiver_generations UNION SELECT to_id FROM messages WHERE to_id != 'you'").and_then(|mut q| q.query_map([], |r| r.get(0))?.collect()).map_err(RpcError::internal)
    }

    pub(crate) fn generation(&self, agent: &str) -> Result<Option<ReceiverState>, RpcError> {
        self.db.query_row("SELECT attempt, revision, closed_revision, closed FROM receiver_generations WHERE id=?", [agent], |r| Ok(ReceiverState { attempt:r.get(0)?, revision:r.get(1)?, closed_revision:r.get(2)?, closed:r.get(3)? })).optional().map_err(RpcError::internal)
    }

    pub(crate) fn set_generation(&self, agent: &str, state: ReceiverState) -> Result<(), RpcError> {
        self.db.execute("INSERT INTO receiver_generations (id,attempt,revision,closed_revision,closed) VALUES (?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET attempt=excluded.attempt,revision=excluded.revision,closed_revision=excluded.closed_revision,closed=excluded.closed", params![agent,state.attempt,state.revision,state.closed_revision,state.closed]).map_err(RpcError::internal)?;
        Ok(())
    }

    pub(crate) fn require_generation(&self, agent: &str, binding: Binding) -> Result<(), RpcError> {
        if self.generation(agent)?.is_some_and(|state| {
            state.attempt == binding.0 && !state.closed && binding.1 > state.closed_revision
        }) {
            return Ok(());
        }
        Err(RpcError::conflict(format!(
            "{agent}: stale or stopped Status revision"
        )))
    }

    pub(crate) fn bind(&self, id: u32, binding: Binding) -> Result<(), RpcError> {
        self.db
            .execute(
                "UPDATE messages SET receiver_attempt=?,receiver_revision=? WHERE id=?",
                params![binding.0, binding.1, id],
            )
            .map_err(RpcError::internal)?;
        Ok(())
    }

    pub(crate) fn bound(&self, id: u32) -> Result<Binding, RpcError> {
        self.db
            .query_row(
                "SELECT receiver_attempt,receiver_revision FROM messages WHERE id=?",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(RpcError::internal)
    }

    pub(crate) fn takeover_bound(&self, agent: &str) -> Option<Binding> {
        self.active_takeovers.get(agent).copied()
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

    /// Runs `f` as one SQLite transaction: its writes all land or none do, so a kill between two
    /// of them never leaves a Message and the chain that should have moved with it disagreeing
    /// (B8, B17).
    pub(crate) fn atomically<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, RpcError>,
    ) -> Result<T, RpcError> {
        self.db
            .execute_batch("BEGIN IMMEDIATE")
            .map_err(RpcError::internal)?;
        match f(self) {
            Ok(value) => {
                if let Err(err) = self.db.execute_batch("COMMIT") {
                    let _ = self.db.execute_batch("ROLLBACK");
                    return Err(RpcError::internal(err));
                }
                Ok(value)
            }
            Err(err) => {
                let _ = self.db.execute_batch("ROLLBACK");
                Err(err)
            }
        }
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
        binding: Binding,
        passed_from: Option<u32>,
    ) -> Result<Message, RpcError> {
        self.db
            .execute(
                "INSERT INTO messages (from_actor, to_id, kind, body, reply_to, status, reason, at, receiver_attempt, receiver_revision, passed_from, rank)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                     CASE WHEN ?6 = 'pending' THEN (SELECT COALESCE(MAX(rank), 0) + 1 FROM messages) ELSE 0 END)",
                params![
                    actor_json(from),
                    to,
                    text_of(kind),
                    body,
                    reply_to,
                    text_of(status),
                    reason.map(text_of),
                    at,
                    binding.0,
                    binding.1,
                    passed_from
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
                "SELECT id, from_actor, to_id, kind, body, reply_to, status, reason, at, passed_from
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
                "SELECT id, from_actor, to_id, kind, body, reply_to, status, reason, at, passed_from
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

    /// The `pending` Messages to `to` bound before `before`, in the order they became deliverable
    /// (B7): their `rank`, ties by id.
    pub(crate) fn pending_queue(
        &self,
        to: &str,
        before: Binding,
    ) -> Result<Vec<Message>, RpcError> {
        self.db
            .prepare(
                "SELECT id, from_actor, to_id, kind, body, reply_to, status, reason, at, passed_from
                 FROM messages WHERE to_id = ?1 AND status = 'pending' AND receiver_attempt = ?2 AND receiver_revision < ?3
                 ORDER BY rank ASC, id ASC",
            )
            .and_then(|mut stmt| {
                stmt.query_map(params![to, before.0, before.1], row_to_message)?
                    .collect()
            })
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
        let attempt = self.bound(id)?;
        if self.require_generation(&message.to, attempt).is_err() {
            return Ok(None);
        }
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

    /// B2: a refused `deliver` that will never take the Message (`not accepted`, `receiver
    /// gone`): `dropped` with `reason`, only if it is still `delivered`.
    pub(crate) fn drop_delivered(
        &self,
        id: u32,
        reason: Reason,
    ) -> Result<Option<Message>, RpcError> {
        let changed = self
            .db
            .execute(
                "UPDATE messages SET status = 'dropped', reason = ?2 WHERE id = ?1 AND status = 'delivered'",
                params![id, text_of(reason)],
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
                "SELECT id, from_actor, to_id, kind, body, reply_to, status, reason, at, passed_from
                 FROM messages WHERE to_id = ?1 AND status = 'pending' ORDER BY id",
            )
            .and_then(|mut stmt| stmt.query_map(params![to], row_to_message)?.collect())
            .map_err(RpcError::internal)
    }

    fn list_takeover_held_to(&self, to: &str) -> Result<Vec<Message>, RpcError> {
        self.db
            .prepare(
                "SELECT id, from_actor, to_id, kind, body, reply_to, status, reason, at, passed_from
                 FROM messages WHERE to_id = ?1 AND status = 'held' AND reason = 'takeover'
                 ORDER BY id",
            )
            .and_then(|mut stmt| stmt.query_map(params![to], row_to_message)?.collect())
            .map_err(RpcError::internal)
    }

    pub(crate) fn is_takeover_active(&self, agent: &str) -> bool {
        self.active_takeovers.contains_key(agent)
    }

    /// Begins a Takeover of `agent`: every Message to it that is `pending` and that `hold` (the
    /// pure `step`) names becomes `held` with the reason `takeover`, in id order (B6). `None` when a Takeover was
    /// already active (a repeated `begin` changes nothing); `Some` with the newly held Messages
    /// otherwise, even when that list is empty.
    pub(crate) fn begin_takeover(
        &mut self,
        agent: &str,
        hold: fn(&[Message]) -> Vec<u32>,
    ) -> Result<Option<Vec<Message>>, RpcError> {
        let binding = self
            .generation(agent)?
            .map_or((0, 0), |state| (state.attempt, state.revision));
        if self
            .active_takeovers
            .insert(agent.to_owned(), binding)
            .is_some()
        {
            return Ok(None);
        }
        let mut held = Vec::new();
        let pending = self.list_pending_to(agent)?;
        let to_hold = hold(&pending);
        for message in pending {
            if !to_hold.contains(&message.id) {
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
    /// in the order `order` (the pure `step`) gives (B6). `None` when no Takeover was active (a repeated `end` changes nothing);
    /// `Some` with the newly pending Messages otherwise, even when that list is empty.
    pub(crate) fn end_takeover(
        &mut self,
        agent: &str,
        order: fn(&[Message]) -> Vec<u32>,
    ) -> Result<Option<Vec<Message>>, RpcError> {
        if self.active_takeovers.remove(agent).is_none() {
            return Ok(None);
        }
        let mut pending = Vec::new();
        let held = self.list_takeover_held_to(agent)?;
        for id in order(&held) {
            let message = held.iter().find(|m| m.id == id).expect("named by `order`");
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

    /// Drops every `pending` or takeover-held Message to `to` that `gone` (the pure `step`) names,
    /// with `reason`, in id order (B9).
    pub(crate) fn drop_all_pending(
        &mut self,
        to: &str,
        reason: Reason,
        through: Binding,
        gone: fn(&[Message]) -> Vec<u32>,
    ) -> Result<Vec<Message>, RpcError> {
        let mut dropped = Vec::new();
        let mut open = Vec::new();
        for message in self
            .list_pending_to(to)?
            .into_iter()
            .chain(self.list_takeover_held_to(to)?)
        {
            if self.bound(message.id)? <= through {
                open.push(message);
            }
        }
        let to_drop = gone(&open);
        for message in open {
            if !to_drop.contains(&message.id) {
                continue;
            }
            let changed = self
                .db
                .execute(
                    "UPDATE messages SET status = 'dropped', reason = ?2
                     WHERE id = ?1 AND (status = 'pending' OR (status = 'held' AND reason = 'takeover'))",
                    params![message.id, text_of(reason)],
                )
                .map_err(RpcError::internal)?;
            if changed == 1 {
                dropped.push(self.get(message.id)?.expect("just updated"));
            }
        }
        Ok(dropped)
    }

    /// B13: the live question chain whose current hop is `hop`.
    pub(crate) fn chain_of(&self, hop: u32) -> Result<Option<Chain>, RpcError> {
        self.chains("WHERE done = 0 AND current = ?1", params![hop])
            .map(|mut chains| chains.pop())
    }

    /// B13, B14: every question chain still waiting on a hop, oldest first.
    pub(crate) fn live_chains(&self) -> Result<Vec<Chain>, RpcError> {
        self.chains("WHERE done = 0", params![])
    }

    fn chains(&self, filter: &str, args: impl rusqlite::Params) -> Result<Vec<Chain>, RpcError> {
        self.db
            .prepare(&format!(
                "SELECT id, origin, body, rest, current, armed_at FROM chains {filter} ORDER BY id"
            ))
            .and_then(|mut stmt| {
                stmt.query_map(args, |row| {
                    let origin: String = row.get(1)?;
                    let rest: String = row.get(3)?;
                    let bad = |err: serde_json::Error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            1,
                            rusqlite::types::Type::Text,
                            Box::new(err),
                        )
                    };
                    Ok(Chain {
                        id: row.get(0)?,
                        origin: serde_json::from_str(&origin).map_err(bad)?,
                        body: row.get(2)?,
                        rest: serde_json::from_str(&rest).map_err(bad)?,
                        current: row.get(4)?,
                        armed_at: row.get(5)?,
                    })
                })?
                .collect()
            })
            .map_err(RpcError::internal)
    }

    pub(crate) fn start_chain(
        &self,
        origin: &Actor,
        body: &str,
        rest: &[String],
        current: u32,
    ) -> Result<(), RpcError> {
        self.db
            .execute(
                "INSERT INTO chains (origin, body, rest, current) VALUES (?1, ?2, ?3, ?4)",
                params![
                    actor_json(origin),
                    body,
                    serde_json::to_string(rest).expect("strings serialize"),
                    current
                ],
            )
            .map_err(RpcError::internal)?;
        Ok(())
    }

    /// Points a chain at its next hop with the Doors still to come, or ends it (`current` of
    /// `None`).
    pub(crate) fn move_chain(
        &self,
        id: i64,
        current: Option<u32>,
        rest: &[String],
    ) -> Result<(), RpcError> {
        self.db
            .execute(
                "UPDATE chains SET current = COALESCE(?2, current), done = ?2 IS NULL, rest = ?3, armed_at = NULL
                 WHERE id = ?1",
                params![id, current, serde_json::to_string(rest).expect("strings serialize")],
            )
            .map_err(RpcError::internal)?;
        Ok(())
    }

    /// B14: a hop's bound runs from the moment it is `pending` or `delivered`. A `held` hop has
    /// none, and a hop that became `held` again (a Takeover began) starts over when released.
    pub(crate) fn arm_chains(&self, now: i64) -> Result<(), RpcError> {
        self.db
            .execute_batch(
                "UPDATE chains SET armed_at = NULL WHERE done = 0 AND armed_at IS NOT NULL
                     AND (SELECT status FROM messages WHERE id = current) NOT IN ('pending', 'delivered');",
            )
            .map_err(RpcError::internal)?;
        self.db
            .execute(
                "UPDATE chains SET armed_at = ?1 WHERE done = 0 AND armed_at IS NULL
                     AND (SELECT status FROM messages WHERE id = current) IN ('pending', 'delivered')",
                params![now],
            )
            .map_err(RpcError::internal)?;
        Ok(())
    }

    /// B13: a hop still `pending` becomes `dropped` with the reason `passed`; any other status
    /// is left as it is. `None` when the hop was not `pending`.
    pub(crate) fn drop_passed(&self, id: u32) -> Result<Option<Message>, RpcError> {
        let changed = self
            .db
            .execute(
                "UPDATE messages SET status = 'dropped', reason = 'passed'
                 WHERE id = ?1 AND status = 'pending'",
                params![id],
            )
            .map_err(RpcError::internal)?;
        if changed == 0 {
            return Ok(None);
        }
        self.get(id)
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
        passed_from: row.get(9)?,
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
                (1, 1),
                None,
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

        assert_eq!(store.pending_queue("b", (0, i64::MAX)).unwrap()[0].id, 1);
        assert_eq!(new, 2);
    }

    #[test]
    fn b6_begin_between_check_and_record_holds_the_message() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = open(dir.path());
        let id = put(&store, MessageStatus::Pending, None);
        assert_eq!(store.pending_queue("b", (1, i64::MAX)).unwrap()[0].id, id);

        store
            .begin_takeover("b", |pending| pending.iter().map(|m| m.id).collect())
            .unwrap();

        assert!(store.mark_delivered(id).unwrap().is_none());
        let message = store.get(id).unwrap().unwrap();
        assert_eq!(
            (message.status, message.reason),
            (MessageStatus::Held, Some(Reason::Takeover))
        );
    }
}
