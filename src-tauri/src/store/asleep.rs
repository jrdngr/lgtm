//! Workspaces put to sleep, by name, "" being the main one.

use std::collections::HashSet;

use rusqlite::params;

use super::Store;

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS asleep (workspace TEXT PRIMARY KEY);")
}

impl Store {
    pub fn asleep(&self) -> rusqlite::Result<HashSet<String>> {
        let mut stmt = self.db.prepare("SELECT workspace FROM asleep")?;
        stmt.query_map([], |r| r.get(0))?.collect()
    }

    pub fn set_asleep(&self, workspace: &str, asleep: bool) -> rusqlite::Result<()> {
        let sql = if asleep {
            "INSERT OR IGNORE INTO asleep (workspace) VALUES (?)"
        } else {
            "DELETE FROM asleep WHERE workspace = ?"
        };
        self.db.execute(sql, params![workspace])?;
        Ok(())
    }
}
