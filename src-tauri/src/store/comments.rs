//! Local review comments. A comment is anchored like a mark, on the first and last changed
//! rows of the selection it was made on, and keeps its workshop conversation with an agent.

use rusqlite::{OptionalExtension, params, params_from_iter};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::Store;
use crate::anchor::{Anchor, Side};

/// An anchor with the file side it was taken on, as for a mark.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pin {
    pub side: Side,
    pub line_hash: i64,
    pub above: Vec<i64>,
    pub below: Vec<i64>,
    pub commit: String,
    pub line: u32,
}

impl Pin {
    pub fn new(anchor: &Anchor, commit: String, line: u32) -> Self {
        Self {
            side: anchor.side,
            line_hash: anchor.line_hash,
            above: anchor.above.clone(),
            below: anchor.below.clone(),
            commit,
            line,
        }
    }

    pub fn anchor(&self, path: &str) -> Anchor {
        Anchor {
            path: path.to_string(),
            side: self.side,
            line_hash: self.line_hash,
            above: self.above.clone(),
            below: self.below.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Author {
    User,
    Agent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Message {
    pub author: Author,
    pub text: String,
    /// A user message that asked for a change, which the agent could make by editing files.
    #[serde(default)]
    pub change: bool,
}

/// The agent conversation inside a comment block.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workshop {
    /// The agent's session, resumed by each follow-up.
    pub session: Option<String>,
    pub messages: Vec<Message>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comment {
    pub id: i64,
    pub path: String,
    /// First and last changed rows of the selection. One pin when they're the same row.
    pub pins: Vec<Pin>,
    pub body: String,
    /// The commented code as it was, for when it can't be found any more.
    pub snippet: String,
    pub review: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub workshop: Workshop,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS comments (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL,
    pins TEXT NOT NULL,
    body TEXT NOT NULL,
    snippet TEXT NOT NULL,
    review TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    workshop TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS comments_path ON comments (path);
";

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(SCHEMA)
}

const COLUMNS: &str = "id, path, pins, body, snippet, review, created_at, updated_at, workshop";

fn json_err(e: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, e.into())
}

fn row_to_comment(r: &rusqlite::Row) -> rusqlite::Result<Comment> {
    Ok(Comment {
        id: r.get(0)?,
        path: r.get(1)?,
        pins: serde_json::from_str(&r.get::<_, String>(2)?).map_err(json_err)?,
        body: r.get(3)?,
        snippet: r.get(4)?,
        review: r.get(5)?,
        created_at: r.get(6)?,
        updated_at: r.get(7)?,
        workshop: serde_json::from_str(&r.get::<_, String>(8)?).map_err(json_err)?,
    })
}

impl Store {
    pub fn comments_for_paths(&self, paths: &[&str]) -> rusqlite::Result<Vec<Comment>> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = vec!["?"; paths.len()].join(",");
        let sql = format!("SELECT {COLUMNS} FROM comments WHERE path IN ({placeholders})");
        let mut stmt = self.db.prepare(&sql)?;
        stmt.query_map(params_from_iter(paths), row_to_comment)?
            .collect()
    }

    pub fn comment(&self, id: i64) -> rusqlite::Result<Option<Comment>> {
        let sql = format!("SELECT {COLUMNS} FROM comments WHERE id = ?");
        self.db.query_row(&sql, [id], row_to_comment).optional()
    }

    /// Inserts a comment and returns its id.
    pub fn add_comment(&self, c: &Comment) -> rusqlite::Result<i64> {
        self.db.execute(
            "INSERT INTO comments (path, pins, body, snippet, review, created_at, updated_at, workshop)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                c.path,
                serde_json::to_string(&c.pins).map_err(json_err)?,
                c.body,
                c.snippet,
                c.review,
                c.created_at,
                c.updated_at,
                serde_json::to_string(&c.workshop).map_err(json_err)?,
            ],
        )?;
        Ok(self.db.last_insert_rowid())
    }

    pub fn set_comment_body(&self, id: i64, body: &str, at: i64) -> rusqlite::Result<()> {
        self.db.execute(
            "UPDATE comments SET body = ?, updated_at = ? WHERE id = ?",
            params![body, at, id],
        )?;
        Ok(())
    }

    pub fn set_workshop(&self, id: i64, workshop: &Workshop) -> rusqlite::Result<()> {
        self.db.execute(
            "UPDATE comments SET workshop = ? WHERE id = ?",
            params![serde_json::to_string(workshop).map_err(json_err)?, id],
        )?;
        Ok(())
    }

    pub fn delete_comment(&self, id: i64) -> rusqlite::Result<()> {
        self.db.execute("DELETE FROM comments WHERE id = ?", [id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_round_trip() {
        let store = Store::in_memory();
        let pin = Pin {
            side: Side::New,
            line_hash: -3,
            above: vec![1],
            below: vec![],
            commit: "c".into(),
            line: 4,
        };
        let mut c = Comment {
            id: 0,
            path: "a.rs".into(),
            pins: vec![pin],
            body: "why?".into(),
            snippet: "+x".into(),
            review: "@".into(),
            created_at: 1,
            updated_at: 1,
            workshop: Workshop::default(),
        };
        c.id = store.add_comment(&c).unwrap();
        assert_eq!(store.comments_for_paths(&["a.rs"]).unwrap(), [c.clone()]);

        store.set_comment_body(c.id, "because", 2).unwrap();
        let workshop = Workshop {
            session: Some("s".into()),
            messages: vec![Message {
                author: Author::Agent,
                text: "hi".into(),
                change: false,
            }],
        };
        store.set_workshop(c.id, &workshop).unwrap();
        let got = store.comment(c.id).unwrap().unwrap();
        assert_eq!((got.body.as_str(), got.updated_at), ("because", 2));
        assert_eq!(got.workshop, workshop);

        store.delete_comment(c.id).unwrap();
        assert_eq!(store.comment(c.id).unwrap(), None);
    }
}
