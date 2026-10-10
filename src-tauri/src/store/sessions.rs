//! Review sessions: the commits a review showed and the marks it matched, so the next session
//! can show what changed since.

use rusqlite::{OptionalExtension, params};

use super::{Store, from_blob, to_blob};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    pub id: i64,
    pub started_at: i64,
    /// Commit IDs of the review's changes, oldest first.
    pub commits: Vec<String>,
    /// Ids of the marks the session matched onto its rows.
    pub seen: Vec<i64>,
}

impl Store {
    /// The most recent session of `review`.
    pub fn last_session(&self, review: &str) -> rusqlite::Result<Option<Session>> {
        self.session_before(review, i64::MAX)
    }

    /// The most recent session of `review` that started before session `id`.
    pub fn session_before(&self, review: &str, id: i64) -> rusqlite::Result<Option<Session>> {
        self.db
            .query_row(
                "SELECT id, started_at, commits, seen FROM sessions WHERE review = ? AND id < ?
                 ORDER BY id DESC LIMIT 1",
                params![review, id],
                |r| {
                    let commits: String = r.get(2)?;
                    Ok(Session {
                        id: r.get(0)?,
                        started_at: r.get(1)?,
                        commits: commits.split_whitespace().map(str::to_string).collect(),
                        seen: from_blob(&r.get::<_, Vec<u8>>(3)?),
                    })
                },
            )
            .optional()
    }

    /// Starts a session of `review`, a review of `revset` in `workspace` (empty for main).
    pub fn start_session(
        &self,
        review: &str,
        workspace: &str,
        revset: &str,
        started_at: i64,
    ) -> rusqlite::Result<i64> {
        self.db.execute(
            "INSERT INTO sessions (review, workspace, revset, started_at, commits, seen)
             VALUES (?, ?, ?, ?, '', x'')",
            params![review, workspace, revset, started_at],
        )?;
        Ok(self.db.last_insert_rowid())
    }

    /// The revset of the latest review in `workspace`.
    pub fn last_revset(&self, workspace: &str) -> rusqlite::Result<Option<String>> {
        self.db
            .query_row(
                "SELECT CASE revset WHEN '' THEN review ELSE revset END FROM sessions
                 WHERE workspace = ? ORDER BY id DESC LIMIT 1",
                [workspace],
                |r| r.get(0),
            )
            .optional()
    }

    pub fn save_session(&self, id: i64, commits: &[String], seen: &[i64]) -> rusqlite::Result<()> {
        self.db.execute(
            "UPDATE sessions SET commits = ?, seen = ? WHERE id = ?",
            params![commits.join(" "), to_blob(seen), id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_session_is_the_newest_for_its_review() {
        let store = Store::in_memory();
        assert_eq!(store.last_session("r").unwrap(), None);
        let a = store.start_session("r", "", "r", 1).unwrap();
        store
            .save_session(a, &["c1".into(), "c2".into()], &[7])
            .unwrap();
        let b = store.start_session("r", "", "r", 2).unwrap();
        store.start_session("@ in t", "t", "@", 3).unwrap();
        let last = store.last_session("r").unwrap().unwrap();
        assert_eq!((last.id, last.commits.len()), (b, 0));
        store.save_session(b, &["c3".into()], &[1, 2]).unwrap();
        let last = store.last_session("r").unwrap().unwrap();
        assert_eq!(
            (last.commits, last.seen),
            (vec!["c3".to_string()], vec![1, 2])
        );
        assert_eq!(store.last_revset("").unwrap().as_deref(), Some("r"));
        assert_eq!(store.last_revset("t").unwrap().as_deref(), Some("@"));
        assert_eq!(store.last_revset("gone").unwrap(), None);
    }
}
