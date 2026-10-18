//! Per-repo SQLite store for marks, file checks, comments, review sessions, and reports.

mod arch;
mod bots;
mod checks;
mod comments;
mod marks;
mod notes;
mod prs;
mod qa;
mod sessions;
mod setup;
mod threads;

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params, params_from_iter};

pub use checks::FileCheck;
pub use comments::{Author, Comment, Conversation, GhLink, GhPost, Message, Pin};
pub use marks::{Mark, MarkColor};
pub use notes::{Category, FlowStep, Level, Note, Source};
pub use sessions::Session;
pub use threads::{Quote, Stage, Take, TakeVerdict, ThreadState};

pub struct Store {
    db: Connection,
}

/// One undoable change to marks and file checks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Edit {
    pub marks_removed: Vec<Mark>,
    pub marks_added: Vec<Mark>,
    pub checks_removed: Vec<FileCheck>,
    pub checks_added: Vec<FileCheck>,
}

impl Edit {
    pub fn is_empty(&self) -> bool {
        self.marks_removed.is_empty()
            && self.marks_added.is_empty()
            && self.checks_removed.is_empty()
            && self.checks_added.is_empty()
    }

    /// The edit that undoes this one.
    pub fn inverse(&self) -> Edit {
        Edit {
            marks_removed: self.marks_added.clone(),
            marks_added: self.marks_removed.clone(),
            checks_removed: self.checks_added.clone(),
            checks_added: self.checks_removed.clone(),
        }
    }
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS marks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL,
    side TEXT NOT NULL,
    line_hash INTEGER NOT NULL,
    above BLOB NOT NULL,
    below BLOB NOT NULL,
    commit_id TEXT NOT NULL,
    line INTEGER NOT NULL,
    color TEXT NOT NULL,
    review TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS marks_path ON marks (path);
CREATE INDEX IF NOT EXISTS marks_review ON marks (review);
CREATE TABLE IF NOT EXISTS file_checks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL,
    old_hashes BLOB NOT NULL,
    new_hashes BLOB NOT NULL,
    review TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    fingerprint INTEGER
);
CREATE INDEX IF NOT EXISTS file_checks_path ON file_checks (path);
CREATE TABLE IF NOT EXISTS sessions (
    id INTEGER PRIMARY KEY,
    review TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    commits TEXT NOT NULL,
    seen BLOB NOT NULL
);
CREATE INDEX IF NOT EXISTS sessions_review ON sessions (review);
CREATE TABLE IF NOT EXISTS line_hashes (
    commit_id TEXT NOT NULL,
    path TEXT NOT NULL,
    hashes BLOB NOT NULL,
    PRIMARY KEY (commit_id, path)
);
";

pub(super) fn json_err(e: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, e.into())
}

/// A unit enum stored as its serde name.
pub(super) fn enum_from<T: serde::de::DeserializeOwned>(s: String) -> rusqlite::Result<T> {
    serde_json::from_value(serde_json::Value::String(s)).map_err(json_err)
}

pub(super) fn enum_to(v: &impl serde::Serialize) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

pub(crate) fn to_blob(hashes: &[i64]) -> Vec<u8> {
    hashes.iter().flat_map(|h| h.to_le_bytes()).collect()
}

pub(crate) fn from_blob(blob: &[u8]) -> Vec<i64> {
    blob.as_chunks::<8>()
        .0
        .iter()
        .map(|c| i64::from_le_bytes(*c))
        .collect()
}

/// Columns added after the first schema. Rows from before default to the main workspace,
/// with the review label as the revset.
fn migrate(db: &Connection) -> rusqlite::Result<()> {
    let has_workspace: bool = db.query_row(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('sessions') WHERE name = 'workspace'",
        [],
        |r| r.get(0),
    )?;
    if !has_workspace {
        db.execute_batch(
            "ALTER TABLE sessions ADD COLUMN workspace TEXT NOT NULL DEFAULT '';
             ALTER TABLE sessions ADD COLUMN revset TEXT NOT NULL DEFAULT '';",
        )?;
    }
    migrate_autoincrement(db)?;
    migrate_mark_colors(db)?;
    migrate_check_fingerprints(db)?;
    migrate_verdict_names(db)
}

