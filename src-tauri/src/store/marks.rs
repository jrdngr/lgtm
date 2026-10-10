//! Row marks. A mark is anchored to a changed line so it can follow the code through edits.

use rusqlite::{Transaction, params};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{Store, from_blob, to_blob};
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

const COLUMNS: &str =
    "id, path, side, line_hash, above, below, commit_id, line, state, review, created_at";

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

pub(super) fn insert(tx: &Transaction, m: &Mark) -> rusqlite::Result<Mark> {
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
    Ok(Mark {
        id: tx.last_insert_rowid(),
        ..m.clone()
    })
}

impl Store {
    pub fn marks_for_paths(&self, paths: &[&str]) -> rusqlite::Result<Vec<Mark>> {
        self.select_by_paths("marks", COLUMNS, paths, row_to_mark)
    }

    pub fn marks_for_review(&self, review: &str) -> rusqlite::Result<Vec<Mark>> {
        self.select_by_review("marks", COLUMNS, review, row_to_mark)
    }
}

#[cfg(test)]
mod tests {
    use super::super::Edit;
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
    fn apply_round_trips_and_redo_keeps_ids() {
        let mut store = Store::in_memory();
        let add = |marks: Vec<Mark>| Edit {
            marks_added: marks,
            ..Edit::default()
        };
        let inserted = store
            .apply(&add(vec![mark("a.rs", "r1"), mark("b.rs", "r1")]))
            .unwrap()
            .marks_added;
        let got = store.marks_for_paths(&["a.rs"]).unwrap();
        assert_eq!(got, [inserted[0].clone()]);

        let removed = store
            .apply(&add(vec![inserted[0].clone()]).inverse())
            .unwrap();
        assert!(store.marks_for_paths(&["a.rs"]).unwrap().is_empty());

        let redone = store.apply(&removed.inverse()).unwrap().marks_added;
        assert_eq!(redone[0].id, inserted[0].id);
        assert_eq!(store.marks_for_review("r1").unwrap().len(), 2);
    }
}
