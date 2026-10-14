use serde::Deserialize;
use specta::Type;
use tauri::ipc::Channel;
use tauri::{State, WebviewWindow};

use super::{CmdResult, err, repo};
use crate::agent::Runs;
use crate::qa::Session;
use crate::qa::queue::Answer;

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
    PlanComment {
        quote: String,
        body: String,
    },
    DeletePlanComment {
        id: u32,
    },
    DiscardPlan,
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
    if let SessionEdit::PlanComment { quote, body } = &edit {
        return ctx.add_plan_comment(&ws, &id, quote, body).map_err(err);
    }
    ctx.edit_qa(&ws, &id, |s| {
        match edit {
            SessionEdit::Prompt { prompt, attach } => {
                s.prompt = prompt;
                s.attach = attach;
            }
            SessionEdit::Note { note } => s.note = note,
            SessionEdit::Archive { archived } => s.archived = archived,
            SessionEdit::Sent { terminal } => s.sent = Some(terminal),
            SessionEdit::DeletePlanComment { id } => {
                if let Some(p) = s.plan.as_mut() {
                    p.comments.retain(|c| c.id != id);
                }
            }
            SessionEdit::DiscardPlan => s.plan = None,
            SessionEdit::PlanComment { .. } => {}
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

/// Has the agent turn the session's prompt into questions, answering the obvious ones
/// itself when `auto` is on.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ask_qa(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    workspace: Option<String>,
    session: String,
    auto: bool,
    on_tool: Channel<String>,
) -> CmdResult<Session> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?
        .ask_qa(&ws, &session, auto, &runs, |t| {
            on_tool.send(t).ok();
        })
        .await
        .map_err(err)
}

/// Applies the answer to question `id` to the rest of the queue.
#[tauri::command]
#[specta::specta]
pub(crate) async fn apply_answer(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    workspace: Option<String>,
    session: String,
    id: String,
) -> CmdResult<Session> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?
        .apply_answer(&ws, &session, &id, &runs)
        .await
        .map_err(err)
}

/// A discussion turn on a question, streaming the agent's reply through `on_text`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn discuss_question(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    workspace: Option<String>,
    session: String,
    id: String,
    message: String,
    on_text: Channel<String>,
) -> CmdResult<Session> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?
        .discuss_question(&ws, &session, &id, &message, &runs, |t| {
            on_text.send(t).ok();
        })
        .await
        .map_err(err)
}

/// Rewrites a question after its discussion.
#[tauri::command]
#[specta::specta]
pub(crate) async fn reask_question(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    workspace: Option<String>,
    session: String,
    id: String,
) -> CmdResult<Session> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?
        .reask_question(&ws, &session, &id, &runs)
        .await
        .map_err(err)
}

/// Plans the write, or revises the plan with the comments on it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn write_plan(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    workspace: Option<String>,
    session: String,
    on_tool: Channel<String>,
) -> CmdResult<Session> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?
        .write_plan(&ws, &session, &runs, |t| {
            on_tool.send(t).ok();
        })
        .await
        .map_err(err)
}

/// Writes the decisions into the workspace's @.
#[tauri::command]
#[specta::specta]
pub(crate) async fn apply_qa(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    workspace: Option<String>,
    session: String,
    on_tool: Channel<String>,
) -> CmdResult<Session> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?
        .apply_qa(&ws, &session, &runs, |t| {
            on_tool.send(t).ok();
        })
        .await
        .map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn stop_qa(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    workspace: Option<String>,
    session: String,
) -> CmdResult<()> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?.stop_qa(&ws, &session, &runs);
    Ok(())
}
