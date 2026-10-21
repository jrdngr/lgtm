//! Drawings, stored whole as JSON by the drawings module.

use rusqlite::{OptionalExtension, params};

use super::Store;

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS drawings (
            id TEXT PRIMARY KEY,
            data TEXT NOT NULL
        );",
    )
}

impl Store {
    pub fn drawing(&self, id: &str) -> rusqlite::Result<Option<String>> {
        self.db
            .query_row("SELECT data FROM drawings WHERE id = ?", [id], |r| r.get(0))
            .optional()
    }

    pub fn drawings(&self) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self.db.prepare("SELECT data FROM drawings")?;
        stmt.query_map([], |r| r.get(0))?.collect()
    }

    pub fn save_drawing(&self, id: &str, data: &str) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO drawings (id, data) VALUES (?, ?)",
            params![id, data],
        )?;
        Ok(())
    }

    pub fn delete_drawing(&self, id: &str) -> rusqlite::Result<()> {
        self.db.execute("DELETE FROM drawings WHERE id = ?", [id])?;
        Ok(())
    }
}
