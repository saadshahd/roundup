//! SQLite persistence for Todos. `blocked` is computed on read, never stored.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use contracts::todo::Todo;
use rpc::RpcError;
use rusqlite::{Connection, OptionalExtension, params};

pub(crate) struct Store {
    db: Connection,
}

impl Store {
    pub(crate) fn open(path: &Path) -> rusqlite::Result<Self> {
        let db = Connection::open(path)?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.pragma_update(None, "foreign_keys", true)?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS todos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                body TEXT NOT NULL,
                done INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS blockers (
                todo INTEGER NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
                blocker INTEGER NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
                PRIMARY KEY (todo, blocker)
            );",
        )?;
        Ok(Self { db })
    }

    /// Every id in `blockers` must exist, or the call is `NOT_FOUND`.
    pub(crate) fn insert(
        &self,
        title: &str,
        body: &str,
        blockers: &[u32],
    ) -> Result<Todo, RpcError> {
        self.require_all(blockers)?;
        let created_at = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
        )
        .unwrap_or(i64::MAX);
        let tx = self
            .db
            .unchecked_transaction()
            .map_err(RpcError::internal)?;
        tx.execute(
            "INSERT INTO todos (title, body, created_at) VALUES (?1, ?2, ?3)",
            params![title, body, created_at],
        )
        .map_err(RpcError::internal)?;
        let id = u32::try_from(tx.last_insert_rowid()).map_err(RpcError::internal)?;
        self.insert_blockers(id, blockers)?;
        tx.commit().map_err(RpcError::internal)?;
        self.get(id)
    }

    pub(crate) fn get(&self, id: u32) -> Result<Todo, RpcError> {
        let row = self
            .db
            .query_row(
                "SELECT title, body, done, created_at, EXISTS(
                    SELECT 1 FROM blockers b JOIN todos o ON o.id = b.blocker
                    WHERE b.todo = todos.id AND o.done = 0
                ) FROM todos WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()
            .map_err(RpcError::internal)?;
        let (title, body, done, created_at, blocked) =
            row.ok_or_else(|| RpcError::not_found(format!("todo {id}")))?;
        Ok(Todo {
            id,
            title,
            body,
            done,
            blockers: self.blockers_of(id)?,
            blocked,
            created_at,
        })
    }

    pub(crate) fn list(&self) -> Result<Vec<Todo>, RpcError> {
        let ids = self
            .db
            .prepare("SELECT id FROM todos ORDER BY id")
            .and_then(|mut stmt| {
                stmt.query_map([], |r| r.get::<_, u32>(0))?
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(RpcError::internal)?;
        ids.into_iter().map(|id| self.get(id)).collect()
    }

    pub(crate) fn update(
        &self,
        id: u32,
        title: Option<&str>,
        body: Option<&str>,
    ) -> Result<Todo, RpcError> {
        self.db
            .execute(
                "UPDATE todos SET title = COALESCE(?2, title), body = COALESCE(?3, body) WHERE id = ?1",
                params![id, title, body],
            )
            .map_err(RpcError::internal)?;
        self.get(id)
    }

    pub(crate) fn complete(&self, id: u32) -> Result<Todo, RpcError> {
        self.get(id)?;
        self.db
            .execute("UPDATE todos SET done = 1 WHERE id = ?1", [id])
            .map_err(internal)?;
        self.get(id)
    }

    /// Replaces the whole blocker list. The caller has checked ids and cycles.
    pub(crate) fn set_blockers(&self, id: u32, blockers: &[u32]) -> Result<Todo, RpcError> {
        self.db
            .execute("DELETE FROM blockers WHERE todo = ?1", [id])
            .map_err(internal)?;
        self.insert_blockers(id, blockers)?;
        self.get(id)
    }

    /// Blockers pointing at `id` go with it (foreign keys cascade).
    pub(crate) fn delete(&self, id: u32) -> Result<(), RpcError> {
        self.get(id)?;
        self.db
            .execute("DELETE FROM todos WHERE id = ?1", [id])
            .map_err(internal)?;
        Ok(())
    }

    pub(crate) fn blocked_ids(&self) -> Result<BTreeSet<u32>, RpcError> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|t| t.blocked)
            .map(|t| t.id)
            .collect())
    }

    pub(crate) fn edges(&self) -> Result<HashMap<u32, Vec<u32>>, RpcError> {
        Ok(self
            .list()?
            .into_iter()
            .map(|t| (t.id, t.blockers))
            .collect())
    }

    fn blockers_of(&self, id: u32) -> Result<Vec<u32>, RpcError> {
        self.db
            .prepare("SELECT blocker FROM blockers WHERE todo = ?1 ORDER BY blocker")
            .and_then(|mut stmt| stmt.query_map([id], |r| r.get(0))?.collect())
            .map_err(RpcError::internal)
    }

    fn insert_blockers(&self, id: u32, blockers: &[u32]) -> Result<(), RpcError> {
        for blocker in blockers {
            self.db
                .execute(
                    "INSERT OR IGNORE INTO blockers (todo, blocker) VALUES (?1, ?2)",
                    [id, *blocker],
                )
                .map_err(RpcError::internal)?;
        }
        Ok(())
    }

    pub(crate) fn require_all(&self, ids: &[u32]) -> Result<(), RpcError> {
        ids.iter().try_for_each(|id| self.get(*id).map(drop))
    }
}
