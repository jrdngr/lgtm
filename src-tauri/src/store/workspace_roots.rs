//! Where the user said a workspace's files are, for ones jj's recorded path doesn't find.

use std::collections::HashMap;
use std::path::PathBuf;

use rusqlite::params;

use super::Store;

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS workspace_roots (
            workspace TEXT PRIMARY KEY,
            root TEXT NOT NULL
        );",
    )
}

impl Store {
    pub fn workspace_roots(&self) -> rusqlite::Result<HashMap<String, PathBuf>> {
        let mut stmt = self
            .db
            .prepare("SELECT workspace, root FROM workspace_roots")?;
        stmt.query_map([], |r| {
            Ok((r.get(0)?, PathBuf::from(r.get::<_, String>(1)?)))
        })?
        .collect()
    }

    pub fn set_workspace_root(&self, workspace: &str, root: &str) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO workspace_roots (workspace, root) VALUES (?, ?)",
            params![workspace, root],
        )?;
        Ok(())
    }
}
