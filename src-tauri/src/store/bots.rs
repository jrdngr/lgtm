//! Persona review runs, stored whole as JSON.

use rusqlite::{OptionalExtension, params};

use super::Store;
use crate::bots::BotRun;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS bot_runs (
    id INTEGER PRIMARY KEY,
    workspace TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    data TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS bot_runs_workspace ON bot_runs (workspace);
";

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(SCHEMA)
}

fn to_json(run: &BotRun) -> rusqlite::Result<String> {
    serde_json::to_string(run).map_err(super::json_err)
}

fn row_to_run(r: &rusqlite::Row) -> rusqlite::Result<BotRun> {
    let mut run: BotRun = serde_json::from_str(&r.get::<_, String>(1)?).map_err(super::json_err)?;
    run.id = r.get(0)?;
    Ok(run)
}

impl Store {
    /// Inserts a run and returns its id.
    pub fn add_bot_run(&self, run: &BotRun) -> rusqlite::Result<u32> {
        self.db.execute(
            "INSERT INTO bot_runs (workspace, created_at, data) VALUES (?, ?, ?)",
            params![
                run.workspace.as_deref().unwrap_or(""),
                run.created_at as i64,
                to_json(run)?
            ],
        )?;
        Ok(self.db.last_insert_rowid() as u32)
    }

    pub fn bot_run(&self, id: u32) -> rusqlite::Result<Option<BotRun>> {
        self.db
            .query_row(
                "SELECT id, data FROM bot_runs WHERE id = ?",
                [id],
                row_to_run,
            )
            .optional()
    }

    /// A workspace's runs, newest first. `""` is the main workspace. Runs stored by an older
    /// build in a shape this one can't read are left out.
    pub fn bot_runs(&self, workspace: &str) -> rusqlite::Result<Vec<BotRun>> {
        let mut stmt = self.db.prepare(
            "SELECT id, data FROM bot_runs WHERE workspace = ? ORDER BY created_at DESC, id DESC",
        )?;
        Ok(stmt
            .query_map([workspace], row_to_run)?
            .filter_map(Result::ok)
            .collect())
    }

    /// Every workspace's runs.
    pub fn all_bot_runs(&self) -> rusqlite::Result<Vec<BotRun>> {
        let mut stmt = self.db.prepare("SELECT id, data FROM bot_runs")?;
        Ok(stmt
            .query_map([], row_to_run)?
            .filter_map(Result::ok)
            .collect())
    }

    pub fn save_bot_run(&self, run: &BotRun) -> rusqlite::Result<()> {
        self.db.execute(
            "UPDATE bot_runs SET data = ? WHERE id = ?",
            params![to_json(run)?, run.id],
        )?;
        Ok(())
    }

    pub fn delete_bot_run(&self, id: u32) -> rusqlite::Result<()> {
        self.db.execute("DELETE FROM bot_runs WHERE id = ?", [id])?;
        Ok(())
    }
}

#[cfg(test)]
impl Store {
    /// Stores a run as raw JSON, as an older build might have.
    pub fn add_bot_run_raw_for_test(&self, workspace: &str, data: &str) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT INTO bot_runs (workspace, created_at, data) VALUES (?, 0, ?)",
            params![workspace, data],
        )?;
        Ok(())
    }
}
