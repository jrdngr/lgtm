//! The PR each PR workspace was checked out for, stored whole as JSON.

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{Store, json_err};
use crate::github::GhRepo;
use crate::jj::git::remote_bookmark;

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

impl PrRecord {
    /// What the workspace reviews: the PR's own commits, which match GitHub's diff.
    /// A stack PR's stop at the open PR below it, even when both target the same base.
    pub fn revset(&self) -> String {
        let base = remote_bookmark(&self.base_ref, &self.base_remote);
        let own = format!("{base}..{}", self.head_revset());
        match &self.below {
            Some(b) => format!("{own} ~ ::{}", head_bookmark_revset(b)),
            None => own,
        }
    }

    /// The PR's head. A stack PR's is its bookmark: the local one, else where it was fetched.
    /// A checked-out PR's is where it was fetched to, since its branch name, like a fork's
    /// `main`, can name another branch here.
    pub fn head_revset(&self) -> String {
        match (self.stack, &self.head_remote) {
            (true, _) => head_bookmark_revset(&self.head_ref),
            (false, Some(remote)) => remote_bookmark(&self.head_ref, remote),
            (false, None) => format!(
                "bookmarks(exact:{})",
                quoted(&fallback_bookmark(self.number))
            ),
        }
    }
}

fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""))
}

/// A branch head: the local bookmark, else the remote one.
pub fn head_bookmark_revset(head: &str) -> String {
    let head = quoted(head);
    format!("coalesce(bookmarks(exact:{head}), remote_bookmarks(exact:{head}))")
}

/// The bookmark a PR's head is fetched into when its branch can't be fetched.
pub fn fallback_bookmark(number: u32) -> String {
    format!("lgtm/pr-{number}")
}