/// Takes saved before the verdicts were renamed to agree, change, disagree and decide.
fn migrate_verdict_names(db: &Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        r#"UPDATE comments SET thread = replace(replace(replace(thread,
             '"verdict":"agreeWithChanges"', '"verdict":"change"'),
             '"verdict":"pushBack"', '"verdict":"disagree"'),
             '"verdict":"needsYou"', '"verdict":"decide"')
           WHERE thread LIKE '%"verdict":"%';"#,
    )
}

/// Checks from before fingerprints were stored keep a null one.
fn migrate_check_fingerprints(db: &Connection) -> rusqlite::Result<()> {
    let has_fingerprint: bool = db.query_row(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('file_checks') WHERE name = 'fingerprint'",
        [],
        |r| r.get(0),
    )?;
    if !has_fingerprint {
        db.execute_batch("ALTER TABLE file_checks ADD COLUMN fingerprint INTEGER;")?;
    }
    Ok(())
}

/// Marks were once reviewed, skipped or flagged; those are now the colors they were drawn in.
fn migrate_mark_colors(db: &Connection) -> rusqlite::Result<()> {
    let has_state: bool = db.query_row(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('marks') WHERE name = 'state'",
        [],
        |r| r.get(0),
    )?;
    if has_state {
        db.execute_batch(
            "ALTER TABLE marks RENAME COLUMN state TO color;
             UPDATE marks SET color = CASE color
                 WHEN 'reviewed' THEN 'cyan' WHEN 'skipped' THEN 'gray' WHEN 'flagged' THEN 'red'
                 ELSE color END;",
        )?;
    }
    Ok(())
}

