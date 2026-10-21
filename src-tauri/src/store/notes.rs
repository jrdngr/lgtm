//! Markdown notes and their folders. A note without a workspace belongs to the whole store: the
//! repo, or every repo in the global store.

use rusqlite::{OptionalExtension, Row, params};

use super::Store;

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS notes (
            id TEXT PRIMARY KEY,
            workspace TEXT,
            folder TEXT NOT NULL,
            name TEXT NOT NULL,
            body TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS note_folders (
            workspace TEXT,
            path TEXT NOT NULL
        );",
    )
}

/// A stored note. `folder` is a `/`-separated path, empty at the top.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteRow {
    pub id: String,
    pub workspace: Option<String>,
    pub folder: String,
    pub name: String,
    pub body: String,
    pub created_at: i64,
    pub updated_at: i64,
}

const COLUMNS: &str = "id, workspace, folder, name, body, created_at, updated_at";

fn row(r: &Row) -> rusqlite::Result<NoteRow> {
    Ok(NoteRow {
        id: r.get(0)?,
        workspace: r.get(1)?,
        folder: r.get(2)?,
        name: r.get(3)?,
        body: r.get(4)?,
        created_at: r.get(5)?,
        updated_at: r.get(6)?,
    })
}

impl Store {
    pub fn note(&self, id: &str) -> rusqlite::Result<Option<NoteRow>> {
        self.db
            .query_row(&format!("SELECT {COLUMNS} FROM notes WHERE id = ?"), [id], row)
            .optional()
    }

    /// The notes of `workspace`, or with `None` the ones that belong to no workspace.
    pub fn notes_of(&self, workspace: Option<&str>) -> rusqlite::Result<Vec<NoteRow>> {
        let mut stmt = self
            .db
            .prepare(&format!("SELECT {COLUMNS} FROM notes WHERE workspace IS ?"))?;
        stmt.query_map([workspace], row)?.collect()
    }

    pub fn put_note(&self, n: &NoteRow) -> rusqlite::Result<()> {
        self.db.execute(
            &format!("INSERT OR REPLACE INTO notes ({COLUMNS}) VALUES (?, ?, ?, ?, ?, ?, ?)"),
            params![
                n.id,
                n.workspace,
                n.folder,
                n.name,
                n.body,
                n.created_at,
                n.updated_at
            ],
        )?;
        Ok(())
    }

    pub fn delete_note(&self, id: &str) -> rusqlite::Result<()> {
        self.db.execute("DELETE FROM notes WHERE id = ?", [id])?;
        Ok(())
    }

    /// The folders made for `workspace` (or for no workspace), whether or not they hold notes.
    pub fn note_folders(&self, workspace: Option<&str>) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self
            .db
            .prepare("SELECT path FROM note_folders WHERE workspace IS ?")?;
        stmt.query_map([workspace], |r| r.get(0))?.collect()
    }

    pub fn add_note_folder(&self, workspace: Option<&str>, path: &str) -> rusqlite::Result<()> {
        if !self.note_folders(workspace)?.iter().any(|p| p == path) {
            self.db.execute(
                "INSERT INTO note_folders (workspace, path) VALUES (?, ?)",
                params![workspace, path],
            )?;
        }
        Ok(())
    }

    pub fn delete_note_folder(&self, workspace: Option<&str>, path: &str) -> rusqlite::Result<()> {
        self.db.execute(
            "DELETE FROM note_folders WHERE workspace IS ? AND path = ?",
            params![workspace, path],
        )?;
        Ok(())
    }
}
