//! GitHub threads' workflow: where each thread on a PR is, from new to done, and the agent's
//! take on it. A comment with a thread state is a thread (on GitHub, or a draft of one); one
//! without is a local note.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::comments::{COLUMNS, row_to_comment};
use super::{Comment, GhLink, Store, json_err};

/// Where a thread is in the reply workflow.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    /// Synced, not triaged yet.
    #[default]
    New,
    /// Needs a call only the user can make.
    Decide,
    /// Agreed; the code is to change as asked.
    ToFix,
    /// A reply is being worked out.
    Reply,
    /// The reply is final, or the fix is in and needs none. Goes out with the next send.
    Ready,
    /// Sent, resolved, or answered already.
    Done,
}

/// What the agent thinks of a thread's ask.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TakeVerdict {
    Agree,
    AgreeWithChanges,
    PushBack,
    NeedsYou,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Take {
    pub verdict: TakeVerdict,
    pub why: String,
    /// What the user has to decide, for `NeedsYou`.
    pub question: Option<String>,
}

/// Where a proposed file change is: waiting for the reply to go final, written, or not
/// written because the file changed after it was proposed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ProposedState {
    Proposed,
    Applied,
    Stale,
}

/// A change to one file the agent proposed in a thread's chat, written once the reply is
/// final. `before` is empty for a new file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Proposed {
    /// Relative to the workspace root.
    pub path: String,
    pub before: String,
    pub after: String,
    pub state: ProposedState,
}

/// A thread's place in the workflow, kept across syncs and restarts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ThreadState {
    /// `owner/name` of the PR's repo.
    pub repo: String,
    pub pr: u32,
    pub stage: Stage,
    pub take: Option<Take>,
    /// A few words on what the thread asks, from triage.
    pub summary: String,
    /// Code changes the agent's reply promises, by file.
    #[serde(default)]
    pub proposal: Vec<Proposed>,
}

impl ThreadState {
    pub fn new(repo: &str, pr: u32, stage: Stage) -> Self {
        Self {
            repo: repo.to_string(),
            pr,
            stage,
            take: None,
            summary: String::new(),
            proposal: vec![],
        }
    }

    /// The state of a thread pulled before threads had one.
    pub(crate) fn from_link(link: &GhLink) -> Self {
        let stage = if link.resolved {
            Stage::Done
        } else {
            Stage::New
        };
        Self::new(&link.repo, link.pr, stage)
    }
}

pub(super) fn thread_to_sql(t: &Option<ThreadState>) -> rusqlite::Result<String> {
    match t {
        Some(t) => serde_json::to_string(t).map_err(json_err),
        None => Ok(String::new()),
    }
}

pub(super) fn thread_from_sql(
    s: String,
    github: Option<&GhLink>,
) -> rusqlite::Result<Option<ThreadState>> {
    if s.is_empty() {
        return Ok(github.map(ThreadState::from_link));
    }
    serde_json::from_str(&s).map(Some).map_err(json_err)
}

impl Store {
    pub fn set_thread(&self, id: i64, thread: &Option<ThreadState>) -> rusqlite::Result<()> {
        self.db.execute(
            "UPDATE comments SET thread = ? WHERE id = ?",
            params![thread_to_sql(thread)?, id],
        )?;
        Ok(())
    }

    /// Threads and drafts on `repo`'s PR `pr`.
    pub fn pr_threads(&self, repo: &str, pr: u32) -> rusqlite::Result<Vec<Comment>> {
        let sql = format!("SELECT {COLUMNS} FROM comments WHERE thread != '' OR github != ''");
        let mut stmt = self.db.prepare(&sql)?;
        let rows = stmt.query_map([], row_to_comment)?;
        let all: rusqlite::Result<Vec<Comment>> = rows.collect();
        Ok(all?
            .into_iter()
            .filter(|c| {
                c.thread
                    .as_ref()
                    .is_some_and(|t| t.repo == repo && t.pr == pr)
            })
            .collect())
    }
}
