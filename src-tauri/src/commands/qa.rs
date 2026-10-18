use tauri::WebviewWindow;

use super::{CmdResult, err, repo};
use crate::qa::queue::Turn;
use crate::qa::{QuestionAct, Session, SessionEdit};

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

#[tauri::command]
#[specta::specta]
pub(crate) async fn edit_qa(
    window: WebviewWindow,
    workspace: Option<String>,
    id: String,
    edit: SessionEdit,
) -> CmdResult<Session> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?.edit_qa_session(&ws, &id, edit).map_err(err)
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
    repo(&window)?
        .act_on_question(&ws, &session, act)
        .map_err(err)
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
