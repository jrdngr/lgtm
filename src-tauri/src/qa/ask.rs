//! What a terminal agent changes in a session: the questions it asks, rewrites and updates.

use super::queue::{NewQuestion, Rewrite, Update};
use crate::ctx::RepoCtx;
use crate::workspace::Workspace;

/// Where an agent's questions go.
pub enum AskIn {
    /// More questions for a session. A session already sent is done, so they start a new one
    /// from its prompt.
    Session(String),
    /// A new session from a prompt, with the paths to read first.
    New { prompt: String, attach: Vec<String> },
}

impl RepoCtx {
    /// Queues an agent's questions as a round. The agent in `terminal` runs the session's
    /// steps. Returns the session and the new questions' ids in the order given.
    pub fn ask_qa(
        &self,
        ws: &Workspace,
        into: AskIn,
        terminal: Option<String>,
        questions: Vec<NewQuestion>,
    ) -> anyhow::Result<(String, Vec<String>)> {
        let (id, new) = match into {
            AskIn::Session(id) => {
                let s = self.qa_session(ws, &id)?;
                if s.sent.is_some() {
                    (self.create_qa(ws, &s.prompt, s.attach)?.id, true)
                } else {
                    (id, false)
                }
            }
            AskIn::New { prompt, attach } => (self.create_qa(ws, &prompt, attach)?.id, true),
        };
        let ids = self.edit_qa(ws, &id, |s| {
            s.from_terminal |= new;
            // The agent that asked runs the session's steps.
            if terminal.is_some() {
                s.terminal = terminal;
            }
            Ok(s.queue.add_round(questions))
        })?;
        Ok((id, ids))
    }

    /// Rewrites a queued question.
    pub fn reask_qa(&self, ws: &Workspace, id: &str, r: Rewrite) -> anyhow::Result<()> {
        self.edit_qa(ws, id, |s| s.queue.reask(r))
    }

    /// Applies the answer to question `after` to the questions that depend on it.
    pub fn update_qa(
        &self,
        ws: &Workspace,
        id: &str,
        after: &str,
        update: Update,
    ) -> anyhow::Result<()> {
        self.edit_qa(ws, id, |s| {
            anyhow::ensure!(s.queue.get(after).is_some(), "no question {after}");
            s.queue.apply(after, update);
            Ok(())
        })
    }
}
