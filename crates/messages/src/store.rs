//! SQLite persistence for Messages and Routes. `<dir>/messages.db` (ADR 0004, WAL).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use contracts::Actor;
use contracts::message::{Delivery, Message, MessageKind, MessageStatus, Reason, Route};
use rpc::{OpenError, RpcError};

use crate::wake::Wake;
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

pub(crate) enum FirstPrompt {
    Absent,
    Reserved(String),
    Recorded { body: String, id: u32 },
    Published { body: String, id: u32 },
}

impl FirstPrompt {
    pub(crate) fn body(&self) -> Option<&str> {
        match self {
            Self::Absent => None,
            Self::Reserved(body) | Self::Recorded { body, .. } | Self::Published { body, .. } => {
                Some(body)
            }
        }
    }

    pub(crate) fn note(&self) -> Option<u32> {
        match self {
            Self::Absent | Self::Reserved(_) => None,
            Self::Recorded { id, .. } | Self::Published { id, .. } => Some(*id),
        }
    }
}

pub(crate) struct Restart {
    pub before: i64,
    pub prompt: FirstPrompt,
    pub released: Option<i64>,
}

pub(crate) struct Store {
    db: Connection,
    /// Agents under a Takeover. Held in memory only (B6): never persisted, so a restarted
    /// Daemon has none.
    active_takeovers: HashMap<String, Binding>,
    /// Agents whose Terminal holds the user's unsent input (B32). Memory only, like a Takeover.
    unsent_input: HashSet<String>,
}

impl Store {
    pub(crate) fn open(path: &Path) -> Result<Self, OpenError> {
        let mut db = Connection::open(path)?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        crate::schema::open(&mut db, path)?;
        let mut store = Self {
            db,
            active_takeovers: HashMap::new(),
            unsent_input: HashSet::new(),
        };
        store.release_stale_takeovers()?;
        Ok(store)
    }

    pub(crate) fn is_reserved(&self, agent: &str) -> Result<bool, RpcError> {
        self.db
            .prepare("SELECT 1 FROM message_restarts WHERE agent=? AND released IS NULL")
            .and_then(|mut q| q.exists([agent]))
            .map_err(RpcError::internal)
    }

    pub(crate) fn is_protected(&self, agent: &str) -> Result<bool, RpcError> {
        self.db
            .prepare("SELECT 1 FROM message_protection WHERE agent=?")
            .and_then(|mut q| q.exists([agent]))
            .map_err(RpcError::internal)
    }

