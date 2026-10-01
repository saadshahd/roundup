//! The app-stored Pads: one SQLite table in `<dir>/pads.db`.

use std::path::Path;

use contracts::{Actor, pad::Pad};
use rusqlite::{Connection, OptionalExtension, params};
use unicode_normalization::UnicodeNormalization;

pub struct Store {
    db: Connection,
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let db = Connection::open(path)?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS pads (
                key TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                owner TEXT NOT NULL,
                text TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );",
        )?;
        Ok(Self { db })
    }

    /// False when a Pad with this name already exists.
    pub fn insert(&self, pad: &Pad) -> rusqlite::Result<bool> {
        let changed = self.db.execute(
            "INSERT OR IGNORE INTO pads (key, name, owner, text, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                key(&pad.name),
                pad.name,
                owner_json(&pad.owner),
                pad.text,
                pad.updated_at
            ],
        )?;
        Ok(changed == 1)
    }

    pub fn get(&self, name: &str) -> rusqlite::Result<Option<Pad>> {
        self.db
            .query_row(
                "SELECT name, owner, text, updated_at FROM pads WHERE key = ?1",
                [key(name)],
                row_to_pad,
            )
            .optional()
    }

    /// Ordered by name.
    pub fn list(&self) -> rusqlite::Result<Vec<Pad>> {
        let mut stmt = self
            .db
            .prepare("SELECT name, owner, text, updated_at FROM pads ORDER BY name")?;
        stmt.query_map([], row_to_pad)?.collect()
    }

    pub fn set_text(&self, name: &str, text: &str, at: i64) -> rusqlite::Result<()> {
        self.db.execute(
            "UPDATE pads SET text = ?2, updated_at = ?3 WHERE key = ?1",
            params![key(name), text, at],
        )?;
        Ok(())
    }

    pub fn delete(&self, name: &str) -> rusqlite::Result<()> {
        self.db
            .execute("DELETE FROM pads WHERE key = ?1", [key(name)])?;
        Ok(())
    }

    pub fn set_owner(&self, name: &str, owner: &Actor, at: i64) -> rusqlite::Result<()> {
        self.db.execute(
            "UPDATE pads SET owner = ?2, updated_at = ?3 WHERE key = ?1",
            params![key(name), owner_json(owner), at],
        )?;
        Ok(())
    }
}

/// Names are unique ignoring case and Unicode normalization: APFS treats both kinds of pair as one file.
fn key(name: &str) -> String {
    let decomposed: String = name.nfd().collect();
    caseless::default_case_fold_str(&decomposed).nfc().collect()
}

fn owner_json(owner: &Actor) -> String {
    serde_json::to_string(owner).expect("an Actor is always serializable")
}

fn row_to_pad(row: &rusqlite::Row<'_>) -> rusqlite::Result<Pad> {
    let owner: String = row.get(1)?;
    Ok(Pad {
        name: row.get(0)?,
        owner: serde_json::from_str(&owner).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(err))
        })?,
        text: row.get(2)?,
        updated_at: row.get(3)?,
    })
}
