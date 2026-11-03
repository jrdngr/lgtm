//! Comments the user leaves on code for an agent to read, numbered C1, C2, … per repo. An
//! agent marks one done once it has dealt with it.

use rusqlite::{Row, params};

use super::{Store, enum_from, enum_to};
use crate::anchor::Side;

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS agent_comments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            number INTEGER NOT NULL,
            path TEXT NOT NULL,
            side TEXT NOT NULL,
            line INTEGER NOT NULL,
            line_text TEXT NOT NULL,
            body TEXT NOT NULL,
            review TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            done INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS agent_comments_path ON agent_comments (path);",
    )?;
    for (name, def) in [("start_line", "INTEGER"), ("start_text", "TEXT")] {
        let has: bool = db.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('agent_comments') WHERE name = ?",
            [name],
            |r| r.get(0),
        )?;
        if !has {
            db.execute_batch(&format!(
                "ALTER TABLE agent_comments ADD COLUMN {name} {def};"
            ))?;
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentComment {
    pub id: i64,
    /// Shown as `C<number>`.
    pub number: i64,
    pub path: String,
    pub side: Side,
    /// 1-based line on `side` when the comment was made.
    pub line: u32,
    /// The line's code then, which finds it again after edits.
    pub line_text: String,
    /// For a comment on a range of lines, its first line on `side` and that line's code;
    /// `line` is its last.
    pub start: Option<(u32, String)>,
    pub body: String,
    /// The review it was made in, e.g. its revset.
    pub review: String,
    /// Unix milliseconds.
    pub created_at: i64,
    pub done: bool,
}

const COLUMNS: &str = "id, number, path, side, line, line_text, body, review, created_at, done, start_line, start_text";

fn row_to_comment(r: &Row) -> rusqlite::Result<AgentComment> {
    Ok(AgentComment {
        id: r.get(0)?,
        number: r.get(1)?,
        path: r.get(2)?,
        side: enum_from(r.get(3)?)?,
        line: r.get(4)?,
        line_text: r.get(5)?,
        body: r.get(6)?,
        review: r.get(7)?,
        created_at: r.get(8)?,
        done: r.get(9)?,
        start: r
            .get::<_, Option<u32>>(10)?
            .zip(r.get::<_, Option<String>>(11)?),
    })
}

impl Store {
    /// Stores `c` under the next number. Returns it with its id and number.
    pub fn add_agent_comment(&self, c: &AgentComment) -> rusqlite::Result<AgentComment> {
        let number = self.next_agent_comment_number()?;
        self.db.execute(
            "INSERT INTO agent_comments
             (number, path, side, line, line_text, body, review, created_at, done,
              start_line, start_text)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                number,
                c.path,
                enum_to(&c.side),
                c.line,
                c.line_text,
                c.body,
                c.review,
                c.created_at,
                c.done,
                c.start.as_ref().map(|s| s.0),
                c.start.as_ref().map(|s| s.1.as_str()),
            ],
        )?;
        Ok(AgentComment {
            id: self.db.last_insert_rowid(),
            number,
            ..c.clone()
        })
    }

    /// The number the next comment will get.
    pub fn next_agent_comment_number(&self) -> rusqlite::Result<i64> {
        self.db.query_row(
            "SELECT COALESCE(MAX(number), 0) + 1 FROM agent_comments",
            [],
            |r| r.get(0),
        )
    }

    /// Comments on any of `paths`, oldest first.
    pub fn agent_comments_for_paths(&self, paths: &[&str]) -> rusqlite::Result<Vec<AgentComment>> {
        let mut found = self.select_by_paths("agent_comments", COLUMNS, paths, row_to_comment)?;
        found.sort_by_key(|c| c.number);
        Ok(found)
    }

    /// Sets whether comment `C<number>` is done. Returns whether there is one.
    pub fn set_agent_comment_done(&self, number: i64, done: bool) -> rusqlite::Result<bool> {
        let n = self.db.execute(
            "UPDATE agent_comments SET done = ? WHERE number = ?",
            params![done, number],
        )?;
        Ok(n > 0)
    }

    /// Replaces comment `C<number>`'s text.
    pub fn set_agent_comment_body(&self, number: i64, body: &str) -> rusqlite::Result<()> {
        self.db.execute(
            "UPDATE agent_comments SET body = ? WHERE number = ?",
            params![body, number],
        )?;
        Ok(())
    }

    pub fn delete_agent_comment(&self, number: i64) -> rusqlite::Result<()> {
        self.db
            .execute("DELETE FROM agent_comments WHERE number = ?", [number])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comment(path: &str) -> AgentComment {
        AgentComment {
            id: 0,
            number: 0,
            path: path.into(),
            side: Side::New,
            line: 3,
            line_text: "let x = 1;".into(),
            start: None,
            body: "why 1?".into(),
            review: "@".into(),
            created_at: 1,
            done: false,
        }
    }

    #[test]
    fn comments_are_numbered_in_order_and_marked_done_by_number() {
        let store = Store::in_memory();
        let first = store.add_agent_comment(&comment("a.rs")).unwrap();
        let ranged = AgentComment {
            start: Some((1, "fn a() {".into())),
            ..comment("b.rs")
        };
        let second = store.add_agent_comment(&ranged).unwrap();
        assert_eq!((first.number, second.number), (1, 2));
        assert!(store.set_agent_comment_done(2, true).unwrap());
        assert!(!store.set_agent_comment_done(9, true).unwrap());
        let found = store.agent_comments_for_paths(&["a.rs", "b.rs"]).unwrap();
        assert_eq!(
            found.iter().map(|c| (c.number, c.done)).collect::<Vec<_>>(),
            [(1, false), (2, true)]
        );
        assert_eq!(found[1].start, Some((1, "fn a() {".into())));
        store.set_agent_comment_body(1, "why 2?").unwrap();
        assert_eq!(store.agent_comments_for_paths(&["a.rs"]).unwrap()[0].body, "why 2?");
        // A deleted comment's number isn't reused while a later one exists.
        store.delete_agent_comment(1).unwrap();
        assert_eq!(store.add_agent_comment(&comment("a.rs")).unwrap().number, 3);
    }
}
