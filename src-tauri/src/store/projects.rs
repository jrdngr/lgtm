//! Projects: named lists of repos that one window shows. Kept in the app's global store.

use std::path::PathBuf;

use rusqlite::{OptionalExtension, Row, params};

use super::Store;

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            repos TEXT NOT NULL,
            asleep TEXT NOT NULL,
            ui TEXT NOT NULL,
            open INTEGER NOT NULL,
            used INTEGER NOT NULL
        );",
    )
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectRow {
    pub id: String,
    pub name: String,
    /// Repo roots, in the order they were added.
    pub repos: Vec<PathBuf>,
    /// The roots of the repos put to sleep.
    pub asleep: Vec<PathBuf>,
    /// The window's own state for its repo list, as JSON it reads back.
    pub ui: String,
    /// Open in a window, or open when the app last quit.
    pub open: bool,
    /// When it was last opened, in ms.
    pub used: i64,
}

fn paths(json: String) -> Vec<PathBuf> {
    serde_json::from_str(&json).unwrap_or_default()
}

fn row(r: &Row) -> rusqlite::Result<ProjectRow> {
    Ok(ProjectRow {
        id: r.get(0)?,
        name: r.get(1)?,
        repos: paths(r.get(2)?),
        asleep: paths(r.get(3)?),
        ui: r.get(4)?,
        open: r.get(5)?,
        used: r.get(6)?,
    })
}

const COLUMNS: &str = "id, name, repos, asleep, ui, open, used";

impl Store {
    /// Every project, most recently used first.
    pub fn projects(&self) -> rusqlite::Result<Vec<ProjectRow>> {
        let mut stmt = self.db.prepare(&format!(
            "SELECT {COLUMNS} FROM projects ORDER BY used DESC"
        ))?;
        stmt.query_map([], row)?.collect()
    }

    pub fn project(&self, id: &str) -> rusqlite::Result<Option<ProjectRow>> {
        self.db
            .query_row(
                &format!("SELECT {COLUMNS} FROM projects WHERE id = ?"),
                [id],
                row,
            )
            .optional()
    }

    pub fn save_project(&self, p: &ProjectRow) -> rusqlite::Result<()> {
        let json = |v: &Vec<PathBuf>| serde_json::to_string(v).unwrap_or_else(|_| "[]".into());
        self.db.execute(
            &format!("INSERT OR REPLACE INTO projects ({COLUMNS}) VALUES (?, ?, ?, ?, ?, ?, ?)"),
            params![
                p.id,
                p.name,
                json(&p.repos),
                json(&p.asleep),
                p.ui,
                p.open,
                p.used
            ],
        )?;
        Ok(())
    }

    pub fn delete_project(&self, id: &str) -> rusqlite::Result<()> {
        self.db.execute("DELETE FROM projects WHERE id = ?", [id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_round_trip_newest_first() {
        let store = Store::in_memory();
        let old = ProjectRow {
            id: "a".into(),
            name: "lgtm".into(),
            repos: vec!["/r/lgtm".into(), "/r/agent-stuff".into()],
            asleep: vec!["/r/agent-stuff".into()],
            ui: "{}".into(),
            open: true,
            used: 1,
        };
        let new = ProjectRow {
            id: "b".into(),
            name: "notes".into(),
            used: 2,
            ..Default::default()
        };
        store.save_project(&old).unwrap();
        store.save_project(&new).unwrap();
        assert_eq!(store.projects().unwrap(), [new.clone(), old.clone()]);

        store.delete_project("b").unwrap();
        assert_eq!(store.project("b").unwrap(), None);
        assert_eq!(store.project("a").unwrap(), Some(old));
    }
}
