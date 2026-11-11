//! Guided reviews: each workspace's last guided review of a revset as JSON, the verdicts given
//! on its chunks, and the edits terminal agents made, for where a chunk's code came from.

use rusqlite::{OptionalExtension, params};

use super::Store;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS guided_reviews (
    workspace TEXT NOT NULL,
    revset TEXT NOT NULL,
    data TEXT NOT NULL,
    PRIMARY KEY (workspace, revset)
);
CREATE TABLE IF NOT EXISTS guided_verdicts (
    workspace TEXT NOT NULL,
    revset TEXT NOT NULL,
    chunk TEXT NOT NULL,
    data TEXT NOT NULL,
    PRIMARY KEY (workspace, revset, chunk)
);
CREATE TABLE IF NOT EXISTS agent_prompts (
    session TEXT PRIMARY KEY,
    prompt TEXT NOT NULL,
    at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS agent_edits (
    id INTEGER PRIMARY KEY,
    session TEXT NOT NULL,
    prompt TEXT NOT NULL,
    path TEXT NOT NULL,
    text TEXT NOT NULL,
    at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS agent_edits_at ON agent_edits (at);
";

/// How many agent edits are kept, newest first.
const KEEP_EDITS: i64 = 20_000;

pub(super) fn create_table(db: &rusqlite::Connection) -> rusqlite::Result<()> {
    db.execute_batch(SCHEMA)
}

/// An edit a terminal agent made, under the prompt it was working on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentEdit {
    pub session: String,
    pub prompt: String,
    /// The edited file, as the agent named it: usually absolute.
    pub path: String,
    /// The text the edit wrote.
    pub text: String,
    pub at: i64,
}

impl Store {
    pub fn guided(&self, workspace: &str, revset: &str) -> rusqlite::Result<Option<String>> {
        self.db
            .query_row(
                "SELECT data FROM guided_reviews WHERE workspace = ? AND revset = ?",
                [workspace, revset],
                |r| r.get(0),
            )
            .optional()
    }

    pub fn save_guided(&self, workspace: &str, revset: &str, data: &str) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO guided_reviews (workspace, revset, data) VALUES (?, ?, ?)",
            params![workspace, revset, data],
        )?;
        Ok(())
    }

    /// The verdicts on the chunks of `revset`'s guided review in `workspace`, by chunk id.
    pub fn guided_verdicts(
        &self,
        workspace: &str,
        revset: &str,
    ) -> rusqlite::Result<Vec<(String, String)>> {
        let mut stmt = self.db.prepare(
            "SELECT chunk, data FROM guided_verdicts WHERE workspace = ? AND revset = ?",
        )?;
        stmt.query_map([workspace, revset], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect()
    }

    /// Sets each chunk's verdict, or with `None` clears it.
    pub fn set_guided_verdicts(
        &mut self,
        workspace: &str,
        revset: &str,
        verdicts: &[(String, Option<String>)],
    ) -> rusqlite::Result<()> {
        let tx = self.db.transaction()?;
        for (chunk, data) in verdicts {
            match data {
                Some(data) => tx.execute(
                    "INSERT OR REPLACE INTO guided_verdicts (workspace, revset, chunk, data)
                     VALUES (?, ?, ?, ?)",
                    params![workspace, revset, chunk, data],
                )?,
                None => tx.execute(
                    "DELETE FROM guided_verdicts WHERE workspace = ? AND revset = ? AND chunk = ?",
                    params![workspace, revset, chunk],
                )?,
            };
        }
        tx.commit()
    }

    /// Drops the verdicts on chunks of `revset` other than `keep`.
    pub fn keep_guided_verdicts(
        &mut self,
        workspace: &str,
        revset: &str,
        keep: &[&str],
    ) -> rusqlite::Result<()> {
        let gone: Vec<String> = self
            .guided_verdicts(workspace, revset)?
            .into_iter()
            .map(|(chunk, _)| chunk)
            .filter(|c| !keep.contains(&c.as_str()))
            .collect();
        let clear: Vec<_> = gone.into_iter().map(|c| (c, None)).collect();
        self.set_guided_verdicts(workspace, revset, &clear)
    }

    /// Remembers the prompt a terminal agent's session is working on.
    pub fn record_agent_prompt(
        &self,
        session: &str,
        prompt: &str,
        at: i64,
    ) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO agent_prompts (session, prompt, at) VALUES (?, ?, ?)",
            params![session, prompt, at],
        )?;
        Ok(())
    }

    /// Records an edit under the session's last prompt. Edits from before any prompt are
    /// left out, since nothing asked for them.
    pub fn record_agent_edit(
        &self,
        session: &str,
        path: &str,
        text: &str,
        at: i64,
    ) -> rusqlite::Result<()> {
        let prompt: Option<String> = self
            .db
            .query_row(
                "SELECT prompt FROM agent_prompts WHERE session = ?",
                [session],
                |r| r.get(0),
            )
            .optional()?;
        let Some(prompt) = prompt else {
            return Ok(());
        };
        self.db.execute(
            "INSERT INTO agent_edits (session, prompt, path, text, at) VALUES (?, ?, ?, ?, ?)",
            params![session, prompt, path, text, at],
        )?;
        self.db.execute(
            "DELETE FROM agent_edits WHERE id <= (SELECT max(id) FROM agent_edits) - ?",
            [KEEP_EDITS],
        )?;
        Ok(())
    }

    /// Edits to a file whose path ends with one of `paths`, newest first.
    pub fn agent_edits(&self, paths: &[&str]) -> rusqlite::Result<Vec<AgentEdit>> {
        let mut stmt = self.db.prepare(
            "SELECT session, prompt, path, text, at FROM agent_edits ORDER BY at DESC, id DESC",
        )?;
        let all = stmt.query_map([], |r| {
            Ok(AgentEdit {
                session: r.get(0)?,
                prompt: r.get(1)?,
                path: r.get(2)?,
                text: r.get(3)?,
                at: r.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for e in all {
            let e = e?;
            if paths.iter().any(|p| ends_with_path(&e.path, p)) {
                out.push(e);
            }
        }
        Ok(out)
    }
}

/// Whether `full` is `path` or ends with `/path`.
fn ends_with_path(full: &str, path: &str) -> bool {
    full == path
        || full
            .strip_suffix(path)
            .is_some_and(|head| head.ends_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdicts_are_kept_per_chunk_and_cleared() {
        let mut store = Store::in_memory();
        let set = |c: &str, d: Option<&str>| (c.to_string(), d.map(str::to_string));
        store
            .set_guided_verdicts("w", "@", &[set("a", Some("1")), set("b", Some("2"))])
            .unwrap();
        store
            .set_guided_verdicts("w", "@", &[set("a", None)])
            .unwrap();
        assert_eq!(
            store.guided_verdicts("w", "@").unwrap(),
            [("b".to_string(), "2".to_string())]
        );
        assert!(store.guided_verdicts("", "@").unwrap().is_empty());
        store.keep_guided_verdicts("w", "@", &["c"]).unwrap();
        assert!(store.guided_verdicts("w", "@").unwrap().is_empty());
    }

    #[test]
    fn edits_carry_their_sessions_last_prompt_and_match_by_path_suffix() {
        let store = Store::in_memory();
        store
            .record_agent_edit("s", "/r/src/a.rs", "early", 1)
            .unwrap();
        store.record_agent_prompt("s", "Fix the lock", 2).unwrap();
        store.record_agent_edit("s", "/r/src/a.rs", "x", 3).unwrap();
        store.record_agent_prompt("s", "Now the badge", 4).unwrap();
        store
            .record_agent_edit("s", "/r/src/data.rs", "y", 5)
            .unwrap();
        let got = store.agent_edits(&["src/a.rs"]).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(
            (got[0].prompt.as_str(), got[0].text.as_str()),
            ("Fix the lock", "x")
        );
        let data = store.agent_edits(&["data.rs"]).unwrap();
        assert_eq!(data[0].prompt, "Now the badge");
        assert!(store.agent_edits(&["a.rs.bak"]).unwrap().is_empty());
    }

    #[test]
    fn forgetting_a_workspace_drops_its_guided_reviews() {
        let mut store = Store::in_memory();
        store.save_guided("w", "@", "{}").unwrap();
        store
            .set_guided_verdicts("w", "@", &[("a".into(), Some("1".into()))])
            .unwrap();
        store.save_guided("", "@", "{}").unwrap();
        store.forget_workspace("w").unwrap();
        assert!(store.guided("w", "@").unwrap().is_none());
        assert!(store.guided_verdicts("w", "@").unwrap().is_empty());
        assert!(store.guided("", "@").unwrap().is_some());
    }
}
