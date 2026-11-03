//! Labels `lgtm snap` gives snapshots of the working copy, by the commit jj made for each.

use std::collections::HashMap;

use rusqlite::params;

use super::Store;

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS snap_labels (
            commit_id TEXT PRIMARY KEY,
            label TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );",
    )
}

impl Store {
    /// Labels the snapshot `commit`, replacing any label it had.
    pub fn label_snapshot(&self, commit: &str, label: &str, now: i64) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO snap_labels (commit_id, label, created_at) VALUES (?, ?, ?)",
            params![commit, label, now],
        )?;
        Ok(())
    }

    /// The labels of those of `commits` that have one.
    pub fn snapshot_labels(&self, commits: &[String]) -> rusqlite::Result<HashMap<String, String>> {
        let mut stmt = self
            .db
            .prepare_cached("SELECT label FROM snap_labels WHERE commit_id = ?")?;
        let mut out = HashMap::new();
        for c in commits {
            let label: Option<String> =
                stmt.query_row([c], |r| r.get(0))
                    .map(Some)
                    .or_else(|e| match e {
                        rusqlite::Error::QueryReturnedNoRows => Ok(None),
                        e => Err(e),
                    })?;
            if let Some(label) = label {
                out.insert(c.clone(), label);
            }
        }
        Ok(out)
    }
}
