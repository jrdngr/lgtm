//! The revset each workspace's revisions log shows, when it isn't jj's default.

use rusqlite::{OptionalExtension, params};

use super::Store;

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS log_revsets (
            workspace TEXT PRIMARY KEY,
            revset TEXT NOT NULL
        );",
    )
}

impl Store {
    pub fn log_revset(&self, workspace: &str) -> rusqlite::Result<Option<String>> {
        self.db
            .query_row(
                "SELECT revset FROM log_revsets WHERE workspace = ?",
                [workspace],
                |r| r.get(0),
            )
            .optional()
    }

    /// Sets the workspace's log revset, or with `None` goes back to jj's default.
    pub fn set_log_revset(&self, workspace: &str, revset: Option<&str>) -> rusqlite::Result<()> {
        match revset {
            Some(r) => self.db.execute(
                "INSERT OR REPLACE INTO log_revsets (workspace, revset) VALUES (?, ?)",
                params![workspace, r],
            )?,
            None => self
                .db
                .execute("DELETE FROM log_revsets WHERE workspace = ?", [workspace])?,
        };
        Ok(())
    }
}
