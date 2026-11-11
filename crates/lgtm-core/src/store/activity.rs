//! What a running agent turn does, for watching it: each edit's line counts and the agent's
//! todo list.

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::Store;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS turn_edits (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    turn_id INTEGER NOT NULL,
    at INTEGER NOT NULL,
    added INTEGER NOT NULL,
    removed INTEGER NOT NULL,
    rewritten INTEGER NOT NULL,
    paths TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS turn_edits_turn ON turn_edits (turn_id);
CREATE TABLE IF NOT EXISTS agent_todos (
    session TEXT PRIMARY KEY,
    todos TEXT NOT NULL,
    at INTEGER NOT NULL
);
";

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(SCHEMA)
}

/// One tool call's change to the files, within a turn.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TurnEdit {
    /// When it finished (Unix ms).
    #[specta(type = f64)]
    pub at: i64,
    pub added: u32,
    pub removed: u32,
    /// Removed lines the same turn had added: work the agent redid.
    pub rewritten: u32,
    pub paths: Vec<String>,
}

/// An item of the agent's todo list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Todo {
    pub content: String,
    pub status: TodoStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

/// What a workspace's newest agent turn has done so far.
#[derive(Clone, Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentActivity {
    pub prompt: Option<String>,
    /// The terminal panel the agent runs in.
    pub terminal: Option<String>,
    pub running: bool,
    /// The working-copy commit the turn started from.
    #[serde(skip)]
    pub start: String,
    /// When the turn started and ended (Unix ms).
    #[specta(type = f64)]
    pub started_at: i64,
    #[specta(type = Option<f64>)]
    pub ended_at: Option<i64>,
    /// The turn's edits, oldest first.
    pub edits: Vec<TurnEdit>,
    /// The session's last todo list.
    pub todos: Vec<Todo>,
}

impl Store {
    /// The workspace's newest agent turn, if it has one.
    pub fn agent_activity(&self, workspace: &str) -> rusqlite::Result<Option<AgentActivity>> {
        let Some(turn) = self.latest_agent_turn(workspace)? else {
            return Ok(None);
        };
        let todos = match &turn.session {
            Some(s) => self.todos(s)?.unwrap_or_default(),
            None => vec![],
        };
        Ok(Some(AgentActivity {
            prompt: turn.prompt,
            terminal: turn.terminal,
            running: turn.ended_at.is_none(),
            start: turn.start,
            started_at: turn.started_at,
            ended_at: turn.ended_at,
            edits: self.turn_edits(turn.id)?,
            todos,
        }))
    }

    pub fn add_turn_edit(&self, turn: i64, edit: &TurnEdit) -> rusqlite::Result<()> {
        let paths = serde_json::to_string(&edit.paths).map_err(super::json_err)?;
        self.db.execute(
            "INSERT INTO turn_edits (turn_id, at, added, removed, rewritten, paths)
             VALUES (?, ?, ?, ?, ?, ?)",
            params![
                turn,
                edit.at,
                edit.added,
                edit.removed,
                edit.rewritten,
                paths
            ],
        )?;
        Ok(())
    }

    /// A turn's edits, oldest first.
    pub fn turn_edits(&self, turn: i64) -> rusqlite::Result<Vec<TurnEdit>> {
        let mut stmt = self.db.prepare(
            "SELECT at, added, removed, rewritten, paths FROM turn_edits WHERE turn_id = ?
             ORDER BY id",
        )?;
        stmt.query_map([turn], |r| {
            let paths: String = r.get(4)?;
            Ok(TurnEdit {
                at: r.get(0)?,
                added: r.get(1)?,
                removed: r.get(2)?,
                rewritten: r.get(3)?,
                paths: serde_json::from_str(&paths).map_err(super::json_err)?,
            })
        })?
        .collect()
    }

    /// Keeps `todos` as the session's list, replacing the last one.
    pub fn set_todos(&self, session: &str, todos: &[Todo], at: i64) -> rusqlite::Result<()> {
        let json = serde_json::to_string(todos).map_err(super::json_err)?;
        self.db.execute(
            "INSERT OR REPLACE INTO agent_todos (session, todos, at) VALUES (?, ?, ?)",
            params![session, json, at],
        )?;
        Ok(())
    }

    /// The session's last todo list, if it wrote one.
    pub fn todos(&self, session: &str) -> rusqlite::Result<Option<Vec<Todo>>> {
        let json: Option<String> = self
            .db
            .query_row(
                "SELECT todos FROM agent_todos WHERE session = ?",
                [session],
                |r| r.get(0),
            )
            .optional()?;
        json.map(|j| serde_json::from_str(&j).map_err(super::json_err))
            .transpose()
    }

    /// Drops edits of turns that are gone and todo lists older than `before`.
    pub fn prune_activity(&self, before: i64) -> rusqlite::Result<()> {
        self.db.execute(
            "DELETE FROM turn_edits WHERE turn_id NOT IN (SELECT id FROM agent_turns)",
            [],
        )?;
        self.db
            .execute("DELETE FROM agent_todos WHERE at < ?", [before])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_stay_with_their_turn_and_go_with_it() {
        let s = Store::in_memory();
        let turn = s.start_turn("", Some("s"), None, "c", "x", 1).unwrap();
        let edit = |added| TurnEdit {
            at: 2,
            added,
            removed: 1,
            rewritten: 0,
            paths: vec!["a.rs".into()],
        };
        s.add_turn_edit(turn, &edit(3)).unwrap();
        s.add_turn_edit(turn, &edit(5)).unwrap();
        assert_eq!(s.turn_edits(turn).unwrap(), [edit(3), edit(5)]);
        s.delete_turn(turn).unwrap();
        s.prune_activity(0).unwrap();
        assert!(s.turn_edits(turn).unwrap().is_empty());
    }

    #[test]
    fn a_sessions_todo_list_is_its_latest_until_it_ages_out() {
        let s = Store::in_memory();
        let todo = |status| Todo {
            content: "Run the tests".into(),
            status,
        };
        s.set_todos("s", &[todo(TodoStatus::Pending)], 1).unwrap();
        s.set_todos("s", &[todo(TodoStatus::Completed)], 5).unwrap();
        assert_eq!(
            s.todos("s").unwrap(),
            Some(vec![todo(TodoStatus::Completed)])
        );
        s.prune_activity(10).unwrap();
        assert_eq!(s.todos("s").unwrap(), None);
    }
}
