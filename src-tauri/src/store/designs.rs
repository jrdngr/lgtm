//! Designs, stored whole as JSON by the design module.

use rusqlite::{OptionalExtension, params};

use super::Store;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS designs (
    id TEXT PRIMARY KEY,
    data TEXT NOT NULL
);
";

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(SCHEMA)
}

impl Store {
    pub fn design(&self, id: &str) -> rusqlite::Result<Option<String>> {
        self.db
            .query_row("SELECT data FROM designs WHERE id = ?", [id], |r| r.get(0))
            .optional()
    }

    /// Every design, oldest first.
    pub fn designs(&self) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self.db.prepare("SELECT data FROM designs ORDER BY id")?;
        stmt.query_map([], |r| r.get(0))?.collect()
    }

    pub fn save_design(&self, id: &str, data: &str) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO designs (id, data) VALUES (?, ?)",
            params![id, data],
        )?;
        Ok(())
    }

    pub fn delete_design(&self, id: &str) -> rusqlite::Result<()> {
        self.db.execute("DELETE FROM designs WHERE id = ?", [id])?;
        Ok(())
    }
}
