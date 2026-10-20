//! Designs, stored whole as JSON by the design module.

use rusqlite::{OptionalExtension, params};

use super::Store;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS designs (
    id TEXT PRIMARY KEY,
    data TEXT NOT NULL
);
";

/// Creates the table. A `designs` table of another shape, left by an earlier design feature,
/// is kept as `designs_old`, and any of its rows holding a design's JSON with an `id` move over.
pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    let columns: Vec<String> = db
        .prepare("SELECT name FROM pragma_table_info('designs')")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let current = columns.iter().any(|c| c == "id") && columns.iter().any(|c| c == "data");
    if columns.is_empty() || current {
        return db.execute_batch(SCHEMA);
    }
    db.execute_batch("ALTER TABLE designs RENAME TO designs_old;")?;
    db.execute_batch(SCHEMA)?;
    if columns.iter().any(|c| c == "data") {
        db.execute_batch(
            "INSERT OR IGNORE INTO designs (id, data)
             SELECT json_extract(data, '$.id'), data FROM designs_old
             WHERE json_valid(data) AND json_extract(data, '$.id') IS NOT NULL;",
        )?;
    }
    Ok(())
}

impl Store {
    pub fn design(&self, id: &str) -> rusqlite::Result<Option<String>> {
        self.db
            .query_row("SELECT data FROM designs WHERE id = ?", [id], |r| r.get(0))
            .optional()
    }

    /// Every design, oldest first.
    pub fn designs(&self) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self.db.prepare("SELECT data FROM designs ORDER BY id")?;
        stmt.query_map([], |r| r.get(0))?.collect()
    }

    pub fn save_design(&self, id: &str, data: &str) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO designs (id, data) VALUES (?, ?)",
            params![id, data],
        )?;
        Ok(())
    }

    pub fn delete_design(&self, id: &str) -> rusqlite::Result<()> {
        self.db.execute("DELETE FROM designs WHERE id = ?", [id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_old_designs_table_is_kept_aside_and_its_designs_move_over() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE designs (slug TEXT PRIMARY KEY, data TEXT NOT NULL);
             INSERT INTO designs VALUES ('a', '{\"id\":\"d1\",\"name\":\"A\"}');
             INSERT INTO designs VALUES ('b', 'not json');",
        )
        .unwrap();
        create_table(&db).unwrap();
        let store = Store { db };
        assert_eq!(store.designs().unwrap(), [r#"{"id":"d1","name":"A"}"#]);
        let old: i64 = store
            .db
            .query_row("SELECT COUNT(*) FROM designs_old", [], |r| r.get(0))
            .unwrap();
        assert_eq!(old, 2);
    }
}
