//! Q&A: working through decisions with an agent before it writes them into the repo. A
//! session is a prompt, the question queue built from it, an optional plan, and what Apply
//! did. Sessions live in the store, never in the repo.

pub mod agent;
mod discuss;
mod order;
pub mod queue;
pub mod text;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::review::{RepoCtx, now_ms};
use crate::workspace::Workspace;
use queue::Queue;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum FileChange {
    New,
    Edit,
    Delete,
}

/// A file the plan would touch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanFile {
    pub path: String,
    pub change: FileChange,
    /// What changes in it, a line each.
    pub edits: Vec<String>,
    /// The ids of the decisions it follows from.
    pub decisions: Vec<String>,
}

/// A comment on a passage of the plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanComment {
    pub id: u32,
    pub quote: String,
    pub body: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub files: Vec<PlanFile>,
    #[serde(default)]
    pub comments: Vec<PlanComment>,
}

/// What Apply did.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    pub summary: String,
    /// Files it created or edited, relative to the workspace root.
    pub files: Vec<String>,
    pub at: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub title: String,
    pub prompt: String,
    /// Paths the agent reads first, relative to the workspace root.
    #[serde(default)]
    pub attach: Vec<String>,
    #[serde(default)]
    pub queue: Queue,
    /// The agent has turned the prompt into questions, even when it found none.
    #[serde(default)]
    pub asked: bool,
    /// Anything the user adds for the write.
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub plan: Option<Plan>,
    #[serde(default)]
    pub applied: Option<Applied>,
    /// The terminal it was sent to, when it was.
    #[serde(default)]
    pub sent: Option<String>,
    /// A terminal agent asked its questions, and waits for the decisions.
    #[serde(default)]
    pub from_terminal: bool,
    /// That terminal agent's Claude Code session, which the agent here starts from a copy of.
    #[serde(default)]
    pub fork_from: Option<String>,
    /// Implemented: applied, or marked so after a send.
    #[serde(default)]
    pub archived: bool,
    pub created: f64,
    /// The agent's conversation, resumed by each step.
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    next_comment: u32,
}

/// A title from the prompt's first line, until the agent names it.
fn title(prompt: &str) -> String {
    let line = prompt
        .lines()
        .map(|l| l.trim().trim_start_matches('#').trim())
        .find(|l| !l.is_empty())
        .unwrap_or("Q&A");
    match line.char_indices().nth(60) {
        Some((i, _)) => format!("{}…", &line[..i]),
        None => line.to_string(),
    }
}

/// The store's key for a workspace.
fn ws_key(ws: &Workspace) -> &str {
    ws.name.as_deref().unwrap_or("")
}

impl RepoCtx {
    pub fn qa_sessions(&self, ws: &Workspace) -> anyhow::Result<Vec<Session>> {
        let rows = self.store.lock().unwrap().qa_sessions(ws_key(ws))?;
        rows.iter()
            .map(|d| serde_json::from_str(d).context("a stored Q&A session can't be read"))
            .collect()
    }

    pub fn qa_session(&self, ws: &Workspace, id: &str) -> anyhow::Result<Session> {
        let data = self.store.lock().unwrap().qa_session(ws_key(ws), id)?;
        let data = data.with_context(|| format!("there's no Q&A session {id}"))?;
        Ok(serde_json::from_str(&data)?)
    }

    /// Changes a session in the store. Answers and agent steps both go through here.
    pub fn edit_qa<T>(
        &self,
        ws: &Workspace,
        id: &str,
        f: impl FnOnce(&mut Session) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let store = self.store.lock().unwrap();
        let data = store
            .qa_session(ws_key(ws), id)?
            .with_context(|| format!("there's no Q&A session {id}"))?;
        let mut s: Session = serde_json::from_str(&data)?;
        let out = f(&mut s)?;
        store.save_qa_session(ws_key(ws), id, &serde_json::to_string(&s)?)?;
        Ok(out)
    }

    pub fn create_qa(
        &self,
        ws: &Workspace,
        prompt: &str,
        attach: Vec<String>,
    ) -> anyhow::Result<Session> {
        let created = now_ms();
        let s = Session {
            id: format!("{created:013}"),
            title: title(prompt),
            prompt: prompt.to_string(),
            attach,
            queue: Queue::default(),
            asked: false,
            note: String::new(),
            plan: None,
            applied: None,
            sent: None,
            from_terminal: false,
            fork_from: None,
            archived: false,
            created: created as f64,
            agent: None,
            next_comment: 0,
        };
        self.store.lock().unwrap().save_qa_session(
            ws_key(ws),
            &s.id,
            &serde_json::to_string(&s)?,
        )?;
        Ok(s)
    }

    pub fn delete_qa(&self, ws: &Workspace, id: &str) -> anyhow::Result<()> {
        Ok(self
            .store
            .lock()
            .unwrap()
            .delete_qa_session(ws_key(ws), id)?)
    }

    pub fn add_plan_comment(
        &self,
        ws: &Workspace,
        id: &str,
        quote: &str,
        body: &str,
    ) -> anyhow::Result<Session> {
        self.edit_qa(ws, id, |s| {
            let plan = s.plan.as_mut().context("there's no plan to comment on")?;
            s.next_comment += 1;
            plan.comments.push(PlanComment {
                id: s.next_comment,
                quote: quote.to_string(),
                body: body.to_string(),
            });
            Ok(())
        })?;
        self.qa_session(ws, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_come_from_the_prompt() {
        assert_eq!(
            title("\n## Workspace templates\nmore"),
            "Workspace templates"
        );
        assert_eq!(title("  "), "Q&A");
        assert_eq!(title(&"a".repeat(70)), format!("{}…", "a".repeat(60)));
    }
}
