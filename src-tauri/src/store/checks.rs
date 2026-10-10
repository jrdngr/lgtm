//! File checkboxes. A check remembers the changed rows the file had when it was checked.

use rusqlite::{Transaction, params};

use super::{Store, from_blob, to_blob};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileCheck {
    pub id: i64,
    pub path: String,
    /// Line hashes of the file's removed and added rows when it was checked.
    pub old_hashes: Vec<i64>,
    pub new_hashes: Vec<i64>,
    pub review: String,
    pub created_at: i64,
}

const COLUMNS: &str = "id, path, old_hashes, new_hashes, review, created_at";

fn row_to_check(r: &rusqlite::Row) -> rusqlite::Result<FileCheck> {
    Ok(FileCheck {
        id: r.get(0)?,
        path: r.get(1)?,
        old_hashes: from_blob(&r.get::<_, Vec<u8>>(2)?),
        new_hashes: from_blob(&r.get::<_, Vec<u8>>(3)?),
        review: r.get(4)?,
        created_at: r.get(5)?,
    })
}

pub(super) fn insert(tx: &Transaction, c: &FileCheck) -> rusqlite::Result<FileCheck> {
    tx.execute(
        "INSERT INTO file_checks (id, path, old_hashes, new_hashes, review, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
        params![
            (c.id != 0).then_some(c.id),
            c.path,
            to_blob(&c.old_hashes),
            to_blob(&c.new_hashes),
            c.review,
            c.created_at,
        ],
    )?;
    Ok(FileCheck {
        id: tx.last_insert_rowid(),
        ..c.clone()
    })
}

impl Store {
    pub fn checks_for_paths(&self, paths: &[&str]) -> rusqlite::Result<Vec<FileCheck>> {
        self.select_by_paths("file_checks", COLUMNS, paths, row_to_check)
    }

    pub fn checks_for_review(&self, review: &str) -> rusqlite::Result<Vec<FileCheck>> {
        self.select_by_review("file_checks", COLUMNS, review, row_to_check)
    }
}

#[cfg(test)]
mod tests {
    use super::super::Edit;
    use super::*;

    #[test]
    fn checks_round_trip_and_undo_keeps_ids() {
        let mut store = Store::in_memory();
        let check = FileCheck {
            id: 0,
            path: "a.rs".into(),
            old_hashes: vec![1],
            new_hashes: vec![2, 3],
            review: "r".into(),
            created_at: 5,
        };
        let added = store
            .apply(&Edit {
                checks_added: vec![check],
                ..Edit::default()
            })
            .unwrap();
        let got = store.checks_for_paths(&["a.rs", "b.rs"]).unwrap();
        assert_eq!(got, added.checks_added);
        let undone = store.apply(&added.inverse()).unwrap();
        assert!(store.checks_for_review("r").unwrap().is_empty());
        store.apply(&undone.inverse()).unwrap();
        assert_eq!(store.checks_for_review("r").unwrap(), added.checks_added);
    }
}
