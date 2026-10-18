//! Q&A: working through decisions with a terminal agent before it writes them into the repo.
//! A session is a prompt and the question queue built from it. Sessions live in the store,
//! never in the repo.

mod ask;
pub mod discuss;
mod edit;
#[cfg(test)]
mod edit_tests;
mod order;
pub mod queue;
pub mod text;
pub mod update;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::ctx::{RepoCtx, now_ms};
use crate::workspace::Workspace;
use queue::{QState, Queue};

pub use ask::AskIn;
pub use edit::{QuestionAct, SessionEdit};

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
    /// The terminal it was sent to, when it was.
    #[serde(default)]
    pub sent: Option<String>,
    /// A terminal agent asked its questions, and waits for the decisions.
    #[serde(default)]
    pub from_terminal: bool,
    /// The terminal panel whose agent asked the questions. It applies each answer and talks
    /// questions over, and Apply in terminal sends it the decisions.
    #[serde(default)]
    pub terminal: Option<String>,
    /// Implemented: marked so after a send.
    #[serde(default)]
    pub archived: bool,
    pub created: f64,
}

impl Session {
    /// How many of its questions are in `state`.
    pub fn count(&self, state: QState) -> usize {
        self.queue
            .questions
            .iter()
            .filter(|q| q.state == state)
            .count()
    }
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

    /// Changes a session in the store. Answers and the agent's tools both go through here.
    fn edit_qa<T>(
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
            sent: None,
            from_terminal: false,
            terminal: None,
            archived: false,
            created: created as f64,
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
