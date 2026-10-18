//! The hash of the `.lgtm/setup` the user last let run.

use rusqlite::OptionalExtension;

use super::Store;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS setup_approval (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    hash TEXT NOT NULL
);
";

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(SCHEMA)
}

impl Store {
    pub fn approved_setup(&self) -> rusqlite::Result<Option<String>> {
        self.db
            .query_row("SELECT hash FROM setup_approval WHERE id = 0", [], |r| {
                r.get(0)
            })
            .optional()
    }

    pub fn approve_setup(&self, hash: &str) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO setup_approval (id, hash) VALUES (0, ?)",
            [hash],
        )?;
        Ok(())
    }
}
