//! Per-repo SQLite store for marks.

use std::path::Path;

use rusqlite::{Connection, params, params_from_iter};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::anchor::{Anchor, Side};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum MarkState {
    Reviewed,
    Skipped,
    Flagged,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    pub id: i64,
    pub anchor: Anchor,
    /// Commit holding the file side the mark was made on.
    pub commit: String,
    /// 1-based line in that file side.
    pub line: u32,
    pub state: MarkState,
    /// The review that created the mark, e.g. its revset.
    pub review: String,
    /// Unix milliseconds.
    pub created_at: i64,
}

pub struct Store {
    db: Connection,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS marks (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL,
    side TEXT NOT NULL,
    line_hash INTEGER NOT NULL,
    above BLOB NOT NULL,
    below BLOB NOT NULL,
    commit_id TEXT NOT NULL,
    line INTEGER NOT NULL,
    state TEXT NOT NULL,
    review TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS marks_path ON marks (path);
CREATE INDEX IF NOT EXISTS marks_review ON marks (review);
CREATE TABLE IF NOT EXISTS line_hashes (
    commit_id TEXT NOT NULL,
    path TEXT NOT NULL,
    hashes BLOB NOT NULL,
    PRIMARY KEY (commit_id, path)
);
";

fn to_blob(hashes: &[i64]) -> Vec<u8> {
    hashes.iter().flat_map(|h| h.to_le_bytes()).collect()
}

fn from_blob(blob: &[u8]) -> Vec<i64> {
    blob.chunks_exact(8)
        .map(|c| i64::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

fn side_str(side: Side) -> &'static str {
    match side {
        Side::Old => "old",
        Side::New => "new",
    }
}

fn state_str(state: MarkState) -> &'static str {
    match state {
        MarkState::Reviewed => "reviewed",
        MarkState::Skipped => "skipped",
        MarkState::Flagged => "flagged",
    }
}

fn parse_err(what: &str, value: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        format!("bad {what}: {value}").into(),
    )
}

fn row_to_mark(r: &rusqlite::Row) -> rusqlite::Result<Mark> {
    let side = match r.get::<_, String>(2)?.as_str() {
        "old" => Side::Old,
        "new" => Side::New,
        other => return Err(parse_err("side", other.into())),
    };
    let state = match r.get::<_, String>(8)?.as_str() {
        "reviewed" => MarkState::Reviewed,
        "skipped" => MarkState::Skipped,
        "flagged" => MarkState::Flagged,
        other => return Err(parse_err("state", other.into())),
    };
    Ok(Mark {
        id: r.get(0)?,
        anchor: Anchor {
            path: r.get(1)?,
            side,
            line_hash: r.get(3)?,
            above: from_blob(&r.get::<_, Vec<u8>>(4)?),
            below: from_blob(&r.get::<_, Vec<u8>>(5)?),
        },
        commit: r.get(6)?,
        line: r.get(7)?,
        state,
        review: r.get(9)?,
        created_at: r.get(10)?,
    })
}

const COLUMNS: &str =
    "id, path, side, line_hash, above, below, commit_id, line, state, review, created_at";

impl Store {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Ok(Self::init(Connection::open(path)?)?)
    }

    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self::init(Connection::open_in_memory().unwrap()).unwrap()
    }

    fn init(db: Connection) -> rusqlite::Result<Self> {
        db.execute_batch("PRAGMA journal_mode = WAL;")?;
        db.execute_batch(SCHEMA)?;
        Ok(Self { db })
    }

    pub fn marks_for_paths(&self, paths: &[&str]) -> rusqlite::Result<Vec<Mark>> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = vec!["?"; paths.len()].join(",");
        let sql = format!("SELECT {COLUMNS} FROM marks WHERE path IN ({placeholders})");
        let mut stmt = self.db.prepare(&sql)?;
        stmt.query_map(params_from_iter(paths), row_to_mark)?
            .collect()
    }

    pub fn marks_for_review(&self, review: &str) -> rusqlite::Result<Vec<Mark>> {
        let sql = format!("SELECT {COLUMNS} FROM marks WHERE review = ?");
        let mut stmt = self.db.prepare(&sql)?;
        stmt.query_map([review], row_to_mark)?.collect()
    }

    /// Removes `remove` and inserts `insert` in one transaction. Inserted marks keep their
    /// ids when they have one (redo) and get fresh ids otherwise. Returns the inserted marks.
    pub fn replace(&mut self, remove: &[i64], insert: &[Mark]) -> rusqlite::Result<Vec<Mark>> {
        let tx = self.db.transaction()?;
        for id in remove {
            tx.execute("DELETE FROM marks WHERE id = ?", [id])?;
        }
        let mut inserted = Vec::with_capacity(insert.len());
        for m in insert {
            tx.execute(
                "INSERT INTO marks (id, path, side, line_hash, above, below, commit_id, line, state, review, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    (m.id != 0).then_some(m.id),
                    m.anchor.path,
                    side_str(m.anchor.side),
                    m.anchor.line_hash,
                    to_blob(&m.anchor.above),
                    to_blob(&m.anchor.below),
                    m.commit,
                    m.line,
                    state_str(m.state),
                    m.review,
                    m.created_at,
                ],
            )?;
            inserted.push(Mark {
                id: tx.last_insert_rowid(),
                ..m.clone()
            });
        }
        tx.commit()?;
        Ok(inserted)
    }

    pub fn line_hashes(&self, commit: &str, path: &str) -> rusqlite::Result<Option<Vec<i64>>> {
        let mut stmt = self
            .db
            .prepare_cached("SELECT hashes FROM line_hashes WHERE commit_id = ? AND path = ?")?;
        let mut rows = stmt.query([commit, path])?;
        Ok(match rows.next()? {
            Some(r) => Some(from_blob(&r.get::<_, Vec<u8>>(0)?)),
            None => None,
        })
    }

    pub fn save_line_hashes(
        &self,
        commit: &str,
        path: &str,
        hashes: &[i64],
    ) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT OR IGNORE INTO line_hashes (commit_id, path, hashes) VALUES (?, ?, ?)",
            params![commit, path, to_blob(hashes)],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mark(path: &str, review: &str) -> Mark {
        Mark {
            id: 0,
            anchor: Anchor {
                path: path.into(),
                side: Side::Old,
                line_hash: -42,
                above: vec![1, i64::MIN],
                below: vec![],
            },
            commit: "abc".into(),
            line: 7,
            state: MarkState::Flagged,
            review: review.into(),
            created_at: 1000,
        }
    }

    #[test]
    fn replace_round_trips_and_redo_keeps_ids() {
        let mut store = Store::in_memory();
        let inserted = store
            .replace(&[], &[mark("a.rs", "r1"), mark("b.rs", "r1")])
            .unwrap();
        let got = store.marks_for_paths(&["a.rs"]).unwrap();
        assert_eq!(got, [inserted[0].clone()]);

        store.replace(&[inserted[0].id], &[]).unwrap();
        assert!(store.marks_for_paths(&["a.rs"]).unwrap().is_empty());

        let redone = store.replace(&[], &[inserted[0].clone()]).unwrap();
        assert_eq!(redone[0].id, inserted[0].id);
        assert_eq!(store.marks_for_review("r1").unwrap().len(), 2);
    }

    #[test]
    fn line_hashes_round_trip() {
        let store = Store::in_memory();
        assert_eq!(store.line_hashes("c", "p").unwrap(), None);
        store.save_line_hashes("c", "p", &[3, -4]).unwrap();
        assert_eq!(store.line_hashes("c", "p").unwrap(), Some(vec![3, -4]));
    }
}
