use serde::Deserialize;
use specta::Type;
use tauri::WebviewWindow;

use super::{CmdResult, err, repo};
use crate::qa::Session;
use crate::qa::queue::{Answer, Turn};

/// The workspace's Q&A sessions, oldest first.
#[tauri::command]
#[specta::specta]
pub(crate) async fn list_qa(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<Vec<Session>> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?.qa_sessions(&ws).map_err(err)
}

/// Starts a session from a prompt. Nothing runs until it's asked.
#[tauri::command]
#[specta::specta]
pub(crate) async fn create_qa(
    window: WebviewWindow,
    workspace: Option<String>,
    prompt: String,
    attach: Vec<String>,
) -> CmdResult<Session> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?.create_qa(&ws, &prompt, attach).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_qa(
    window: WebviewWindow,
    workspace: Option<String>,
    id: String,
) -> CmdResult<()> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?.delete_qa(&ws, &id).map_err(err)
}

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

#[tauri::command]
#[specta::specta]
pub(crate) async fn edit_qa(
    window: WebviewWindow,
    workspace: Option<String>,
    id: String,
    edit: SessionEdit,
) -> CmdResult<Session> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    let ctx = repo(&window)?;
    ctx.edit_qa(&ws, &id, |s| {
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
    })
    .map_err(err)?;
    ctx.qa_session(&ws, &id).map_err(err)
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

#[tauri::command]
#[specta::specta]
pub(crate) async fn act_on_question(
    window: WebviewWindow,
    workspace: Option<String>,
    session: String,
    act: QuestionAct,
) -> CmdResult<Session> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    let ctx = repo(&window)?;
    ctx.edit_qa(&ws, &session, |s| {
        let q = &mut s.queue;
        match act {
            QuestionAct::Decide { id, answer } => q.decide(
                &id,
                Answer {
                    option: answer.option,
                    note: answer.note.filter(|n| !n.trim().is_empty()),
                    other: answer.other.filter(|o| !o.trim().is_empty()),
                    at: crate::review::now_ms() as f64,
                    auto: None,
                },
            ),
            QuestionAct::Undo => {
                q.undo();
                Ok(())
            }
            QuestionAct::Defer { id } => q.defer(&id, crate::review::now_ms() as f64).map(|_| ()),
            QuestionAct::Reopen { id } => q.change_answer(&id, crate::review::now_ms() as f64),
            QuestionAct::Seen { id } => q.seen(&id),
        }
    })
    .map_err(err)?;
    ctx.qa_session(&ws, &session).map_err(err)
}

/// The open questions deferring question `id` would also defer.
#[tauri::command]
#[specta::specta]
pub(crate) async fn defer_preview(
    window: WebviewWindow,
    workspace: Option<String>,
    session: String,
    id: String,
) -> CmdResult<Vec<String>> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    let s = repo(&window)?.qa_session(&ws, &session).map_err(err)?;
    Ok(s.queue.defer_preview(&id))
}

/// Adds a turn to question `id`'s discussion: the user's message, or the agent's reply.
#[tauri::command]
#[specta::specta]
pub(crate) async fn add_qa_turn(
    window: WebviewWindow,
    workspace: Option<String>,
    session: String,
    id: String,
    you: bool,
    text: String,
) -> CmdResult<Session> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?
        .add_qa_turn(&ws, &session, &id, Turn { you, text })
        .map_err(err)
}
