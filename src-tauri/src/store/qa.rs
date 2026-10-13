//! Q&A sessions, stored whole as JSON by the qa module.

use rusqlite::{OptionalExtension, params};

use super::Store;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS qa_sessions (
    workspace TEXT NOT NULL,
    id TEXT NOT NULL,
    data TEXT NOT NULL,
    PRIMARY KEY (workspace, id)
);
";

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(SCHEMA)
}

impl Store {
    pub fn qa_session(&self, workspace: &str, id: &str) -> rusqlite::Result<Option<String>> {
        self.db
            .query_row(
                "SELECT data FROM qa_sessions WHERE workspace = ? AND id = ?",
                [workspace, id],
                |r| r.get(0),
            )
            .optional()
    }

    /// Every session in the workspace, oldest first.
    pub fn qa_sessions(&self, workspace: &str) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self
            .db
            .prepare("SELECT data FROM qa_sessions WHERE workspace = ? ORDER BY id")?;
        stmt.query_map([workspace], |r| r.get(0))?.collect()
    }

    pub fn save_qa_session(&self, workspace: &str, id: &str, data: &str) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO qa_sessions (workspace, id, data) VALUES (?, ?, ?)",
            params![workspace, id, data],
        )?;
        Ok(())
    }

    pub fn delete_qa_session(&self, workspace: &str, id: &str) -> rusqlite::Result<()> {
        self.db.execute(
            "DELETE FROM qa_sessions WHERE workspace = ? AND id = ?",
            [workspace, id],
        )?;
        Ok(())
    }
}
