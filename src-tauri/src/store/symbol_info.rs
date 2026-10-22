//! Symbol Information, stored as JSON by the symbol_info module.

use rusqlite::params;

use super::Store;

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS symbol_info (
            path TEXT NOT NULL,
            name TEXT NOT NULL,
            data TEXT NOT NULL,
            working INTEGER NOT NULL,
            PRIMARY KEY (path, name)
        );",
    )
}

impl Store {
    /// Names, data and whether an agent is on it, for symbols defined in `path`.
    pub fn symbol_infos(&self, path: &str) -> rusqlite::Result<Vec<(String, String, bool)>> {
        let mut stmt = self
            .db
            .prepare("SELECT name, data, working FROM symbol_info WHERE path = ?")?;
        stmt.query_map([path], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect()
    }

    pub fn save_symbol_info(
        &self,
        path: &str,
        name: &str,
        data: &str,
        working: bool,
    ) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO symbol_info (path, name, data, working) VALUES (?, ?, ?, ?)",
            params![path, name, data, working],
        )?;
        Ok(())
    }

    pub fn delete_symbol_info(&self, path: &str, name: &str) -> rusqlite::Result<()> {
        self.db.execute(
            "DELETE FROM symbol_info WHERE path = ? AND name = ?",
            params![path, name],
        )?;
        Ok(())
    }
}
