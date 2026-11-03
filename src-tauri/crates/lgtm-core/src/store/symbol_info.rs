//! Symbol Information, stored as JSON by the symbol_info module.

use rusqlite::params;

use super::Store;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS symbol_info (
    workspace TEXT NOT NULL,
    path TEXT NOT NULL,
    name TEXT NOT NULL,
    nth INTEGER NOT NULL,
    data TEXT NOT NULL,
    working INTEGER NOT NULL,
    PRIMARY KEY (workspace, path, name, nth)
);
";

/// Creates the table. Rows of the first shape, keyed by path and name only, move over as the
/// main workspace's, on the first definition of the name.
pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    let has = |column: &str| -> rusqlite::Result<bool> {
        db.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('symbol_info') WHERE name = ?",
            [column],
            |r| r.get(0),
        )
    };
    if has("path")? && !has("workspace")? {
        db.execute_batch(&format!(
            "ALTER TABLE symbol_info RENAME TO symbol_info_old;
             {SCHEMA}
             INSERT INTO symbol_info (workspace, path, name, nth, data, working)
             SELECT '', path, name, 0, data, working FROM symbol_info_old;
             DROP TABLE symbol_info_old;"
        ))
    } else {
        db.execute_batch(SCHEMA)
    }
}

/// One definition's row: its workspace, path, name, and which of the file's definitions of
/// that name it is, from 0.
pub struct InfoKey<'a> {
    pub workspace: &'a str,
    pub path: &'a str,
    pub name: &'a str,
    pub nth: u32,
}

impl Store {
    /// Names, which definition, data, and whether an agent is on it, for symbols defined in
    /// `path` in `workspace`.
    pub fn symbol_infos(
        &self,
        workspace: &str,
        path: &str,
    ) -> rusqlite::Result<Vec<(String, u32, String, bool)>> {
        let mut stmt = self.db.prepare(
            "SELECT name, nth, data, working FROM symbol_info WHERE workspace = ? AND path = ?",
        )?;
        stmt.query_map([workspace, path], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect()
    }

    pub fn save_symbol_info(&self, k: &InfoKey, data: &str, working: bool) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO symbol_info (workspace, path, name, nth, data, working)
             VALUES (?, ?, ?, ?, ?, ?)",
            params![k.workspace, k.path, k.name, k.nth, data, working],
        )?;
        Ok(())
    }

    pub fn delete_symbol_info(&self, k: &InfoKey) -> rusqlite::Result<()> {
        self.db.execute(
            "DELETE FROM symbol_info WHERE workspace = ? AND path = ? AND name = ? AND nth = ?",
            params![k.workspace, k.path, k.name, k.nth],
        )?;
        Ok(())
    }
}
