//! Review comments: local notes, and GitHub threads and drafts of them. A comment is anchored
//! like a mark, on the first and last changed rows of the selection it was made on, and keeps
//! the replies terminal agents gave on it.

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::threads::{ThreadState, thread_from_sql, thread_to_sql};
use super::{Store, json_err};
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
    /// The path on the pin's side, which differs from its comment's across a rename. Pins saved
    /// before this have none and use their comment's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl Pin {
    pub fn new(anchor: &Anchor, commit: String, line: u32, path: Option<String>) -> Self {
        Self {
            side: anchor.side,
            line_hash: anchor.line_hash,
            above: anchor.above.clone(),
            below: anchor.below.clone(),
            commit,
            line,
            path,
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
}

/// The conversation inside a comment block. Stored in the `workshop` column.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conversation {
    pub messages: Vec<Message>,
    /// A reply a synced terminal agent suggested, for the user to use or not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

/// A post in a GitHub review thread.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GhPost {
    pub author: String,
    pub body: String,
    pub created_at: String,
    /// In the viewer's review, which isn't submitted yet.
    pub pending: bool,
}

/// The GitHub thread a comment is on: a review thread on code, or a comment or review summary
/// on the PR, whose comment's path is empty. The comment's body is then a reply not yet sent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GhLink {
    /// `owner/name` of the PR's repo.
    pub repo: String,
    pub pr: u32,
    pub thread: String,
    pub url: String,
    pub resolved: bool,
    pub outdated: bool,
    pub posts: Vec<GhPost>,
    /// The thread's last line on the PR head, while it isn't outdated.
    #[serde(default)]
    pub line: Option<u32>,
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
    pub conversation: Conversation,
    pub github: Option<GhLink>,
    /// Set on threads and drafts of them; notes have none.
    pub thread: Option<ThreadState>,
    /// A note the user is done with.
    pub done: bool,
}

impl Comment {
    /// A new thread on code not yet sent to GitHub.
    pub fn is_draft(&self) -> bool {
        self.github.is_none() && self.thread.as_ref().is_some_and(|t| t.quote.is_none())
    }

    /// The part of a comment on the PR this thread answers.
    pub fn quote(&self) -> Option<&super::Quote> {
        self.thread.as_ref().and_then(|t| t.quote.as_ref())
    }
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS comments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL,
    pins TEXT NOT NULL,
    body TEXT NOT NULL,
    snippet TEXT NOT NULL,
    review TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    workshop TEXT NOT NULL,
    github TEXT NOT NULL DEFAULT '',
    thread TEXT NOT NULL DEFAULT '',
    done INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS comments_path ON comments (path);
";

/// Columns added after the table first shipped, for stores made before them.
const ADDED: [(&str, &str); 3] = [
    ("github", "TEXT NOT NULL DEFAULT ''"),
    ("thread", "TEXT NOT NULL DEFAULT ''"),
    ("done", "INTEGER NOT NULL DEFAULT 0"),
];

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(SCHEMA)?;
    for (name, def) in ADDED {
        let has: bool = db.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('comments') WHERE name = ?",
            [name],
            |r| r.get(0),
        )?;
        if !has {
            db.execute_batch(&format!("ALTER TABLE comments ADD COLUMN {name} {def};"))?;
        }
    }
    Ok(())
}

pub(super) const COLUMNS: &str =
    "id, path, pins, body, snippet, review, created_at, updated_at, workshop, github, thread, done";

fn github_to_sql(g: &Option<GhLink>) -> rusqlite::Result<String> {
    match g {
        Some(g) => serde_json::to_string(g).map_err(json_err),
        None => Ok(String::new()),
    }
}

fn github_from_sql(s: String) -> rusqlite::Result<Option<GhLink>> {
    if s.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(&s).map(Some).map_err(json_err)
}

pub(super) fn row_to_comment(r: &rusqlite::Row) -> rusqlite::Result<Comment> {
    let github = github_from_sql(r.get(9)?)?;
    let thread = thread_from_sql(r.get(10)?, github.as_ref())?;
    Ok(Comment {
        id: r.get(0)?,
        path: r.get(1)?,
        pins: serde_json::from_str(&r.get::<_, String>(2)?).map_err(json_err)?,
        body: r.get(3)?,
        snippet: r.get(4)?,
        review: r.get(5)?,
        created_at: r.get(6)?,
        updated_at: r.get(7)?,
        conversation: serde_json::from_str(&r.get::<_, String>(8)?).map_err(json_err)?,
        github,
        thread,
        done: r.get(11)?,
    })
}

impl Store {
    pub fn comments_for_paths(&self, paths: &[&str]) -> rusqlite::Result<Vec<Comment>> {
        self.select_by_paths("comments", COLUMNS, paths, row_to_comment)
    }

    pub fn comment(&self, id: i64) -> rusqlite::Result<Option<Comment>> {
        let sql = format!("SELECT {COLUMNS} FROM comments WHERE id = ?");
        self.db.query_row(&sql, [id], row_to_comment).optional()
    }

