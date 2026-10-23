//! Agent turns and checkpoints: snapshots of a workspace's working copy that diffs start from.

use rusqlite::{OptionalExtension, params};

use super::Store;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS agent_turns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace TEXT NOT NULL,
    session TEXT,
    prompt TEXT,
    change_id TEXT NOT NULL,
    start TEXT NOT NULL,
    end TEXT,
    paths TEXT NOT NULL,
    added INTEGER NOT NULL DEFAULT 0,
    removed INTEGER NOT NULL DEFAULT 0,
    started_at INTEGER NOT NULL,
    ended_at INTEGER
);
CREATE INDEX IF NOT EXISTS agent_turns_workspace ON agent_turns (workspace, change_id);
";

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(SCHEMA)
}

/// A prompt's turn, or a checkpoint when it has no session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Turn {
    pub id: i64,
    pub workspace: String,
    /// The Claude Code session, `None` for a checkpoint.
    pub session: Option<String>,
    pub prompt: Option<String>,
    /// The working copy's change, as of the turn's last snapshot.
    pub change_id: String,
    /// The working-copy commit the turn started from.
    pub start: String,
    /// The working-copy commit it ended on, `None` while it runs and for a checkpoint.
    pub end: Option<String>,
    /// The files its edits touched. Empty for a checkpoint, which diffs every file.
    pub paths: Vec<String>,
    pub added: u32,
    pub removed: u32,
    pub started_at: i64,
    pub ended_at: Option<i64>,
}

const COLUMNS: &str = "id, workspace, session, prompt, change_id, start, end, paths, added, \
                       removed, started_at, ended_at";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Turn> {
    let paths: String = r.get(7)?;
    Ok(Turn {
        id: r.get(0)?,
        workspace: r.get(1)?,
        session: r.get(2)?,
        prompt: r.get(3)?,
        change_id: r.get(4)?,
        start: r.get(5)?,
        end: r.get(6)?,
        paths: serde_json::from_str(&paths).map_err(super::json_err)?,
        added: r.get(8)?,
        removed: r.get(9)?,
        started_at: r.get(10)?,
        ended_at: r.get(11)?,
    })
}

impl Store {
    /// Starts a turn, or a checkpoint without `session`. Returns its id.
    pub fn start_turn(
        &self,
        workspace: &str,
        session: Option<&str>,
        prompt: Option<&str>,
        change_id: &str,
        start: &str,
        at: i64,
    ) -> rusqlite::Result<i64> {
        self.db.execute(
            "INSERT INTO agent_turns (workspace, session, prompt, change_id, start, paths, started_at)
             VALUES (?, ?, ?, ?, ?, '[]', ?)",
            params![workspace, session, prompt, change_id, start, at],
        )?;
        Ok(self.db.last_insert_rowid())
    }

    pub fn turn(&self, id: i64) -> rusqlite::Result<Option<Turn>> {
        self.db
            .query_row(
                &format!("SELECT {COLUMNS} FROM agent_turns WHERE id = ?"),
                [id],
                row,
            )
            .optional()
    }

    /// The session's turn that hasn't ended, if any.
    pub fn running_turn(&self, session: &str) -> rusqlite::Result<Option<Turn>> {
        self.db
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM agent_turns WHERE session = ? AND ended_at IS NULL
                     ORDER BY id DESC LIMIT 1"
                ),
                [session],
                row,
            )
            .optional()
    }

    /// The workspace's turns and checkpoints on `change_id`, newest first.
    pub fn turns_on(&self, workspace: &str, change_id: &str) -> rusqlite::Result<Vec<Turn>> {
        let mut stmt = self.db.prepare(&format!(
            "SELECT {COLUMNS} FROM agent_turns WHERE workspace = ? AND change_id = ?
             ORDER BY started_at DESC, id DESC"
        ))?;
        stmt.query_map([workspace, change_id], row)?.collect()
    }

    /// Adds to the files a turn touched, keeping each once.
    pub fn add_turn_paths(&self, id: i64, paths: &[String]) -> rusqlite::Result<()> {
        let Some(mut turn) = self.turn(id)? else {
            return Ok(());
        };
        for p in paths {
            if !turn.paths.contains(p) {
                turn.paths.push(p.clone());
            }
        }
        let json = serde_json::to_string(&turn.paths).map_err(super::json_err)?;
        self.db.execute(
            "UPDATE agent_turns SET paths = ? WHERE id = ?",
            params![json, id],
        )?;
        Ok(())
    }

    /// Ends a turn on `end`, in `change_id`, with its line counts.
    pub fn end_turn(
        &self,
        id: i64,
        change_id: &str,
        end: &str,
        (added, removed): (u32, u32),
        at: i64,
    ) -> rusqlite::Result<()> {
        self.db.execute(
            "UPDATE agent_turns SET change_id = ?, end = ?, added = ?, removed = ?, ended_at = ?
             WHERE id = ?",
            params![change_id, end, added, removed, at, id],
        )?;
        Ok(())
    }

    pub fn delete_turn(&self, id: i64) -> rusqlite::Result<()> {
        self.db
            .execute("DELETE FROM agent_turns WHERE id = ?", [id])?;
        Ok(())
    }

    /// Drops turns started before `before`.
    pub fn prune_turns(&self, before: i64) -> rusqlite::Result<()> {
        self.db
            .execute("DELETE FROM agent_turns WHERE started_at < ?", [before])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_collects_each_path_once_and_ends_on_its_last_change() {
        let store = Store::in_memory();
        let id = store
            .start_turn("ws", Some("s"), Some("Fix it"), "c1", "a", 1)
            .unwrap();
        assert_eq!(store.running_turn("s").unwrap().unwrap().id, id);
        store
            .add_turn_paths(id, &["x.rs".into(), "y.rs".into()])
            .unwrap();
        store
            .add_turn_paths(id, &["y.rs".into(), "z.rs".into()])
            .unwrap();
        store.end_turn(id, "c2", "b", (3, 1), 2).unwrap();
        let turn = store.turn(id).unwrap().unwrap();
        assert_eq!(turn.paths, ["x.rs", "y.rs", "z.rs"]);
        assert_eq!(
            (turn.end.as_deref(), turn.added, turn.removed),
            (Some("b"), 3, 1)
        );
        assert!(store.running_turn("s").unwrap().is_none());
        assert!(store.turns_on("ws", "c1").unwrap().is_empty());
        assert_eq!(store.turns_on("ws", "c2").unwrap().len(), 1);
    }

    #[test]
    fn turns_list_newest_first_per_workspace() {
        let store = Store::in_memory();
        let old = store.start_turn("ws", None, None, "c", "a", 1).unwrap();
        let new = store
            .start_turn("ws", Some("s"), Some("p"), "c", "b", 2)
            .unwrap();
        store.start_turn("other", None, None, "c", "a", 3).unwrap();
        let ids: Vec<i64> = store
            .turns_on("ws", "c")
            .unwrap()
            .iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(ids, [new, old]);
        store.prune_turns(2).unwrap();
        let ids: Vec<i64> = store
            .turns_on("ws", "c")
            .unwrap()
            .iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(ids, [new]);
    }
}
