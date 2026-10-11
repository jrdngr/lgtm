//! Hotspots: the parts of a review an agent thinks most need a careful look. Anchored like
//! comments, and kept per review, where each new pass replaces the last.

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

/// Which agent found a hotspot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// The app's own headless run.
    Headless,
    /// An agent in a terminal, through the MCP server.
    Terminal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hotspot {
    pub id: i64,
    pub path: String,
    /// First and last changed rows of the range. One pin when they're the same row.
    pub pins: Vec<Pin>,
    pub level: Level,
    pub reason: String,
    pub review: String,
    pub source: Source,
    pub created_at: i64,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS hotspots (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL,
    pins TEXT NOT NULL,
    level TEXT NOT NULL,
    reason TEXT NOT NULL,
    review TEXT NOT NULL,
    source TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS hotspots_review ON hotspots (review);
";

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(SCHEMA)
}

const COLUMNS: &str = "id, path, pins, level, reason, review, source, created_at";

fn row_to_hotspot(r: &rusqlite::Row) -> rusqlite::Result<Hotspot> {
    Ok(Hotspot {
        id: r.get(0)?,
        path: r.get(1)?,
        pins: serde_json::from_str(&r.get::<_, String>(2)?).map_err(json_err)?,
        level: enum_from(r.get(3)?)?,
        reason: r.get(4)?,
        review: r.get(5)?,
        source: enum_from(r.get(6)?)?,
        created_at: r.get(7)?,
    })
}

impl Store {
    pub fn hotspots(&self, review: &str) -> rusqlite::Result<Vec<Hotspot>> {
        self.select_by_review("hotspots", COLUMNS, review, row_to_hotspot)
    }

    /// Replaces `review`'s hotspots with `hotspots`.
    pub fn replace_hotspots(&mut self, review: &str, hotspots: &[Hotspot]) -> rusqlite::Result<()> {
        let tx = self.db.transaction()?;
        tx.execute("DELETE FROM hotspots WHERE review = ?", [review])?;
        for h in hotspots {
            tx.execute(
                "INSERT INTO hotspots (path, pins, level, reason, review, source, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                params![
                    h.path,
                    serde_json::to_string(&h.pins).map_err(json_err)?,
                    enum_to(&h.level),
                    h.reason,
                    review,
                    enum_to(&h.source),
                    h.created_at,
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
    fn a_new_pass_replaces_the_reviews_hotspots() {
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
        let h = |reason: &str, review: &str| Hotspot {
            id: 0,
            path: "a.rs".into(),
            pins: vec![pin.clone()],
            level: Level::High,
            reason: reason.into(),
            review: review.into(),
            source: Source::Terminal,
            created_at: 5,
        };
        store.replace_hotspots("@", &[h("old", "@")]).unwrap();
        store.replace_hotspots("x", &[h("other", "x")]).unwrap();
        store.replace_hotspots("@", &[h("new", "@")]).unwrap();
        let got = store.hotspots("@").unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].reason.as_str(), got[0].level), ("new", Level::High));
        assert_eq!(got[0].source, Source::Terminal);
        assert_eq!(store.hotspots("x").unwrap().len(), 1);
    }
}
