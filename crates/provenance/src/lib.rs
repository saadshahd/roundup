//! Provenance: the append-only log of Touches. There is no update or delete.

use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use contracts::{Actor, ActorKind, Touch, Verb};
use rusqlite::{Connection, params};

pub struct Touches {
    db: Mutex<Connection>,
}

impl Touches {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        Self::from_connection(Connection::open(path)?)
    }

    pub fn in_memory() -> rusqlite::Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(db: Connection) -> rusqlite::Result<Self> {
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS touches (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                actor_kind TEXT NOT NULL,
                actor_id TEXT NOT NULL,
                actor_parent TEXT,
                verb TEXT NOT NULL,
                item TEXT NOT NULL,
                at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS touches_item ON touches(item, seq);
            CREATE INDEX IF NOT EXISTS touches_actor ON touches(actor_id, seq);",
        )?;
        Ok(Self { db: Mutex::new(db) })
    }

    pub fn record(&self, actor: &Actor, verb: Verb, item: &str) -> rusqlite::Result<()> {
        let at = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
        )
        .unwrap_or(i64::MAX);
        self.db()?.execute(
            "INSERT INTO touches (actor_kind, actor_id, actor_parent, verb, item, at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![kind_str(actor.kind), actor.id, actor.parent, verb_str(verb), item, at],
        )?;
        Ok(())
    }

    /// Oldest first.
    pub fn history(&self, item: &str) -> rusqlite::Result<Vec<Touch>> {
        self.query("WHERE item = ?1", item)
    }

    /// Oldest first.
    pub fn touched(&self, actor_id: &str) -> rusqlite::Result<Vec<Touch>> {
        self.query("WHERE actor_id = ?1", actor_id)
    }

    fn db(&self) -> rusqlite::Result<std::sync::MutexGuard<'_, Connection>> {
        self.db.lock().map_err(|_| rusqlite::Error::InvalidQuery)
    }

    fn query(&self, filter: &str, arg: &str) -> rusqlite::Result<Vec<Touch>> {
        let db = self.db()?;
        let mut stmt = db.prepare(&format!(
            "SELECT actor_kind, actor_id, actor_parent, verb, item, at FROM touches {filter} ORDER BY seq"
        ))?;
        stmt.query_map([arg], |row| {
            Ok(Touch {
                actor: Actor {
                    kind: parse_kind(&row.get::<_, String>(0)?),
                    id: row.get(1)?,
                    parent: row.get(2)?,
                },
                verb: parse_verb(&row.get::<_, String>(3)?),
                item: row.get(4)?,
                at: row.get(5)?,
            })
        })?
        .collect()
    }
}

fn kind_str(kind: ActorKind) -> &'static str {
    match kind {
        ActorKind::User => "user",
        ActorKind::Agent => "agent",
        ActorKind::Ext => "ext",
    }
}

fn parse_kind(text: &str) -> ActorKind {
    match text {
        "agent" => ActorKind::Agent,
        "ext" => ActorKind::Ext,
        _ => ActorKind::User,
    }
}

fn verb_str(verb: Verb) -> &'static str {
    match verb {
        Verb::Read => "read",
        Verb::Wrote => "wrote",
    }
}

fn parse_verb(text: &str) -> Verb {
    if text == "read" {
        Verb::Read
    } else {
        Verb::Wrote
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(id: &str) -> Actor {
        Actor {
            kind: ActorKind::Agent,
            id: id.into(),
            parent: None,
        }
    }

    #[test]
    fn history_lists_touches_of_one_item_oldest_first() {
        let log = Touches::in_memory().unwrap();
        log.record(&Actor::user(), Verb::Wrote, "todo:5").unwrap();
        log.record(&agent("a"), Verb::Read, "todo:5").unwrap();
        log.record(&agent("a"), Verb::Wrote, "pad:notes").unwrap();

        let history = log.history("todo:5").unwrap();

        assert_eq!(history.len(), 2);
        assert_eq!(history[0].actor, Actor::user());
        assert_eq!(history[1].verb, Verb::Read);
    }

    #[test]
    fn touched_lists_everything_one_actor_did() {
        let log = Touches::in_memory().unwrap();
        log.record(&agent("a"), Verb::Read, "todo:5").unwrap();
        log.record(&agent("b"), Verb::Read, "todo:5").unwrap();
        log.record(&agent("a"), Verb::Wrote, "pad:notes").unwrap();

        assert_eq!(log.touched("a").unwrap().len(), 2);
    }
}
