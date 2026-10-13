//! What lgtm keeps about each design outside its folder (the agent's session, comments on its
//! docs, what the user last saw), stored whole as JSON by the design module.

use rusqlite::{OptionalExtension, params};

use super::Store;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS designs (
    workspace TEXT NOT NULL,
    slug TEXT NOT NULL,
    data TEXT NOT NULL,
    PRIMARY KEY (workspace, slug)
);
";

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(SCHEMA)
}

impl Store {
    pub fn design_state(&self, workspace: &str, slug: &str) -> rusqlite::Result<Option<String>> {
        self.db
            .query_row(
                "SELECT data FROM designs WHERE workspace = ? AND slug = ?",
                [workspace, slug],
                |r| r.get(0),
            )
            .optional()
    }

    pub fn save_design_state(
        &self,
        workspace: &str,
        slug: &str,
        data: &str,
    ) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO designs (workspace, slug, data) VALUES (?, ?, ?)",
            params![workspace, slug, data],
        )?;
        Ok(())
    }
}
