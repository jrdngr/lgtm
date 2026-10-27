//! The PR each PR workspace was checked out for, stored whole as JSON.

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{Store, json_err};
use crate::github::GhRepo;

/// A workspace's PR, as stored: the one it was checked out for, or one picked from its stack.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrRecord {
    pub workspace: String,
    /// The repo the PR is in.
    pub repo: GhRepo,
    pub number: u32,
    /// The PR's node id, which mutations take.
    pub id: String,
    pub title: String,
    pub url: String,
    pub author: String,
    pub base_ref: String,
    /// The remote for `repo`, which the base branch is fetched from.
    pub base_remote: String,
    pub head_ref: String,
    /// The remote the head branch was fetched from: the base remote, `gh-<owner>` for a
    /// fork, or `None` when it came from `pull/N/head`.
    pub head_remote: Option<String>,
    /// A fork remote lgtm added for this PR, which goes when the workspace does.
    #[serde(default)]
    pub added_remote: Option<String>,
    /// The head commit as last fetched.
    pub head: String,
    /// Picked from the workspace's stack rather than checked out.
    #[serde(default)]
    pub stack: bool,
    /// A stack PR's commits stop at this bookmark: the head of the open PR below it.
    #[serde(default)]
    pub below: Option<String>,
    /// A stack PR the user pinned, which stays whatever the review is.
    #[serde(default)]
    pub pinned: bool,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS prs (
    workspace TEXT PRIMARY KEY,
    data TEXT NOT NULL
);
";

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(SCHEMA)
}

fn parse(data: String) -> rusqlite::Result<PrRecord> {
    serde_json::from_str(&data).map_err(json_err)
}

impl Store {
    pub fn save_pr(&self, pr: &PrRecord) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO prs (workspace, data) VALUES (?, ?)",
            params![pr.workspace, serde_json::to_string(pr).map_err(json_err)?],
        )?;
        Ok(())
    }

    pub fn pr(&self, workspace: &str) -> rusqlite::Result<Option<PrRecord>> {
        self.db
            .query_row(
                "SELECT data FROM prs WHERE workspace = ?",
                [workspace],
                |r| r.get(0),
            )
            .optional()?
            .map(parse)
            .transpose()
    }

    pub fn prs(&self) -> rusqlite::Result<Vec<PrRecord>> {
        let mut stmt = self.db.prepare("SELECT data FROM prs")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.map(|d| parse(d?)).collect()
    }

    pub fn delete_pr(&self, workspace: &str) -> rusqlite::Result<()> {
        self.db
            .execute("DELETE FROM prs WHERE workspace = ?", [workspace])?;
        Ok(())
    }
}