    pub(crate) fn retirement(&self, agent: &str) -> Result<Option<(String, bool)>, RpcError> {
        self.db
            .query_row(
                "SELECT key,completed FROM message_retirements WHERE agent=?",
                [agent],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(RpcError::internal)
    }

    pub(crate) fn reserve_retirement(&self, agent: &str, key: &str) -> Result<(), RpcError> {
        if self.is_protected(agent)? {
            return Err(RpcError::conflict(
                "another operation reserved this receiver",
            ));
        }
        self.db
            .execute(
                "INSERT INTO message_retirements(agent,key) VALUES (?,?)",
                params![agent, key],
            )
            .map_err(RpcError::internal)?;
        Ok(())
    }

    pub(crate) fn complete_retirement(&self, agent: &str, key: &str) -> Result<(), RpcError> {
        self.db
            .execute(
                "UPDATE message_retirements SET completed=1 WHERE agent=? AND key=?",
                params![agent, key],
            )
            .map_err(RpcError::internal)?;
        Ok(())
    }

    pub(crate) fn restart(&self, agent: &str, key: &str) -> Result<Option<Restart>, RpcError> {
        self.db.query_row("SELECT before,prompt,note,released,published FROM message_restarts WHERE agent=? AND key=?", params![agent,key], |r| {
            let prompt: Option<String> = r.get(1)?;
            let note: Option<u32> = r.get(2)?;
            let published: bool = r.get(4)?;
            let prompt = match (prompt,note,published) {
                (None,None,false) => FirstPrompt::Absent,
                (Some(body),None,false) => FirstPrompt::Reserved(body),
                (Some(body),Some(id),false) => FirstPrompt::Recorded { body,id },
                (Some(body),Some(id),true) => FirstPrompt::Published { body,id },
                _ => return Err(rusqlite::Error::InvalidQuery),
            };
            Ok(Restart { before:r.get(0)?, prompt, released:r.get(3)? })
        }).optional().map_err(RpcError::internal)
    }

    pub(crate) fn reserve_restart(
        &self,
        agent: &str,
        key: &str,
        before: i64,
        prompt: Option<&str>,
    ) -> Result<(), RpcError> {
        if self.is_protected(agent)? {
            return Err(RpcError::conflict("another restart reserved this receiver"));
        }
        self.db
            .execute(
                "INSERT INTO message_restarts(agent,key,before,prompt) VALUES (?,?,?,?)",
                params![agent, key, before, prompt],
            )
            .map_err(RpcError::internal)?;
        Ok(())
    }

    pub(crate) fn rebind_takeover(&mut self, agent: &str, at: Binding) {
        if let Some(bound) = self.active_takeovers.get_mut(agent) {
            *bound = at;
        }
    }

    pub(crate) fn end_restart(&self, agent: &str) -> Result<(), RpcError> {
        self.db
            .execute(
                "UPDATE message_restarts SET released=-1 WHERE agent=? AND released IS NULL",
                [agent],
            )
            .map_err(RpcError::internal)?;
        Ok(())
    }

    pub(crate) fn record_restart_note(
        &self,
        agent: &str,
        key: &str,
        id: u32,
    ) -> Result<(), RpcError> {
        self.db
            .execute(
                "UPDATE message_restarts SET note=? WHERE agent=? AND key=?",
                params![id, agent, key],
            )
            .map_err(RpcError::internal)?;
        Ok(())
    }

    pub(crate) fn publish_restart_note(&self, agent: &str, key: &str) -> Result<(), RpcError> {
        self.db
            .execute(
                "UPDATE message_restarts SET published=1 WHERE agent=? AND key=?",
                params![agent, key],
            )
            .map_err(RpcError::internal)?;
        Ok(())
    }

    pub(crate) fn release_restart(
        &mut self,
        agent: &str,
        key: &str,
        at: Binding,
        now: i64,
    ) -> Result<(), RpcError> {
        self.db.execute("UPDATE messages SET receiver_attempt=?,receiver_revision=? WHERE to_id=? AND status IN ('pending','held')", params![at.0,at.1,agent]).map_err(RpcError::internal)?;
        self.set_generation(
            agent,
            ReceiverState {
                attempt: at.0,
                revision: at.1,
                closed_revision: -1,
                closed: false,
            },
        )?;
        // B6's memory-only Terminal holds vanish on reopen, but a durable reservation keeps
        // their Messages intact until recovery explicitly releases the queue.
        if !self.holds_terminal(agent) {
            self.release_takeover_held(agent, super::release_order)?;
        }
        self.db
            .execute(
                "UPDATE message_restarts SET released=? WHERE agent=? AND key=?",
                params![at.0, agent, key],
            )
            .map_err(RpcError::internal)?;
        self.arm_chains(now)
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
                "SELECT id FROM messages WHERE status = 'held' AND reason = 'takeover' AND to_id NOT IN (SELECT agent FROM message_protection) ORDER BY id",
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
        // SQLite rollback must also restore the in-memory Takeover and input holds.
        let takeovers = self.active_takeovers.clone();
        let input = self.unsent_input.clone();
        // A panic in `f` rolls back too, so the next transaction can begin (B28).
        let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(self)));
        let ran = ran.unwrap_or_else(|panic| {
            let _ = self.db.execute_batch("ROLLBACK");
            self.active_takeovers = takeovers.clone();
            self.unsent_input = input.clone();
            std::panic::resume_unwind(panic)
        });
        match ran {
            Ok(value) => {
                if let Err(err) = self.db.execute_batch("COMMIT") {
                    let _ = self.db.execute_batch("ROLLBACK");
                    self.active_takeovers = takeovers;
                    self.unsent_input = input;
                    return Err(RpcError::internal(err));
                }
                Ok(value)
            }
            Err(err) => {
                let _ = self.db.execute_batch("ROLLBACK");
                self.active_takeovers = takeovers;
                self.unsent_input = input;
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

    /// A32: every Message sent to or by `door`, each with the Iteration it belongs to: one more
    /// than the number of ended Iterations whose last Message came before it.
    pub(crate) fn thread(&self, door: &str) -> Result<Vec<Message>, RpcError> {
        let ended: Vec<u32> = self
            .db
            .prepare("SELECT upto FROM iterations WHERE door = ? ORDER BY iteration")
            .and_then(|mut q| q.query_map([door], |r| r.get(0))?.collect())
            .map_err(RpcError::internal)?;
        let mut thread = self.list()?;
        thread.retain(|m| m.to == door || m.from.id == door);
        for message in &mut thread {
            let before = ended.iter().filter(|upto| **upto < message.id).count();
            message.iteration = Some(before as u32 + 1);
        }
        Ok(thread)
    }

    /// A32: the Iteration `door` is in ends with the Messages stored so far.
    pub(crate) fn end_iteration(&self, door: &str) -> Result<u32, RpcError> {
        let ended: u32 = self
            .db
            .query_row(
                "SELECT COUNT(*) FROM iterations WHERE door = ?",
                [door],
                |r| r.get(0),
            )
            .map_err(RpcError::internal)?;
        self.db
            .execute(
                "INSERT INTO iterations (door, iteration, upto)
                 VALUES (?1, ?2, COALESCE((SELECT MAX(id) FROM messages), 0))",
                params![door, ended + 1],
            )
            .map_err(RpcError::internal)?;
        Ok(ended + 1)
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

    /// B39: saves the in-flight record of `id` (prompt `prompt`), only if it is still `pending`
    /// (B2's one conditional write). `None` when it was not `pending` any more.
    pub(crate) fn record_flight(&self, id: u32, prompt: u32) -> Result<Option<Message>, RpcError> {
        let changed = self
            .db
            .execute(
                "INSERT OR REPLACE INTO message_flight(message_id, prompt)
                 SELECT id, ?2 FROM messages WHERE id = ?1 AND status = 'pending'",
                params![id, prompt],
            )
            .map_err(RpcError::internal)?;
        if changed == 0 {
            return Ok(None);
        }
        self.get(id)
    }

    /// B30, B39: saves the in-flight record of every `id` in one transaction, only if all are
    /// still `pending`; `None` and no change otherwise. The prompt is named by its first id.
    pub(crate) fn record_flight_all(
        &mut self,
        ids: &[u32],
    ) -> Result<Option<Vec<Message>>, RpcError> {
        let mut partial = false;
        let prompt = ids.first().copied().unwrap_or_default();
        let marked = self.atomically(|store| {
            let mut marked = Vec::new();
            for &id in ids {
                match store.record_flight(id, prompt)? {
                    Some(message) => marked.push(message),
                    None => {
                        // The error rolls back the ones already recorded.
                        partial = true;
                        return Err(RpcError::internal("a Message is no longer pending"));
                    }
                }
            }
            Ok(marked)
        });
        match marked {
            Ok(marked) => Ok(Some(marked)),
            Err(_) if partial => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// Whether `id` was typed and not yet confirmed (B39).
    pub(crate) fn in_flight(&self, id: u32) -> Result<bool, RpcError> {
        self.db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM message_flight WHERE message_id = ?1)",
                params![id],
                |row| row.get(0),
            )
            .map_err(RpcError::internal)
    }

    /// B8: forgets the in-flight records of every Message to `to`; they are ordinary `pending`.
    pub(crate) fn clear_flights_to(&self, to: &str) -> Result<(), RpcError> {
        self.db
            .execute(
                "DELETE FROM message_flight
                 WHERE message_id IN (SELECT id FROM messages WHERE to_id = ?1)",
                params![to],
            )
            .map(drop)
            .map_err(RpcError::internal)
    }

    /// B2: a refused `deliver` that typed nothing (`Busy`): the record is cleared and the Message
    /// stays as it is, `pending`, or `held` for `takeover` when one began while it was typed.
    pub(crate) fn clear_flight(&self, id: u32) -> Result<(), RpcError> {
        self.db
            .execute(
                "DELETE FROM message_flight WHERE message_id = ?1",
                params![id],
            )
            .map(drop)
            .map_err(RpcError::internal)
    }

    /// B2: a refused `deliver` that will never take the Message (`not accepted`, `receiver
    /// gone`): `dropped` with `reason`, only if it is still in flight.
    pub(crate) fn drop_in_flight(
        &self,
        id: u32,
        reason: Reason,
    ) -> Result<Option<Message>, RpcError> {
        let changed = self
            .db
            .execute(
                "UPDATE messages SET status = 'dropped', reason = ?2
                 WHERE id = ?1 AND status IN ('pending', 'held')
                   AND EXISTS(SELECT 1 FROM message_flight WHERE message_id = ?1)",
                params![id, text_of(reason)],
            )
            .map_err(RpcError::internal)?;
        self.clear_flight(id)?;
        if changed == 0 {
            return Ok(None);
        }
        self.get(id)
    }

    /// B39: the Signal for a prompt naming `ids` arrived for Agent `agent`. Records `delivered`,
    /// in one write, each of them that is in flight (`pending`, or `held` by a Takeover that began
    /// while it was typed). The answer is the Messages it confirmed, and whether the prompt only
    /// repeats Messages that were `delivered` before (it names at least one, and every one).
    pub(crate) fn confirm_flight(
        &mut self,
        agent: &str,
        ids: &[u32],
    ) -> Result<(Vec<Message>, bool), RpcError> {
        self.atomically(|store| {
            let mut confirmed = Vec::new();
            let mut repeated = !ids.is_empty();
            for &id in ids {
                let message = store.get(id)?.filter(|m| m.to == agent);
                let Some(message) = message else {
                    repeated = false;
                    continue;
                };
                if message.status == MessageStatus::Delivered {
                    continue;
                }
                repeated = false;
                if !store.in_flight(id)? {
                    continue;
                }
                let changed = store
                    .db
                    .execute(
                        "UPDATE messages SET status = 'delivered', reason = NULL
                         WHERE id = ?1 AND status IN ('pending', 'held')",
                        params![id],
                    )
                    .map_err(RpcError::internal)?;
                store.clear_flight(id)?;
                if changed == 1 {
                    confirmed.push(store.get(id)?.expect("just updated"));
                }
            }
            Ok((confirmed, repeated))
        })
    }

    /// B51: a `pending` Message not yet written to the inbox becomes `held` for `inbox`. `None`
    /// when it is not `pending` or is in flight.
    pub(crate) fn hold_for_inbox(&self, id: u32) -> Result<Option<Message>, RpcError> {
        let changed = self
            .db
            .execute(
                "UPDATE messages SET status = 'held', reason = 'inbox'
                 WHERE id = ?1 AND status = 'pending'
                   AND NOT EXISTS(SELECT 1 FROM message_flight WHERE message_id = ?1)",
                params![id],
            )
            .map_err(RpcError::internal)?;
        if changed == 0 {
            return Ok(None);
        }
        self.get(id)
    }

    /// B51(d): every Message held for `inbox` to `agent` becomes `pending`, in id order, as one
    /// write.
    pub(crate) fn release_inbox_held(&mut self, agent: &str) -> Result<Vec<Message>, RpcError> {
        self.atomically(|store| {
            let ids: Vec<u32> = store
                .db
                .prepare(
                    "SELECT id FROM messages
                     WHERE to_id = ?1 AND status = 'held' AND reason = 'inbox' ORDER BY id",
                )
                .and_then(|mut q| q.query_map(params![agent], |r| r.get(0))?.collect())
                .map_err(RpcError::internal)?;
            let mut released = Vec::new();
            for id in ids {
                if store.release_held(id, MessageStatus::Pending, None)? {
                    released.push(store.get(id)?.expect("just updated"));
                }
            }
            Ok(released)
        })
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

    /// B6, B32: whether the user holds `agent`'s Terminal, by a Takeover or by unsent input.
    pub(crate) fn holds_terminal(&self, agent: &str) -> bool {
        self.is_takeover_active(agent) || self.unsent_input.contains(agent)
    }

    /// B32: the user has unsent input in `agent`'s Terminal: every Message that `hold` names
    /// among those `pending` becomes `held` for `takeover`. `None` when input was already unsent.
    pub(crate) fn begin_input(
        &mut self,
        agent: &str,
        hold: fn(&[Message]) -> Vec<u32>,
    ) -> Result<Option<Vec<Message>>, RpcError> {
        if !self.unsent_input.insert(agent.to_owned()) {
            return Ok(None);
        }
        self.hold_pending(agent, hold).map(Some)
    }

    /// B32: the unsent input of `agent`'s Terminal was submitted or cleared. What was held
    /// becomes `pending` as `order` gives, unless a Takeover still holds the Terminal. `None`
    /// when no input was unsent.
    pub(crate) fn end_input(
        &mut self,
        agent: &str,
        order: fn(&[Message]) -> Vec<u32>,
    ) -> Result<Option<Vec<Message>>, RpcError> {
        if !self.unsent_input.remove(agent) {
            return Ok(None);
        }
        if self.is_takeover_active(agent) {
            return Ok(Some(Vec::new()));
        }
        self.release_takeover_held(agent, order).map(Some)
    }

    /// The Agent is gone or restarted: no input of it is unsent any more.
    pub(crate) fn forget_input(&mut self, agent: &str) {
        self.unsent_input.remove(agent);
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
        self.hold_pending(agent, hold).map(Some)
    }

    fn hold_pending(
        &mut self,
        agent: &str,
        hold: fn(&[Message]) -> Vec<u32>,
    ) -> Result<Vec<Message>, RpcError> {
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
        Ok(held)
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
        if self.unsent_input.contains(agent) || self.retirement(agent)?.is_some() {
            return Ok(Some(Vec::new()));
        }
        self.release_takeover_held(agent, order).map(Some)
    }

    fn release_takeover_held(
        &mut self,
        agent: &str,
        order: fn(&[Message]) -> Vec<u32>,
    ) -> Result<Vec<Message>, RpcError> {
        let mut pending = Vec::new();
        if self.retirement(agent)?.is_some() {
            return Ok(pending);
        }
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
        Ok(pending)
    }

    /// Drops every `pending` or takeover-held Message to `to` that `gone` (the pure `step`) names,
    /// with `reason`, in id order (B9). With `keep`, a Message in flight (B39) is not dropped: it
    /// waits for the Agent to be resumed, and moves to `rebind`, the new Attempt, once there is one;
    /// so does a note from the Daemon at a restart (`notes_wait`).
    pub(crate) fn drop_all_pending(
        &mut self,
        to: &str,
        reason: Reason,
        through: Binding,
        gone: fn(&[Message]) -> Vec<u32>,
        keep: Option<Option<Binding>>,
        notes_wait: bool,
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
            // B46: a note from the Daemon (B47) waits for the resumed Agent like one in flight.
            let note =
                notes_wait && message.kind == MessageKind::Note && message.from == Actor::daemon();
            if let Some(rebind) = keep
                && (note || self.in_flight(message.id)?)
            {
                if let Some(at) = rebind {
                    self.bind(message.id, at)?;
                    self.clear_flight(message.id)?;
                }
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
                     AND ((SELECT status FROM messages WHERE id = current) NOT IN ('pending', 'delivered') OR (SELECT to_id FROM messages WHERE id=current) IN (SELECT agent FROM message_protection));",
            )
            .map_err(RpcError::internal)?;
        self.db
            .execute(
                "UPDATE chains SET armed_at = ?1 WHERE done = 0 AND armed_at IS NULL
                     AND (SELECT status FROM messages WHERE id = current) IN ('pending', 'delivered') AND (SELECT to_id FROM messages WHERE id=current) NOT IN (SELECT agent FROM message_protection)",
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

    pub(crate) fn count_wakes(&self, waiter: &str) -> Result<u32, RpcError> {
        self.db
            .query_row(
                "SELECT COUNT(*) FROM wakes WHERE waiter = ?1",
                params![waiter],
                |row| row.get(0),
            )
            .map_err(RpcError::internal)
    }

    pub(crate) fn insert_wake(
        &self,
        waiter: &str,
        ids: &[String],
        any_of: bool,
        finished: &[String],
        seen: u64,
    ) -> Result<Wake, RpcError> {
        self.db
            .execute(
                "INSERT INTO wakes (waiter, any_of, ids, finished, seen) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![waiter, any_of, list_text(ids), list_text(finished), seen as i64],
            )
            .map_err(RpcError::internal)?;
        let id = u32::try_from(self.db.last_insert_rowid()).map_err(RpcError::internal)?;
        self.wake(id)?
            .ok_or_else(|| RpcError::internal("wake request vanished right after it was saved"))
    }

    pub(crate) fn wakes(&self) -> Result<Vec<Wake>, RpcError> {
        self.db
            .prepare("SELECT id, waiter, any_of, ids, finished, seen FROM wakes ORDER BY id")
            .and_then(|mut stmt| stmt.query_map([], row_to_wake)?.collect())
            .map_err(RpcError::internal)
    }

    pub(crate) fn wake(&self, id: u32) -> Result<Option<Wake>, RpcError> {
        self.db
            .query_row(
                "SELECT id, waiter, any_of, ids, finished, seen FROM wakes WHERE id = ?1",
                params![id],
                row_to_wake,
            )
            .optional()
            .map_err(RpcError::internal)
    }

    pub(crate) fn update_wake(
        &self,
        id: u32,
        finished: &[String],
        seen: u64,
    ) -> Result<(), RpcError> {
        self.db
            .execute(
                "UPDATE wakes SET finished = ?2, seen = ?3 WHERE id = ?1",
                params![id, list_text(finished), seen as i64],
            )
            .map(drop)
            .map_err(RpcError::internal)
    }

    /// Whether the request was open.
    pub(crate) fn delete_wake(&self, id: u32) -> Result<bool, RpcError> {
        self.db
            .execute("DELETE FROM wakes WHERE id = ?1", params![id])
            .map(|changed| changed > 0)
            .map_err(RpcError::internal)
    }

    pub(crate) fn delete_wakes_of(&self, waiter: &str) -> Result<(), RpcError> {
        self.db
            .execute("DELETE FROM wakes WHERE waiter = ?1", params![waiter])
            .map(drop)
            .map_err(RpcError::internal)
    }

    /// B1: the Messages to `to` that are `pending` or `held`, pushed digests (B22) and Wake notes
    /// (B42) left out.
    pub(crate) fn count_open(&self, to: &str) -> Result<u32, RpcError> {
        self.db
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM messages WHERE to_id = ?1 AND status IN ('pending', 'held') AND NOT ({DIGEST}) AND NOT (from_actor = ?2 AND substr(body, 1, length(?4)) = ?4)"
                ),
                params![
                    to,
                    actor_json(&Actor::daemon()),
                    crate::pushes::PREFIX,
                    crate::wake::PREFIX
                ],
                |row| row.get(0),
            )
            .map_err(RpcError::internal)
    }

    /// B22: the pushed digests to `to` that are `pending` or `held`, oldest first.
    pub(crate) fn open_digests(&self, to: &str) -> Result<Vec<Message>, RpcError> {
        self.db
            .prepare(&format!(
                "SELECT id, from_actor, to_id, kind, body, reply_to, status, reason, at, passed_from
                 FROM messages WHERE to_id = ?1 AND status IN ('pending', 'held') AND ({DIGEST})
                 ORDER BY id"
            ))
            .and_then(|mut stmt| {
                stmt.query_map(
                    params![to, actor_json(&Actor::daemon()), crate::pushes::PREFIX],
                    row_to_message,
                )?
                .collect()
            })
            .map_err(RpcError::internal)
    }

    /// B22: `id` folded into a newer digest: `dropped` with the reason `merged`, only while it is
    /// still `pending` or `held`. Whether it was.
    pub(crate) fn drop_merged(&self, id: u32) -> Result<bool, RpcError> {
        let changed = self
            .db
            .execute(
                "UPDATE messages SET status = 'dropped', reason = 'merged'
                 WHERE id = ?1 AND status IN ('pending', 'held')",
                params![id],
            )
            .map_err(RpcError::internal)?;
        Ok(changed == 1)
    }

    pub(crate) fn set_route(
        &self,
        from: &str,
        to: &str,
        delivery: Delivery,
        set_by: &str,
    ) -> Result<(), RpcError> {
        self.db
            .execute(
                "INSERT INTO routes (from_id, to_id, delivery, set_by) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(from_id, to_id) DO UPDATE SET
                     delivery = excluded.delivery, set_by = excluded.set_by",
                params![from, to, text_of(delivery), set_by],
            )
            .map_err(RpcError::internal)?;
        Ok(())
    }

    /// M14: whether an Agent set the Route `from` to `to`; false for none, the user's or an Extension's.
    pub(crate) fn route_set_by_agent(&self, from: &str, to: &str) -> Result<bool, RpcError> {
        self.db
            .query_row(
                "SELECT set_by FROM routes WHERE from_id = ?1 AND to_id = ?2",
                params![from, to],
                |row| Ok(row.get::<_, String>(0)? == "agent"),
            )
            .optional()
            .map(|set| set.unwrap_or(false))
            .map_err(RpcError::internal)
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

/// The SQL that picks a pushed digest (B22) out of `messages`, taking the Daemon as `?2` and the
/// body prefix as `?3`.
const DIGEST: &str = "from_actor = ?2 AND substr(body, 1, length(?3)) = ?3";

fn list_text(list: &[String]) -> String {
    serde_json::to_string(list).unwrap_or_else(|_| "[]".to_owned())
}

fn from_list(column: usize, text: &str) -> rusqlite::Result<Vec<String>> {
    serde_json::from_str(text).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            Box::new(err),
        )
    })
}

fn row_to_wake(row: &rusqlite::Row<'_>) -> rusqlite::Result<Wake> {
    let ids: String = row.get(3)?;
    let finished: String = row.get(4)?;
    Ok(Wake {
        id: row.get(0)?,
        waiter: row.get(1)?,
        any_of: row.get(2)?,
        ids: from_list(3, &ids)?,
        finished: from_list(4, &finished)?,
        seen: u64::try_from(row.get::<_, i64>(5)?).unwrap_or(0),
    })
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
        iteration: None,
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
    use contracts::ActorKind;

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
    fn b6_begin_between_check_and_record_holds_the_message() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = open(dir.path());
        let id = put(&store, MessageStatus::Pending, None);
        assert_eq!(store.pending_queue("b", (1, i64::MAX)).unwrap()[0].id, id);

        store
            .begin_takeover("b", |pending| pending.iter().map(|m| m.id).collect())
            .unwrap();

        assert!(store.record_flight(id, id).unwrap().is_none());
        let message = store.get(id).unwrap().unwrap();
        assert_eq!(
            (message.status, message.reason),
            (MessageStatus::Held, Some(Reason::Takeover))
        );
    }

    #[test]
    fn b32_input_between_the_check_and_the_record_holds_the_message() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = open(dir.path());
        let id = put(&store, MessageStatus::Pending, None);
        assert_eq!(store.pending_queue("b", (1, i64::MAX)).unwrap()[0].id, id);

        store
            .begin_input("b", |pending| pending.iter().map(|m| m.id).collect())
            .unwrap();

        assert!(store.record_flight(id, id).unwrap().is_none());
        let message = store.get(id).unwrap().unwrap();
        assert_eq!(
            (message.status, message.reason),
            (MessageStatus::Held, Some(Reason::Takeover))
        );
    }
}
