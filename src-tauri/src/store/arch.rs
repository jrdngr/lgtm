//! Architecture review state: the lens, assessments and Look checks as JSON under a key, and
//! intents as JSON rows.

use rusqlite::{OptionalExtension, params};

use super::Store;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS arch_state (
    key TEXT PRIMARY KEY,
    data TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS arch_intents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    data TEXT NOT NULL
);
";

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(SCHEMA)
}

impl Store {
    pub fn arch_state(&self, key: &str) -> rusqlite::Result<Option<String>> {
        self.db
            .query_row("SELECT data FROM arch_state WHERE key = ?", [key], |r| r.get(0))
            .optional()
    }

    pub fn save_arch_state(&self, key: &str, data: &str) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO arch_state (key, data) VALUES (?, ?)",
            params![key, data],
        )?;
        Ok(())
    }

    /// Every intent, oldest first.
    pub fn arch_intents(&self) -> rusqlite::Result<Vec<(u32, String)>> {
        let mut stmt = self.db.prepare("SELECT id, data FROM arch_intents ORDER BY id")?;
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect()
    }

    pub fn add_arch_intent(&self, data: &str) -> rusqlite::Result<u32> {
        self.db.execute("INSERT INTO arch_intents (data) VALUES (?)", [data])?;
        Ok(self.db.last_insert_rowid() as u32)
    }

    pub fn delete_arch_intent(&self, id: u32) -> rusqlite::Result<()> {
        self.db.execute("DELETE FROM arch_intents WHERE id = ?", [id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intents_keep_order_and_delete_by_id() {
        let store = Store::in_memory();
        let a = store.add_arch_intent("a").unwrap();
        let b = store.add_arch_intent("b").unwrap();
        store.delete_arch_intent(a).unwrap();
        assert_eq!(store.arch_intents().unwrap(), vec![(b, "b".to_string())]);
        store.save_arch_state("lens", "1").unwrap();
        store.save_arch_state("lens", "2").unwrap();
        assert_eq!(store.arch_state("lens").unwrap().as_deref(), Some("2"));
    }
}
