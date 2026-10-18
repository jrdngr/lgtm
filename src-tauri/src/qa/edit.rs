//! What the user changes in the Q&A tab without the agent: a session's own fields, and
//! decisions on its questions.

use serde::Deserialize;
use specta::Type;

use super::Session;
use super::queue::Answer;
use crate::ctx::{RepoCtx, now_ms};
use crate::workspace::Workspace;

/// A change to a session that doesn't need the agent.
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SessionEdit {
    Prompt {
        prompt: String,
        attach: Vec<String>,
    },
    /// What the user adds for the write.
    Note {
        note: String,
    },
    /// Implemented, or back to active.
    Archive {
        archived: bool,
    },
    /// It went to a terminal, named by its title.
    Sent {
        terminal: String,
    },
    /// The terminal panel whose agent now runs its steps.
    Terminal {
        id: String,
    },
}

/// An answer to a question: an option from 0, or none of them with `other`.
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AnswerInput {
    pub option: Option<u32>,
    pub note: Option<String>,
    pub other: Option<String>,
}

/// What can happen to a question without the agent.
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum QuestionAct {
    Decide {
        id: String,
        answer: AnswerInput,
    },
    /// Takes back the last decision or defer.
    Undo,
    /// Puts it off, with the open questions that depend on it.
    Defer {
        id: String,
    },
    /// Opens a decided question to answer it again.
    Reopen {
        id: String,
    },
    /// The user saw what a rewrite changed.
    Seen {
        id: String,
    },
}

impl RepoCtx {
    pub fn edit_qa_session(
        &self,
        ws: &Workspace,
        id: &str,
        edit: SessionEdit,
    ) -> anyhow::Result<Session> {
        self.edit_qa(ws, id, |s| {
            match edit {
                SessionEdit::Prompt { prompt, attach } => {
                    s.prompt = prompt;
                    s.attach = attach;
                }
                SessionEdit::Note { note } => s.note = note,
                SessionEdit::Archive { archived } => s.archived = archived,
                SessionEdit::Sent { terminal } => s.sent = Some(terminal),
                SessionEdit::Terminal { id } => s.terminal = Some(id),
            }
            Ok(())
        })?;
        self.qa_session(ws, id)
    }

    pub fn act_on_question(
        &self,
        ws: &Workspace,
        id: &str,
        act: QuestionAct,
    ) -> anyhow::Result<Session> {
        self.edit_qa(ws, id, |s| {
            let q = &mut s.queue;
            match act {
                QuestionAct::Decide { id, answer } => q.decide(
                    &id,
                    Answer {
                        option: answer.option,
                        note: answer.note.filter(|n| !n.trim().is_empty()),
                        other: answer.other.filter(|o| !o.trim().is_empty()),
                        at: now_ms() as f64,
                        auto: None,
                    },
                ),
                QuestionAct::Undo => {
                    q.undo();
                    Ok(())
                }
                QuestionAct::Defer { id } => q.defer(&id, now_ms() as f64).map(|_| ()),
                QuestionAct::Reopen { id } => q.change_answer(&id, now_ms() as f64),
                QuestionAct::Seen { id } => q.seen(&id),
            }
        })?;
        self.qa_session(ws, id)
    }
}