/// Marks, checks and comments once reused a deleted row's id, which undo history and
/// running agents still refer to.
/// SQLite can't add AUTOINCREMENT in place, so the tables are rebuilt.
fn migrate_autoincrement(db: &Connection) -> rusqlite::Result<()> {
    for table in ["marks", "file_checks", "comments"] {
        let sql: String = db.query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?",
            [table],
            |r| r.get(0),
        )?;
        if sql.contains("AUTOINCREMENT") {
            continue;
        }
        let create = sql.replacen("PRIMARY KEY", "PRIMARY KEY AUTOINCREMENT", 1);
        db.execute_batch(&format!(
            "BEGIN;
             ALTER TABLE {table} RENAME TO {table}_old;
             {create};
             INSERT INTO {table} SELECT * FROM {table}_old;
             DROP TABLE {table}_old;
             COMMIT;"
        ))?;
    }
    // Brings back the indexes dropped with the old tables.
    db.execute_batch(SCHEMA)?;
    comments::create_table(db)
}

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
        comments::create_table(&db)?;
        arch::create_table(&db)?;
        bots::create_table(&db)?;
        notes::create_table(&db)?;
        prs::create_table(&db)?;
        qa::create_table(&db)?;
        setup::create_table(&db)?;
        migrate(&db)?;
        Ok(Self { db })
    }

    /// Deletes workspace `name`'s Reviewer runs, Q&A sessions, review sessions, assessments and
    /// Look checks, so a new workspace with its name starts empty.
    pub fn forget_workspace(&mut self, name: &str) -> rusqlite::Result<()> {
        let tx = self.db.transaction()?;
        for table in ["bot_runs", "qa_sessions", "sessions", "arch_reviews"] {
            tx.execute(&format!("DELETE FROM {table} WHERE workspace = ?"), [name])?;
        }
        tx.commit()
    }

    /// Applies `edit` in one transaction. Added rows keep their ids when they have one (redo)
    /// and get fresh ids otherwise. Returns the edit with the ids it was stored under.
    pub fn apply(&mut self, edit: &Edit) -> rusqlite::Result<Edit> {
        let tx = self.db.transaction()?;
        for m in &edit.marks_removed {
            tx.execute("DELETE FROM marks WHERE id = ?", [m.id])?;
        }
        for c in &edit.checks_removed {
            tx.execute("DELETE FROM file_checks WHERE id = ?", [c.id])?;
        }
        let marks_added = edit
            .marks_added
            .iter()
            .map(|m| marks::insert(&tx, m))
            .collect::<rusqlite::Result<_>>()?;
        let checks_added = edit
            .checks_added
            .iter()
            .map(|c| checks::insert(&tx, c))
            .collect::<rusqlite::Result<_>>()?;
        tx.commit()?;
        Ok(Edit {
            marks_removed: edit.marks_removed.clone(),
            marks_added,
            checks_removed: edit.checks_removed.clone(),
            checks_added,
        })
    }

    pub fn line_hashes(&self, commit: &str, path: &str) -> rusqlite::Result<Option<Vec<i64>>> {
        let mut stmt = self
            .db
            .prepare_cached("SELECT hashes FROM line_hashes WHERE commit_id = ? AND path = ?")?;
        stmt.query_row([commit, path], |r| Ok(from_blob(&r.get::<_, Vec<u8>>(0)?)))
            .optional()
    }

    /// Rows of `table` whose path is one of `paths`.
    fn select_by_paths<T>(
        &self,
        table: &str,
        columns: &str,
        paths: &[&str],
        row: fn(&rusqlite::Row) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<Vec<T>> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = vec!["?"; paths.len()].join(",");
        let sql = format!("SELECT {columns} FROM {table} WHERE path IN ({placeholders})");
        let mut stmt = self.db.prepare(&sql)?;
        stmt.query_map(params_from_iter(paths), row)?.collect()
    }

    /// Rows of `table` created by `review`.
    fn select_by_review<T>(
        &self,
        table: &str,
        columns: &str,
        review: &str,
        row: fn(&rusqlite::Row) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<Vec<T>> {
        let sql = format!("SELECT {columns} FROM {table} WHERE review = ?");
        let mut stmt = self.db.prepare(&sql)?;
        stmt.query_map([review], row)?.collect()
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

    #[test]
    fn old_tables_gain_autoincrement_and_mark_colors_and_keep_their_rows() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            &SCHEMA
                .replace("AUTOINCREMENT", "")
                .replace("color TEXT", "state TEXT"),
        )
        .unwrap();
        db.execute_batch(
            "INSERT INTO marks VALUES (1, 'a.rs', 'new', 0, x'', x'', 'c', 1, 'reviewed', 'r', 0),
                                      (2, 'b.rs', 'new', 0, x'', x'', 'c', 1, 'flagged', 'r', 0);",
        )
        .unwrap();
        let store = Store::init(db).unwrap();
        let colors: Vec<MarkColor> = store
            .marks_for_review("r")
            .unwrap()
            .into_iter()
            .map(|m| m.color)
            .collect();
        assert_eq!(colors, [MarkColor::Cyan, MarkColor::Red]);
        let sql: String = store
            .db
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name = 'marks'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(sql.contains("AUTOINCREMENT"));
        store
            .db
            .execute("DELETE FROM marks WHERE id = 2", [])
            .unwrap();
        store
            .db
            .execute_batch(
                "INSERT INTO marks (path, side, line_hash, above, below, commit_id, line, color, \
                 review, created_at) VALUES ('c.rs', 'new', 0, x'', x'', 'c', 1, 'violet', 'r', 0)",
            )
            .unwrap();
        let ids: Vec<i64> = store
            .db
            .prepare("SELECT id FROM marks ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(ids, [1, 3]);
        let indexes: i64 = store
            .db
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND tbl_name = 'marks'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(indexes, 2);
    }

    #[test]
    fn line_hashes_round_trip() {
        let store = Store::in_memory();
        assert_eq!(store.line_hashes("c", "p").unwrap(), None);
        store.save_line_hashes("c", "p", &[3, -4]).unwrap();
        assert_eq!(store.line_hashes("c", "p").unwrap(), Some(vec![3, -4]));
    }
}
