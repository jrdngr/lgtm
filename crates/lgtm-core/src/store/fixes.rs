//! Fixes an agent linked to PR threads: spans of its turn's snapshots, each over the files its
//! tools changed. Pruned with the turns.

use rusqlite::params;

use super::Store;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS thread_fixes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace TEXT NOT NULL,
    thread INTEGER NOT NULL,
    turn INTEGER NOT NULL,
    start TEXT NOT NULL,
    end TEXT NOT NULL,
    paths TEXT NOT NULL,
    added INTEGER NOT NULL,
    removed INTEGER NOT NULL,
    made_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS thread_fixes_workspace ON thread_fixes (workspace);
";

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(SCHEMA)
}

/// One thread's link to a span of an agent's turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fix {
    pub id: i64,
    pub workspace: String,
    /// The thread's comment id.
    pub thread: i64,
    /// The agent turn it was made in.
    pub turn: i64,
    /// The working-copy commits the fix runs between.
    pub start: String,
    pub end: String,
    /// The files the turn's tools changed in it.
    pub paths: Vec<String>,
    pub added: u32,
    pub removed: u32,
    pub made_at: i64,
}

const COLUMNS: &str = "id, workspace, thread, turn, start, end, paths, added, removed, made_at";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Fix> {
    let paths: String = r.get(6)?;
    Ok(Fix {
        id: r.get(0)?,
        workspace: r.get(1)?,
        thread: r.get(2)?,
        turn: r.get(3)?,
        start: r.get(4)?,
        end: r.get(5)?,
        paths: serde_json::from_str(&paths).map_err(super::json_err)?,
        added: r.get(7)?,
        removed: r.get(8)?,
        made_at: r.get(9)?,
    })
}

impl Store {
    /// Links `fix` (its id ignored) to its thread. Returns the new id.
    pub fn add_fix(&self, fix: &Fix) -> rusqlite::Result<i64> {
        let paths = serde_json::to_string(&fix.paths).map_err(super::json_err)?;
        self.db.execute(
            "INSERT INTO thread_fixes (workspace, thread, turn, start, end, paths, added, removed,
             made_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                fix.workspace,
                fix.thread,
                fix.turn,
                fix.start,
                fix.end,
                paths,
                fix.added,
                fix.removed,
                fix.made_at
            ],
        )?;
        Ok(self.db.last_insert_rowid())
    }

    pub fn fix(&self, id: i64) -> rusqlite::Result<Option<Fix>> {
        use rusqlite::OptionalExtension;
        self.db
            .query_row(
                &format!("SELECT {COLUMNS} FROM thread_fixes WHERE id = ?"),
                [id],
                row,
            )
            .optional()
    }

    /// The workspace's fixes, oldest first.
    pub fn fixes(&self, workspace: &str) -> rusqlite::Result<Vec<Fix>> {
        let mut stmt = self.db.prepare(&format!(
            "SELECT {COLUMNS} FROM thread_fixes WHERE workspace = ? ORDER BY made_at, id"
        ))?;
        stmt.query_map([workspace], row)?.collect()
    }

    /// Where the last fix made in `turn` ended, if any.
    pub fn last_fix_end(&self, turn: i64) -> rusqlite::Result<Option<String>> {
        use rusqlite::OptionalExtension;
        self.db
            .query_row(
                "SELECT end FROM thread_fixes WHERE turn = ? ORDER BY made_at DESC, id DESC LIMIT 1",
                [turn],
                |r| r.get(0),
            )
            .optional()
    }

    /// Drops fixes made before `before`.
    pub fn prune_fixes(&self, before: i64) -> rusqlite::Result<()> {
        self.db
            .execute("DELETE FROM thread_fixes WHERE made_at < ?", [before])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fix(thread: i64, turn: i64, end: &str, at: i64) -> Fix {
        Fix {
            id: 0,
            workspace: "ws".into(),
            thread,
            turn,
            start: "a".into(),
            end: end.into(),
            paths: vec!["a.rs".into()],
            added: 1,
            removed: 1,
            made_at: at,
        }
    }

    #[test]
    fn the_next_fix_in_a_turn_starts_where_the_last_one_ended_and_old_ones_are_pruned() {
        let store = Store::in_memory();
        store.add_fix(&fix(1, 7, "b", 10)).unwrap();
        store.add_fix(&fix(2, 7, "c", 20)).unwrap();
        store.add_fix(&fix(3, 8, "d", 30)).unwrap();
        assert_eq!(store.last_fix_end(7).unwrap().as_deref(), Some("c"));
        assert_eq!(store.last_fix_end(9).unwrap(), None);

        store.prune_fixes(15).unwrap();
        let threads: Vec<i64> = store
            .fixes("ws")
            .unwrap()
            .iter()
            .map(|f| f.thread)
            .collect();
        assert_eq!(threads, [2, 3]);
    }
}
