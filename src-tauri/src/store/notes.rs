//! Walkthrough notes: an agent's guide to a review. Each note is anchored like a comment and
//! has a category, a place in the reading order, and maybe a step in a named flow. Kept per
//! review, where each new walkthrough replaces the last.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::Store;
use super::comments::Pin;
use super::{enum_from, enum_to, json_err};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    High,
    Medium,
    Low,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    /// Needs a careful look. Has a level.
    Important,
    /// A step in a flow through several places.
    Flow,
    /// Intent or background the diff doesn't show.
    Context,
    /// Something for the reviewer or author to settle.
    Question,
}

/// Which agent wrote a note.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// The app's own headless run.
    Headless,
    /// An agent in a terminal, through the MCP server.
    Terminal,
}

/// Where a note sits in a flow.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FlowStep {
    pub flow: String,
    /// 1-based.
    pub step: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub id: i64,
    pub path: String,
    /// First and last changed rows of the range. One pin when they're the same row.
    pub pins: Vec<Pin>,
    pub category: Category,
    /// Only for important notes.
    pub level: Option<Level>,
    pub title: String,
    pub body: String,
    pub flow: Option<FlowStep>,
    /// Its place in the reading order, from 0.
    pub position: u32,
    pub review: String,
    pub source: Source,
    pub created_at: i64,
}

const SCHEMA: &str = "
DROP TABLE IF EXISTS hotspots;
CREATE TABLE IF NOT EXISTS notes (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL,
    pins TEXT NOT NULL,
    category TEXT NOT NULL,
    level TEXT,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    flow TEXT,
    position INTEGER NOT NULL,
    review TEXT NOT NULL,
    source TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS notes_review ON notes (review);
";

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(SCHEMA)
}

const COLUMNS: &str =
    "id, path, pins, category, level, title, body, flow, position, review, source, created_at";

fn row_to_note(r: &rusqlite::Row) -> rusqlite::Result<Note> {
    let flow: Option<String> = r.get(7)?;
    Ok(Note {
        id: r.get(0)?,
        path: r.get(1)?,
        pins: serde_json::from_str(&r.get::<_, String>(2)?).map_err(json_err)?,
        category: enum_from(r.get(3)?)?,
        level: r.get::<_, Option<String>>(4)?.map(enum_from).transpose()?,
        title: r.get(5)?,
        body: r.get(6)?,
        flow: flow
            .map(|f| serde_json::from_str(&f).map_err(json_err))
            .transpose()?,
        position: r.get(8)?,
        review: r.get(9)?,
        source: enum_from(r.get(10)?)?,
        created_at: r.get(11)?,
    })
}

impl Store {
    pub fn notes(&self, review: &str) -> rusqlite::Result<Vec<Note>> {
        self.select_by_review("notes", COLUMNS, review, row_to_note)
    }

    /// Replaces `review`'s notes with `notes`.
    pub fn replace_notes(&mut self, review: &str, notes: &[Note]) -> rusqlite::Result<()> {
        let tx = self.db.transaction()?;
        tx.execute("DELETE FROM notes WHERE review = ?", [review])?;
        for n in notes {
            let flow = n
                .flow
                .as_ref()
                .map(|f| serde_json::to_string(f).map_err(json_err))
                .transpose()?;
            tx.execute(
                "INSERT INTO notes (path, pins, category, level, title, body, flow, position,
                     review, source, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    n.path,
                    serde_json::to_string(&n.pins).map_err(json_err)?,
                    enum_to(&n.category),
                    n.level.as_ref().map(enum_to),
                    n.title,
                    n.body,
                    flow,
                    n.position,
                    review,
                    enum_to(&n.source),
                    n.created_at,
                ],
            )?;
        }
        tx.commit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::Side;

    #[test]
    fn a_new_walkthrough_replaces_the_reviews_notes() {
        let mut store = Store::in_memory();
        let pin = Pin {
            side: Side::New,
            line_hash: 1,
            above: vec![],
            below: vec![2],
            commit: "c".into(),
            line: 3,
            path: None,
        };
        let n = |title: &str, review: &str| Note {
            id: 0,
            path: "a.rs".into(),
            pins: vec![pin.clone()],
            category: Category::Flow,
            level: None,
            title: title.into(),
            body: "body".into(),
            flow: Some(FlowStep {
                flow: "Login".into(),
                step: 2,
            }),
            position: 4,
            review: review.into(),
            source: Source::Terminal,
            created_at: 5,
        };
        store.replace_notes("@", &[n("old", "@")]).unwrap();
        store.replace_notes("x", &[n("other", "x")]).unwrap();
        let mut important = n("new", "@");
        important.category = Category::Important;
        important.level = Some(Level::High);
        store
            .replace_notes("@", &[n("new", "@"), important.clone()])
            .unwrap();
        let mut got = store.notes("@").unwrap();
        for g in &mut got {
            g.id = 0;
        }
        assert_eq!(got, vec![n("new", "@"), important]);
        assert_eq!(store.notes("x").unwrap().len(), 1);
    }
}
