//! The PR each PR workspace was checked out for, stored whole as JSON.

use rusqlite::{OptionalExtension, params};

use super::{Store, json_err};
use crate::pr::PrRecord;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS prs (
    workspace TEXT PRIMARY KEY,
    data TEXT NOT NULL
);
";

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(SCHEMA)
}

fn parse(data: String) -> rusqlite::Result<PrRecord> {
    serde_json::from_str(&data).map_err(json_err)
}

impl Store {
    pub fn save_pr(&self, pr: &PrRecord) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO prs (workspace, data) VALUES (?, ?)",
            params![pr.workspace, serde_json::to_string(pr).map_err(json_err)?],
        )?;
        Ok(())
    }

    pub fn pr(&self, workspace: &str) -> rusqlite::Result<Option<PrRecord>> {
        self.db
            .query_row(
                "SELECT data FROM prs WHERE workspace = ?",
                [workspace],
                |r| r.get(0),
            )
            .optional()?
            .map(parse)
            .transpose()
    }

    pub fn prs(&self) -> rusqlite::Result<Vec<PrRecord>> {
        let mut stmt = self.db.prepare("SELECT data FROM prs")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.map(|d| parse(d?)).collect()
    }

    pub fn delete_pr(&self, workspace: &str) -> rusqlite::Result<()> {
        self.db
            .execute("DELETE FROM prs WHERE workspace = ?", [workspace])?;
        Ok(())
    }
}
