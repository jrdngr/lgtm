//! Architecture review state: the lens and system description as JSON under a key, each
//! workspace's assessments and Look checks as JSON by revset, and intents as JSON rows.

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
CREATE TABLE IF NOT EXISTS arch_reviews (
    workspace TEXT NOT NULL,
    revset TEXT NOT NULL,
    kind TEXT NOT NULL,
    data TEXT NOT NULL,
    PRIMARY KEY (workspace, revset, kind)
);
";

/// Assessments and Look checks were once keyed by revset alone, in arch_state. They move to the
/// main workspace, whose reviews most likely made them.
const MIGRATE: &str = "
BEGIN;
INSERT OR IGNORE INTO arch_reviews
    SELECT '', substr(key, 8), 'assess', data FROM arch_state WHERE key LIKE 'assess:%';
INSERT OR IGNORE INTO arch_reviews
    SELECT '', substr(key, 6), 'look', data FROM arch_state WHERE key LIKE 'look:%';
DELETE FROM arch_state WHERE key LIKE 'assess:%' OR key LIKE 'look:%';
COMMIT;
";

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(SCHEMA)?;
    db.execute_batch(MIGRATE)
}

impl Store {
    pub fn arch_state(&self, key: &str) -> rusqlite::Result<Option<String>> {
        self.db
            .query_row("SELECT data FROM arch_state WHERE key = ?", [key], |r| {
                r.get(0)
            })
            .optional()
    }

    pub fn save_arch_state(&self, key: &str, data: &str) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO arch_state (key, data) VALUES (?, ?)",
            params![key, data],
        )?;
        Ok(())
    }

    /// Workspace `workspace`'s `kind` of state for `revset`.
    pub fn arch_review(
        &self,
        workspace: &str,
        revset: &str,
        kind: &str,
    ) -> rusqlite::Result<Option<String>> {
        self.db
            .query_row(
                "SELECT data FROM arch_reviews WHERE workspace = ? AND revset = ? AND kind = ?",
                [workspace, revset, kind],
                |r| r.get(0),
            )
            .optional()
    }

    pub fn save_arch_review(
        &self,
        workspace: &str,
        revset: &str,
        kind: &str,
        data: &str,
    ) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO arch_reviews (workspace, revset, kind, data) VALUES (?, ?, ?, ?)",
            params![workspace, revset, kind, data],
        )?;
        Ok(())
    }

    /// Every intent, oldest first.
    pub fn arch_intents(&self) -> rusqlite::Result<Vec<(u32, String)>> {
        let mut stmt = self
            .db
            .prepare("SELECT id, data FROM arch_intents ORDER BY id")?;
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect()
    }

    pub fn add_arch_intent(&self, data: &str) -> rusqlite::Result<u32> {
        self.db
            .execute("INSERT INTO arch_intents (data) VALUES (?)", [data])?;
        Ok(self.db.last_insert_rowid() as u32)
    }

    pub fn delete_arch_intent(&self, id: u32) -> rusqlite::Result<()> {
        self.db
            .execute("DELETE FROM arch_intents WHERE id = ?", [id])?;
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

    #[test]
    fn reviews_are_kept_per_workspace() {
        let store = Store::in_memory();
        store.save_arch_review("", "@", "assess", "main").unwrap();
        store.save_arch_review("w", "@", "assess", "w").unwrap();
        let got = |ws| store.arch_review(ws, "@", "assess").unwrap();
        assert_eq!(got("").as_deref(), Some("main"));
        assert_eq!(got("w").as_deref(), Some("w"));
        assert_eq!(store.arch_review("w", "@", "look").unwrap(), None);
    }

    #[test]
    fn forgetting_a_workspace_drops_its_reviews() {
        let mut store = Store::in_memory();
        store.save_arch_review("", "@", "look", "[]").unwrap();
        store.save_arch_review("w", "@", "look", "[]").unwrap();
        store.forget_workspace("w").unwrap();
        assert!(store.arch_review("", "@", "look").unwrap().is_some());
        assert_eq!(store.arch_review("w", "@", "look").unwrap(), None);
    }

    #[test]
    fn revset_keyed_rows_move_to_the_main_workspace() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA).unwrap();
        db.execute_batch(
            "INSERT INTO arch_state VALUES ('assess:a::b', 'x'), ('look:@', 'y'), ('lens', 'z');",
        )
        .unwrap();
        create_table(&db).unwrap();
        let rows: Vec<(String, String, String, String)> = db
            .prepare("SELECT workspace, revset, kind, data FROM arch_reviews ORDER BY kind")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let row = |r: &str, k: &str, d: &str| ("".into(), r.into(), k.into(), d.into());
        assert_eq!(rows, [row("a::b", "assess", "x"), row("@", "look", "y")]);
        let keys: Vec<String> = db
            .prepare("SELECT key FROM arch_state")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(keys, ["lens"]);
    }
}