    /// Inserts a comment and returns its id.
    pub fn add_comment(&self, c: &Comment) -> rusqlite::Result<i64> {
        self.db.execute(
            "INSERT INTO comments (path, pins, body, snippet, review, created_at, updated_at, workshop, github, thread, done)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                c.path,
                serde_json::to_string(&c.pins).map_err(json_err)?,
                c.body,
                c.snippet,
                c.review,
                c.created_at,
                c.updated_at,
                serde_json::to_string(&c.conversation).map_err(json_err)?,
                github_to_sql(&c.github)?,
                thread_to_sql(&c.thread)?,
                c.done,
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

    pub fn set_conversation(&self, id: i64, conversation: &Conversation) -> rusqlite::Result<()> {
        self.db.execute(
            "UPDATE comments SET workshop = ? WHERE id = ?",
            params![serde_json::to_string(conversation).map_err(json_err)?, id],
        )?;
        Ok(())
    }

    pub fn set_comment_pins(
        &self,
        id: i64,
        path: &str,
        pins: &[Pin],
        snippet: &str,
    ) -> rusqlite::Result<()> {
        self.db.execute(
            "UPDATE comments SET path = ?, pins = ?, snippet = ? WHERE id = ?",
            params![
                path,
                serde_json::to_string(pins).map_err(json_err)?,
                snippet,
                id
            ],
        )?;
        Ok(())
    }

    /// Empties the comment's body if it's still `body`, so text saved since isn't lost.
    pub fn clear_body_if(&self, id: i64, body: &str, at: i64) -> rusqlite::Result<()> {
        self.db.execute(
            "UPDATE comments SET body = '', updated_at = ? WHERE id = ? AND body = ?",
            params![at, id, body],
        )?;
        Ok(())
    }

    pub fn set_github(&self, id: i64, github: &Option<GhLink>) -> rusqlite::Result<()> {
        self.db.execute(
            "UPDATE comments SET github = ? WHERE id = ?",
            params![github_to_sql(github)?, id],
        )?;
        Ok(())
    }

    pub fn set_comment_done(&self, id: i64, done: bool) -> rusqlite::Result<()> {
        self.db.execute(
            "UPDATE comments SET done = ? WHERE id = ?",
            params![done, id],
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
    use crate::store::threads::{Stage, TakeVerdict};

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
            path: None,
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
            conversation: Conversation::default(),
            github: None,
            thread: None,
            done: false,
        };
        c.id = store.add_comment(&c).unwrap();
        assert_eq!(store.comments_for_paths(&["a.rs"]).unwrap(), [c.clone()]);

        store.set_comment_body(c.id, "because", 2).unwrap();
        let conversation = Conversation {
            messages: vec![Message {
                author: Author::Agent,
                text: "hi".into(),
            }],
            suggestion: Some("ok".into()),
        };
        store.set_conversation(c.id, &conversation).unwrap();
        let got = store.comment(c.id).unwrap().unwrap();
        assert_eq!((got.body.as_str(), got.updated_at), ("because", 2));
        assert_eq!(got.conversation, conversation);

        let link = Some(GhLink {
            repo: "o/n".into(),
            pr: 3,
            thread: "T".into(),
            url: String::new(),
            resolved: false,
            outdated: false,
            posts: vec![],
            line: Some(4),
        });
        store.set_github(c.id, &link).unwrap();
        // A thread pulled before thread states existed gets one.
        let got = &store.pr_threads("o/n", 3).unwrap()[0];
        assert_eq!(got.github, link);
        assert_eq!(got.thread, Some(ThreadState::new("o/n", 3, Stage::New)));
        assert!(store.pr_threads("o/n", 4).unwrap().is_empty());

        let mut thread = ThreadState::new("o/n", 3, Stage::Ready);
        thread.summary = "Clamp".into();
        store.set_thread(c.id, &Some(thread.clone())).unwrap();
        store.set_comment_done(c.id, true).unwrap();
        let got = store.comment(c.id).unwrap().unwrap();
        assert_eq!((got.thread, got.done), (Some(thread), true));

        store.delete_comment(c.id).unwrap();
        assert_eq!(store.comment(c.id).unwrap(), None);
    }

    #[test]
    fn old_comment_tables_stop_reusing_deleted_ids() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        let old = SCHEMA
            .replace(" AUTOINCREMENT", "")
            .replace(",\n    github TEXT NOT NULL DEFAULT '',\n    thread TEXT NOT NULL DEFAULT '',\n    done INTEGER NOT NULL DEFAULT 0", "");
        db.execute_batch(&old).unwrap();
        db.execute_batch(
            r#"INSERT INTO comments VALUES (1, 'a.rs', '[]', 'one', '', '@', 0, 0, '{"messages":[]}'),
                                           (2, 'a.rs', '[]', 'two', '', '@', 0, 0, '{"messages":[]}');"#,
        )
        .unwrap();
        let store = Store::init(db).unwrap();
        let mut c = store.comment(2).unwrap().unwrap();
        assert_eq!(c.body, "two");
        store.delete_comment(2).unwrap();
        c.body = "three".into();
        assert_eq!(store.add_comment(&c).unwrap(), 3);
        let index: bool = store
            .db
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE name = 'comments_path'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(index);
    }

    #[test]
    fn takes_saved_under_the_old_verdict_names_are_renamed() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA).unwrap();
        for (id, verdict) in [(1, "agreeWithChanges"), (2, "pushBack"), (3, "needsYou")] {
            let thread = format!(
                r#"{{"repo":"o/n","pr":3,"stage":"ready","take":{{"verdict":"{verdict}","why":"w","question":null}},"summary":""}}"#
            );
            db.execute(
                r#"INSERT INTO comments VALUES (?, 'a.rs', '[]', 'b', '', '@', 0, 0, '{"messages":[]}', '', ?, 0)"#,
                rusqlite::params![id, thread],
            )
            .unwrap();
        }
        let store = Store::init(db).unwrap();
        let verdicts: Vec<TakeVerdict> = (1..=3)
            .map(|id| {
                store
                    .comment(id)
                    .unwrap()
                    .unwrap()
                    .thread
                    .unwrap()
                    .take
                    .unwrap()
                    .verdict
            })
            .collect();
        use TakeVerdict::*;
        assert_eq!(verdicts, [Change, Disagree, Decide]);
    }
}
